//! Background presence heartbeat, daemon-mediated agent progress outbox deliverer,
//! and honest, hard-capped dead-man-switch safety net.
//!
//! Replaces the old blind 3-minute template loop with a two-tier design:
//! 1. **Primary (Agent-Driven)**: drains rate-limited, human-written milestone updates
//!    enqueued via `aina progress "<pesan>"` within <= 2 seconds, records them in
//!    conversation history, and resets the silent fallback timer.
//! 2. **Fallback Safety Net (Dead-Man Switch)**: only speaks if the agent is completely
//!    silent for an extended window (e.g. blocked inside a single heavy command) — never
//!    claims false stages ("menuju tahap akhir") and stops after the configured cap.

use crate::core::domain::{PresenceState, ProgressPolicy, SessionRole};
use crate::core::ports::{SessionStorePort, WhatsAppPort};
use std::sync::Arc;
use tracing::warn;

pub struct HeartbeatGuard {
    handle: Option<tokio::task::JoinHandle<()>>,
    whatsapp: Arc<dyn WhatsAppPort>,
    chat_jid: String,
    session_role: SessionRole,
}

impl HeartbeatGuard {
    pub fn new(
        handle: tokio::task::JoinHandle<()>,
        whatsapp: Arc<dyn WhatsAppPort>,
        chat_jid: String,
        session_role: SessionRole,
    ) -> Self {
        Self {
            handle: Some(handle),
            whatsapp,
            chat_jid,
            session_role,
        }
    }

    pub fn dismiss(&mut self) {
        if let Some(h) = self.handle.take() {
            h.abort();
        }
    }
}

impl Drop for HeartbeatGuard {
    fn drop(&mut self) {
        if let Some(h) = self.handle.take() {
            h.abort();
            let wa = Arc::clone(&self.whatsapp);
            let jid = self.chat_jid.clone();
            let role = self.session_role;
            tokio::spawn(async move {
                let _ = wa.send_presence_with_session(&jid, PresenceState::Paused, role).await;
            });
        }
    }
}

/// Spawns a background task maintaining WhatsApp composing presence, delivering
/// agent-authored `aina progress` outbox updates, and running the honest dead-man-switch
/// fallback if the agent is silent while blocked on a heavy tool call.
#[allow(clippy::too_many_arguments)]
pub fn start_presence_heartbeat(
    whatsapp: Arc<dyn WhatsAppPort>,
    session_store: Arc<dyn SessionStorePort>,
    bot_jid: String,
    chat_jid: String,
    turn_token: String,
    session_role: SessionRole,
    quote_id: Option<String>,
) -> HeartbeatGuard {
    start_presence_heartbeat_with_interval(
        whatsapp,
        session_store,
        bot_jid,
        chat_jid,
        turn_token,
        session_role,
        quote_id,
        tokio::time::Duration::from_secs(2),
        ProgressPolicy::from_env(),
    )
}

#[allow(clippy::too_many_arguments)]
fn start_presence_heartbeat_with_interval(
    whatsapp: Arc<dyn WhatsAppPort>,
    session_store: Arc<dyn SessionStorePort>,
    bot_jid: String,
    chat_jid: String,
    turn_token: String,
    session_role: SessionRole,
    quote_id: Option<String>,
    tick_interval: tokio::time::Duration,
    policy: ProgressPolicy,
) -> HeartbeatGuard {
    let wa_clone = Arc::clone(&whatsapp);
    let target_jid = chat_jid.clone();

    let heartbeat_handle = tokio::spawn(async move {
        let mut tick_count: u64 = 0;
        let mut fallback_count: u32 = 0;
        let mut last_fallback_at: Option<i64> = None;

        loop {
            tokio::time::sleep(tick_interval).await;
            tick_count += 1;

            // 1. Drain any agent-authored progress updates from SQLite outbox
            match session_store.take_pending_progress_messages(&turn_token).await {
                Ok(pending) if !pending.is_empty() => {
                    for item in pending {
                        if let Err(e) = wa_clone
                            .send_text_with_session(
                                &target_jid,
                                &item.text,
                                quote_id.as_deref(),
                                session_role,
                            )
                            .await
                        {
                            warn!("Failed to deliver agent progress update to {}: {}", target_jid, e);
                            continue;
                        }
                        let _ = session_store
                            .record_message(&target_jid, &bot_jid, &item.text, true)
                            .await;
                    }
                    // Agent spoke substantively: reset silent dead-man-switch counters
                    fallback_count = 0;
                    last_fallback_at = None;
                }
                Ok(_) => {}
                Err(e) => warn!("Failed to poll turn progress outbox: {}", e),
            }

            // 2. Keep typing presence alive every ~4 seconds (every 2nd tick)
            if tick_count % 2 == 0 {
                let _ = wa_clone
                    .send_presence_with_session(&target_jid, PresenceState::Composing, session_role)
                    .await;
            }

            // 3. Honest dead-man-switch fallback if completely silent for extended period
            if let Ok(Some(session)) = session_store.get_turn_progress_session(&turn_token).await {
                let now_epoch = chrono::Utc::now().timestamp();
                if let Some(fallback_msg) = policy.evaluate_fallback_heartbeat(
                    &session,
                    last_fallback_at,
                    fallback_count,
                    now_epoch,
                ) {
                    if wa_clone
                        .send_text_with_session(
                            &target_jid,
                            &fallback_msg,
                            quote_id.as_deref(),
                            session_role,
                        )
                        .await
                        .is_ok()
                    {
                        let _ = session_store
                            .record_message(&target_jid, &bot_jid, &fallback_msg, true)
                            .await;
                        fallback_count += 1;
                        last_fallback_at = Some(now_epoch);
                    }
                }
            }
        }
    });

    HeartbeatGuard::new(heartbeat_handle, whatsapp, chat_jid, session_role)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::driven::SqliteSessionStore;
    use crate::core::domain::TurnProgressSession;

    struct RecordingWa {
        sent: Arc<tokio::sync::Mutex<Vec<(String, String)>>>,
    }

    #[async_trait::async_trait]
    impl WhatsAppPort for RecordingWa {
        async fn send_text(&self, to: &str, text: &str, _qid: Option<&str>) -> anyhow::Result<()> {
            self.sent.lock().await.push((to.to_string(), text.to_string()));
            Ok(())
        }
        async fn send_presence(&self, _to: &str, _state: PresenceState) -> anyhow::Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_heartbeat_delivers_agent_progress_outbox_and_records_history() {
        let dir = std::env::temp_dir().join(format!("aina_test_hb_{}", rand::random::<u32>()));
        let db_file = dir.join("hb.db");
        let store: Arc<dyn SessionStorePort> = Arc::new(SqliteSessionStore::new(&db_file).unwrap());
        let sent = Arc::new(tokio::sync::Mutex::new(Vec::new()));
        let wa: Arc<dyn WhatsAppPort> = Arc::new(RecordingWa { sent: Arc::clone(&sent) });

        let now = chrono::Utc::now().timestamp();
        let token = "tok_hb_delivery_01";
        let chat = "628123456@s.whatsapp.net";
        let bot = "628999888@s.whatsapp.net";

        store
            .create_turn_progress_session(&TurnProgressSession {
                turn_token: token.to_string(),
                chat_jid: chat.to_string(),
                chat_type: "direct".to_string(),
                message_id: "m1".to_string(),
                started_at_epoch: now - 30,
                last_progress_at_epoch: None,
                last_progress_text: None,
                progress_count: 0,
                is_active: true,
            })
            .await
            .unwrap();

        let policy = ProgressPolicy::default();
        // Simulate `aina progress` CLI enqueuing an update in SQLite
        {
            let conn = rusqlite::Connection::open(&db_file).unwrap();
            crate::adapters::driven::sqlite_store::session::try_enqueue_turn_progress_update(
                &conn,
                token,
                "Sebentar yaa mas, ketemu format EPUB, lagi Aina unduh dari mirror.",
                &policy,
                now,
            )
            .unwrap()
            .unwrap();
        }

        let mut guard = start_presence_heartbeat_with_interval(
            wa,
            Arc::clone(&store),
            bot.to_string(),
            chat.to_string(),
            token.to_string(),
            SessionRole::PrimaryBot,
            None,
            tokio::time::Duration::from_millis(30),
            policy,
        );

        tokio::time::sleep(tokio::time::Duration::from_millis(120)).await;
        guard.dismiss();
        store.complete_turn_progress_session(token).await.unwrap();

        let delivered = sent.lock().await.clone();
        assert_eq!(delivered.len(), 1);
        assert_eq!(delivered[0].0, chat);
        assert!(delivered[0].1.contains("format EPUB"));

        let history = store.get_recent_messages(chat, None, 5).await.unwrap();
        assert!(history.iter().any(|m| m.is_from_me && m.text.contains("format EPUB")));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
