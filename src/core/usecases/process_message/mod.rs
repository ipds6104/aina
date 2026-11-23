//! Core orchestrator use case for processing incoming WhatsApp messages.

mod audit_tracker;
mod builtin_commands;
mod context_preparer;
mod error_handler;
mod heartbeat;
mod response_deliverer;

pub use audit_tracker::AuditTracker;
pub use builtin_commands::try_handle_builtin_command;
pub use context_preparer::{resolve_authority, ContextPreparation, ContextPreparer};
pub use error_handler::ErrorHandler;
pub use heartbeat::start_presence_heartbeat;
pub use response_deliverer::ResponseDeliverer;

// Re-export response formatting & conversational heuristics from domain layer for backward compatibility
#[allow(unused_imports)]
pub use crate::core::domain::{detect_conversational_closing, split_response_into_bubbles};

use crate::core::domain::{
    Authority, ChatType, Gatekeeper, GatekeeperDecision, IncomingMessage, IngressRateLimiter,
    PersonaEngine, PresenceState, RateLimitConfig, ThrottleDecision,
};
use crate::core::ports::{AgentEnginePort, SessionStorePort, WhatsAppPort};
use std::sync::Arc;
use tracing::{error, info, warn};

pub struct ProcessIncomingMessageUseCase {
    session_store: Arc<dyn SessionStorePort>,
    agent_engine: Arc<dyn AgentEnginePort>,
    whatsapp: Arc<dyn WhatsAppPort>,
    persona_engine: Arc<PersonaEngine>,
    rate_limiter: Arc<tokio::sync::Mutex<IngressRateLimiter>>,
    bot_jid: String,
    bot_name: String,
    bot_lid: Option<String>,
}

fn format_record_text(text: &str, has_media: bool, media_path: Option<&str>) -> String {
    if has_media {
        if let Some(path) = media_path {
            format!("{} [Media: {}]", text, path)
        } else {
            format!("{} [Media]", text)
        }
    } else {
        text.to_string()
    }
}

impl ProcessIncomingMessageUseCase {
    pub fn new(
        session_store: Arc<dyn SessionStorePort>,
        agent_engine: Arc<dyn AgentEnginePort>,
        whatsapp: Arc<dyn WhatsAppPort>,
        persona_engine: Arc<PersonaEngine>,
        bot_jid: String,
        bot_name: String,
        bot_lid: Option<String>,
    ) -> Self {
        Self {
            session_store,
            agent_engine,
            whatsapp,
            persona_engine,
            rate_limiter: Arc::new(tokio::sync::Mutex::new(IngressRateLimiter::new(
                RateLimitConfig::from_env(),
            ))),
            bot_jid,
            bot_name,
            bot_lid,
        }
    }

    #[allow(dead_code)]
    pub fn with_rate_limit_config(mut self, config: RateLimitConfig) -> Self {
        self.rate_limiter = Arc::new(tokio::sync::Mutex::new(IngressRateLimiter::new(config)));
        self
    }

    pub fn whatsapp(&self) -> &Arc<dyn WhatsAppPort> {
        &self.whatsapp
    }

    pub async fn execute(&self, msg: IncomingMessage) -> anyhow::Result<()> {
        let chat_type_str = match msg.chat_type {
            ChatType::DirectMessage => "direct",
            ChatType::Group => "group",
        };

        let decision = Gatekeeper::evaluate(
            &msg,
            &self.bot_jid,
            &self.bot_name,
            self.bot_lid.as_deref(),
        );

        match decision {
            GatekeeperDecision::Ignore { reason } => {
                info!("Ignoring message {}: {}", msg.id, reason);
                AuditTracker::record_terminal_event(
                    &self.session_store,
                    msg.id,
                    msg.chat_jid,
                    msg.sender.jid,
                    msg.sender.name,
                    chat_type_str,
                    msg.text,
                    msg.has_media,
                    msg.media_path,
                    "ignore",
                    &reason,
                    "ignored",
                    None,
                    None,
                    None,
                    Some(0.0),
                )
                .await?;
                Ok(())
            }
            GatekeeperDecision::RecordOnly { reason } => {
                info!(
                    "Recording ambient group message from {} in {}: {}",
                    msg.sender.jid, msg.chat_jid, reason
                );
                let record_text =
                    format_record_text(&msg.text, msg.has_media, msg.media_path.as_deref());
                self.session_store
                    .record_message(&msg.chat_jid, &msg.sender.jid, &record_text, false)
                    .await?;

                if msg.chat_type == ChatType::Group {
                    let _ = self.session_store.record_group_membership(
                        &msg.chat_jid,
                        &msg.sender.jid,
                        msg.sender.name.as_deref(),
                        Some("member"),
                    ).await;
                }

                if let Some(ref name) = msg.sender.name {
                    let clean_name = name.trim();
                    if !clean_name.is_empty() {
                        let existing = self.session_store.get_user_profile(&msg.sender.jid).await.ok().flatten();
                        match existing {
                            Some(mut p) => {
                                if p.name.as_deref().unwrap_or("").trim().is_empty() {
                                    p.name = Some(clean_name.to_string());
                                    let _ = self.session_store.save_user_profile(&p).await;
                                }
                            }
                            None => {
                                let is_trusted_group = self.session_store.is_group_trusted(&msg.chat_jid).await.unwrap_or(false);
                                let (authority_level, role) = if is_trusted_group {
                                    ("staff".to_string(), Some("Rekan Tim".to_string()))
                                } else {
                                    ("guest".to_string(), Some("Anggota Grup".to_string()))
                                };
                                let new_profile = crate::core::ports::UserProfile {
                                    sender_jid: msg.sender.jid.clone(),
                                    name: Some(clean_name.to_string()),
                                    role,
                                    authority_level,
                                    notes: Some("Terdaftar otomatis dari obrolan grup (Zero Implicit Trust)".to_string()),
                                    granted_by: None,
                                    granted_at: None,
                                    role_hierarchy: Some("guest".to_string()),
                                    verification_status: if is_trusted_group { "TENTATIVE".to_string() } else { "UNCONFIRMED".to_string() },
                                    ..Default::default()
                                };
                                let _ = self.session_store.save_user_profile(&new_profile).await;
                            }
                        }
                    }
                }

                AuditTracker::record_terminal_event(
                    &self.session_store,
                    msg.id,
                    msg.chat_jid,
                    msg.sender.jid,
                    msg.sender.name,
                    chat_type_str,
                    record_text,
                    msg.has_media,
                    msg.media_path,
                    "record_only",
                    &reason,
                    "recorded",
                    None,
                    None,
                    None,
                    Some(0.0),
                )
                .await?;
                Ok(())
            }
            GatekeeperDecision::Respond { reason } => {
                // Phase 19 Layer 2: Authority-aware pre-LLM ingress rate limiter (ADR-018)
                let sender_authority = if crate::core::domain::capability::is_admin_identity(
                    self.persona_engine.admin_jid(),
                    &msg.sender.jid,
                ) {
                    Authority::Admin
                } else if let Ok(Some(profile)) =
                    self.session_store.get_user_profile(&msg.sender.jid).await
                {
                    context_preparer::resolve_authority(
                        &msg.sender.jid,
                        self.persona_engine.admin_jid(),
                        &profile.authority_level,
                    )
                } else if msg.chat_type == ChatType::Group
                    && self
                        .session_store
                        .is_group_trusted(&msg.chat_jid)
                        .await
                        .unwrap_or(false)
                {
                    Authority::Staff
                } else {
                    Authority::Guest
                };

                let now_epoch = chrono::Utc::now().timestamp();
                let (throttle_decision, tombstone_threshold) = {
                    let mut limiter = self.rate_limiter.lock().await;
                    let threshold = limiter.config().tombstone_threshold;
                    (
                        limiter.check(&msg.sender.jid, sender_authority, now_epoch),
                        threshold,
                    )
                };

                match throttle_decision {
                    ThrottleDecision::Allow => {}
                    ThrottleDecision::NotifyOnceAndDrop { retry_after_secs } => {
                        warn!(
                            "Ingress rate limit triggered for {} ({:?}): sending 1x cooldown notice ({}s)",
                            msg.sender.jid, sender_authority, retry_after_secs
                        );
                        let notice = format!(
                            "Pesan diterima. Aina sedang memproses permintaan lain; mohon tunggu sekitar {} detik sebelum mengirim pesan berikutnya ya.",
                            retry_after_secs
                        );
                        let quote_id = match msg.chat_type {
                            ChatType::Group => Some(msg.id.as_str()),
                            ChatType::DirectMessage => None,
                        };
                        let _ = self
                            .whatsapp
                            .send_text_with_session(
                                &msg.chat_jid,
                                &notice,
                                quote_id,
                                msg.session_role,
                            )
                            .await;

                        let _ = AuditTracker::record_terminal_event(
                            &self.session_store,
                            msg.id,
                            msg.chat_jid,
                            msg.sender.jid,
                            msg.sender.name,
                            chat_type_str,
                            msg.text,
                            msg.has_media,
                            msg.media_path,
                            "rate_limited",
                            "INGRESS_RATE_LIMIT_NOTIFIED",
                            "throttled",
                            Some(&notice),
                            Some("security:rate_limiter"),
                            Some("security_rate_limit"),
                            Some(0.0),
                        )
                        .await;
                        return Ok(());
                    }
                    ThrottleDecision::SilentDrop {
                        retry_after_secs,
                        consecutive_hits,
                        tombstoned,
                    } => {
                        let reason_code = if tombstoned {
                            "INGRESS_RATE_LIMIT_TOMBSTONED"
                        } else {
                            "INGRESS_RATE_LIMIT_DROPPED"
                        };
                        let should_audit = !tombstoned
                            || consecutive_hits == tombstone_threshold
                            || consecutive_hits % 25 == 0;
                        if tombstoned {
                            warn!(
                                "Silent-dropping tombstoned sender {} (hits={}, retry_after={}s)",
                                msg.sender.jid, consecutive_hits, retry_after_secs
                            );
                        } else {
                            info!(
                                "Silent-dropping rate-limited sender {} (hits={}, retry_after={}s)",
                                msg.sender.jid, consecutive_hits, retry_after_secs
                            );
                        }
                        if should_audit {
                            let _ = AuditTracker::record_terminal_event(
                                &self.session_store,
                                msg.id,
                                msg.chat_jid,
                                msg.sender.jid,
                                msg.sender.name,
                                chat_type_str,
                                msg.text,
                                msg.has_media,
                                msg.media_path,
                                "rate_limited",
                                reason_code,
                                if tombstoned { "tombstoned" } else { "throttled" },
                                None,
                                Some("security:rate_limiter"),
                                Some("security_rate_limit"),
                                Some(0.0),
                            )
                            .await;
                        }
                        return Ok(());
                    }
                }

                info!(
                    "Responding to message from {} in {}: {}",
                    msg.sender.jid, msg.chat_jid, reason
                );

                let record_text =
                    format_record_text(&msg.text, msg.has_media, msg.media_path.as_deref());
                self.session_store
                    .record_message(&msg.chat_jid, &msg.sender.jid, &record_text, false)
                    .await?;

                if msg.chat_type == ChatType::Group {
                    let _ = self.session_store.record_group_membership(
                        &msg.chat_jid,
                        &msg.sender.jid,
                        msg.sender.name.as_deref(),
                        Some("member"),
                    ).await;
                }

                let mut audit_tracker = AuditTracker::start_tracking(
                    Arc::clone(&self.session_store),
                    msg.id.clone(),
                    msg.chat_jid.clone(),
                    msg.sender.jid.clone(),
                    msg.sender.name.clone(),
                    chat_type_str,
                    record_text,
                    msg.has_media,
                    msg.media_path.clone(),
                    &reason,
                )
                .await;

                // 1. Intercept local administrative / quick commands
                if let Some(outcome) =
                    try_handle_builtin_command(&msg, &self.session_store, &self.agent_engine, self.persona_engine.admin_jid()).await
                {
                    audit_tracker
                        .complete_handled(
                            &outcome.reply,
                            outcome.tool_name,
                            outcome.usecase,
                            if outcome.is_error { "failed" } else { "success" },
                            outcome.error_detail.as_deref(),
                        )
                        .await;

                    self.session_store
                        .record_message(&msg.chat_jid, &self.bot_jid, &outcome.reply, true)
                        .await?;
                    self.whatsapp
                        .send_text_with_session(
                            &msg.chat_jid,
                            &outcome.reply,
                            Some(&msg.id),
                            msg.session_role,
                        )
                        .await?;
                    return Ok(());
                }

                // 2. Intercept polite conversational closing / gratitude reaction
                if let Some(reaction_emoji) = detect_conversational_closing(&msg.text) {
                    let _ = self
                        .whatsapp
                        .send_reaction_with_session(
                            &msg.chat_jid,
                            &msg.id,
                            reaction_emoji,
                            msg.session_role,
                        )
                        .await;

                    let reaction_note = format!("[Reaksi WhatsApp: {}]", reaction_emoji);
                    audit_tracker
                        .complete_handled(
                            &reaction_note,
                            "builtin:reaction",
                            "casual_and_consultation",
                            "success",
                            None,
                        )
                        .await;

                    self.session_store
                        .record_message(&msg.chat_jid, &self.bot_jid, &reaction_note, true)
                        .await?;
                    return Ok(());
                }

                // 3. Set typing indicator immediately
                let _ = self
                    .whatsapp
                    .send_presence_with_session(
                        &msg.chat_jid,
                        PresenceState::Composing,
                        msg.session_role,
                    )
                    .await;

                // 4. Retrieve active conversation ID (enforcing inactivity window and turn limits to prevent context bloat)
                let session_policy = crate::core::domain::SessionLifecyclePolicy::from_env();
                let existing_conv_id = self
                    .session_store
                    .get_active_conversation_id(
                        &msg.chat_jid,
                        session_policy.inactivity_timeout_secs,
                        session_policy.max_turns,
                    )
                    .await?;

                let quote_id = match msg.chat_type {
                    ChatType::Group => Some(msg.id.as_str()),
                    ChatType::DirectMessage => None,
                };

                // 5. Prepare Cognitive Context (Profiling, Persona, Epistemic Gate, Task Triage)
                let is_resumed = existing_conv_id.is_some();
                let (prompt, capability) = match ContextPreparer::prepare(
                    &msg,
                    &self.persona_engine,
                    &self.session_store,
                    is_resumed,
                )
                .await?
                {
                    ContextPreparation::HaltWithRejection { reject_message } => {
                        self.session_store
                            .record_message(&msg.chat_jid, &self.bot_jid, &reject_message, true)
                            .await?;
                        self.whatsapp
                            .send_text_with_session(
                                &msg.chat_jid,
                                &reject_message,
                                quote_id,
                                msg.session_role,
                            )
                            .await?;
                        return Ok(());
                    }
                    ContextPreparation::Ready { prompt, capability } => {
                        audit_tracker.attach_capability(&capability);
                        (prompt, capability)
                    }
                };

                // Persist the scoped capability so `aina policy` can validate it,
                // register active turn progress session for `aina progress`,
                // and sweep stale rows to keep the table small.
                if let Err(e) = self.session_store.create_turn_capability(&capability).await {
                    warn!("Failed to persist turn capability: {}", e);
                }
                let progress_session = crate::core::domain::TurnProgressSession {
                    turn_token: capability.token.clone(),
                    chat_jid: capability.chat_jid.clone(),
                    chat_type: capability.chat_type.clone(),
                    message_id: capability.message_id.clone(),
                    started_at_epoch: capability.issued_at,
                    last_progress_at_epoch: None,
                    last_progress_text: None,
                    progress_count: 0,
                    is_active: true,
                };
                if let Err(e) = self
                    .session_store
                    .create_turn_progress_session(&progress_session)
                    .await
                {
                    warn!("Failed to persist turn progress session: {}", e);
                }
                let _ = self
                    .session_store
                    .purge_expired_turn_capabilities(chrono::Utc::now().timestamp())
                    .await;

                // 6. Background typing presence heartbeat, agent progress outbox deliverer & dead-man-switch safety net
                let mut heartbeat_guard = start_presence_heartbeat(
                    Arc::clone(&self.whatsapp),
                    Arc::clone(&self.session_store),
                    self.bot_jid.clone(),
                    msg.chat_jid.clone(),
                    capability.token.clone(),
                    msg.session_role,
                    quote_id.map(|s| s.to_string()),
                );

                // 7. Execute Antigravity agent CLI
                let agent_res = match self
                    .agent_engine
                    .execute_with_capability(existing_conv_id.as_deref(), &prompt, Some(&capability))
                    .await
                {
                    Ok(res) => {
                        heartbeat_guard.dismiss();
                        let _ = self
                            .session_store
                            .complete_turn_progress_session(&capability.token)
                            .await;
                        let _ = self
                            .whatsapp
                            .send_presence_with_session(
                                &msg.chat_jid,
                                PresenceState::Paused,
                                msg.session_role,
                            )
                            .await;
                        res
                    }
                    Err(e) => {
                        heartbeat_guard.dismiss();
                        let _ = self
                            .session_store
                            .complete_turn_progress_session(&capability.token)
                            .await;
                        let _ = self
                            .whatsapp
                            .send_presence_with_session(
                                &msg.chat_jid,
                                PresenceState::Paused,
                                msg.session_role,
                            )
                            .await;
                        error!("Agent engine failed to execute for chat {}: {}", msg.chat_jid, e);

                        // Self-healing: if execution failed on an existing resumed session,
                        // evict the session mapping immediately so subsequent messages start fresh without deadlock
                        if existing_conv_id.is_some() {
                            let _ = self.session_store.delete_conversation_id(&msg.chat_jid).await;
                        }

                        // Finalize telemetry failure record
                        audit_tracker
                            .complete_failure(&e.to_string(), existing_conv_id.as_deref())
                            .await;

                        // Delegate failure reporting policy to ErrorHandler
                        ErrorHandler::handle_execution_error(
                            &msg,
                            &e.to_string(),
                            &self.persona_engine,
                            &self.agent_engine,
                            &self.whatsapp,
                            quote_id,
                        )
                        .await;

                        return Err(e);
                    }
                };

                // 8. Update conversation mapping or touch activity
                if existing_conv_id.as_deref() != Some(&agent_res.conversation_id) {
                    self.session_store
                        .save_conversation_id(&msg.chat_jid, &agent_res.conversation_id)
                        .await?;
                } else {
                    let _ = self.session_store.touch_conversation_activity(&msg.chat_jid).await;
                }


                // 9. Deliver response (Multi-bubble WhatsApp delivery & companion media sidecar)
                ResponseDeliverer::deliver_agent_response(
                    &self.whatsapp,
                    &self.session_store,
                    &self.bot_jid,
                    &msg,
                    &agent_res,
                    quote_id,
                )
                .await?;

                // 10. Reset presence
                let _ = self
                    .whatsapp
                    .send_presence_with_session(
                        &msg.chat_jid,
                        PresenceState::Paused,
                        msg.session_role,
                    )
                    .await;

                // 11. Finalize telemetry success record
                audit_tracker
                    .complete_success(&agent_res.conversation_id, &agent_res.response_text)
                    .await;

                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockFailingAgent;

    #[async_trait::async_trait]
    impl AgentEnginePort for MockFailingAgent {
        async fn execute_with_model(
            &self,
            _conversation_id: Option<&str>,
            _prompt: &str,
            _model_override: Option<&str>,
        ) -> anyhow::Result<crate::core::ports::AgentResponse> {
            anyhow::bail!("Antigravity CLI failed: Eligibility check failed: Your current account is not eligible for Antigravity.")
        }
        async fn get_model(&self) -> String {
            "gemini-3.8-flash".to_string()
        }
        async fn set_model(&self, _model: &str) -> anyhow::Result<()> {
            Ok(())
        }
        async fn is_authenticated(&self) -> bool {
            true
        }
        async fn save_auth_token(&self, _token_content: &str) -> anyhow::Result<()> {
            Ok(())
        }
    }

    struct TestRecordingWhatsApp {
        pub sent_messages: Arc<tokio::sync::Mutex<Vec<(String, String)>>>,
    }

    #[async_trait::async_trait]
    impl WhatsAppPort for TestRecordingWhatsApp {
        async fn send_text(&self, to: &str, text: &str, _qid: Option<&str>) -> anyhow::Result<()> {
            self.sent_messages
                .lock()
                .await
                .push((to.to_string(), text.to_string()));
            Ok(())
        }
        async fn send_presence(&self, _to: &str, _state: crate::core::domain::PresenceState) -> anyhow::Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_group_execution_failure_never_spams_group_and_alerts_admin() {
        use crate::adapters::driven::SqliteSessionStore;
        use crate::core::domain::{ChatType, IncomingMessage, Platform, Sender, SessionRole};

        let dir = std::env::temp_dir().join(format!("aina_test_msg_fail_{}", rand::random::<u32>()));
        let db_file = dir.join("msg_fail_test.db");
        let store = Arc::new(SqliteSessionStore::new(&db_file).unwrap());

        let sent = Arc::new(tokio::sync::Mutex::new(Vec::new()));
        let whatsapp = Arc::new(TestRecordingWhatsApp {
            sent_messages: Arc::clone(&sent),
        });

        let persona = Arc::new(PersonaEngine::new(
            "Persona text".to_string(),
            "Org text".to_string(),
            "6289625345646@s.whatsapp.net".to_string(),
            "Asia/Jakarta".to_string(),
            7,
            "id-ID".to_string(),
            "https://aina-wa.dvlpid.my.id".to_string(),
            "628982157341@s.whatsapp.net".to_string(),
            None,
        ));

        let usecase = ProcessIncomingMessageUseCase::new(
            Arc::clone(&store) as _,
            Arc::new(MockFailingAgent),
            Arc::clone(&whatsapp) as _,
            persona,
            "628982157341@s.whatsapp.net".to_string(),
            "Aina".to_string(),
            None,
        );

        // Group message calling @Aina
        let msg = IncomingMessage {
            id: "MSG_FAIL_TEST_01".to_string(),
            platform: Platform::WhatsApp,
            session_role: SessionRole::PrimaryBot,
            chat_jid: "120363377989532476@g.us".to_string(),
            chat_type: ChatType::Group,
            sender: Sender {
                jid: "628111222333@s.whatsapp.net".to_string(),
                name: Some("Rekan Kerja".to_string()),
            },
            text: "@Aina tolong periksa deadline dokumen ini".to_string(),
            timestamp: 1790335800,
            is_from_me: false,
            quoted_message: None,
            mentioned_jids: vec!["628982157341@s.whatsapp.net".to_string()],
            is_bot_mentioned: true,
            bot_lid: None,
            has_media: false,
            media_type: None,
            media_path: None,
        };

        // Execution should fail because agent fails
        let res = usecase.execute(msg).await;
        assert!(res.is_err());

        let msgs = sent.lock().await.clone();
        // WhatsApp group MUST NOT receive error stacktraces
        for (target, _) in &msgs {
            assert_ne!(
                target, "120363377989532476@g.us",
                "Group must NEVER receive raw error stacktraces!"
            );
        }

        // Admin MUST receive the private alert
        let admin_alert = msgs
            .iter()
            .find(|(target, _)| target == "6289625345646@s.whatsapp.net");
        assert!(
            admin_alert.is_some(),
            "Admin must receive the private error notification"
        );
        let (_, alert_text) = admin_alert.unwrap();
        assert!(alert_text.contains("🚨 *Laporan Kegagalan Pemrosesan Pesan Aina*"));
        assert!(alert_text.contains("120363377989532476@g.us"));
        assert!(alert_text.contains("Eligibility check failed"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_admin_dm_execution_failure_receives_alert_directly_in_dm() {
        use crate::adapters::driven::SqliteSessionStore;
        use crate::core::domain::{ChatType, IncomingMessage, Platform, Sender, SessionRole};

        let dir = std::env::temp_dir().join(format!("aina_test_admin_dm_fail_{}", rand::random::<u32>()));
        let db_path = dir.join("test.db");
        let store = Arc::new(SqliteSessionStore::new(&db_path).unwrap());

        let sent = Arc::new(tokio::sync::Mutex::new(Vec::new()));
        let whatsapp = Arc::new(TestRecordingWhatsApp {
            sent_messages: Arc::clone(&sent),
        });

        let persona = Arc::new(PersonaEngine::new(
            "Persona text".to_string(),
            "Org text".to_string(),
            "6289625345646@s.whatsapp.net".to_string(),
            "Asia/Jakarta".to_string(),
            7,
            "id-ID".to_string(),
            "https://aina-wa.dvlpid.my.id".to_string(),
            "628982157341@s.whatsapp.net".to_string(),
            None,
        ));

        let usecase = ProcessIncomingMessageUseCase::new(
            Arc::clone(&store) as _,
            Arc::new(MockFailingAgent),
            Arc::clone(&whatsapp) as _,
            persona,
            "628982157341@s.whatsapp.net".to_string(),
            "Aina".to_string(),
            None,
        );

        // Admin DM message
        let msg = IncomingMessage {
            id: "MSG_FAIL_ADMIN_DM".to_string(),
            platform: Platform::WhatsApp,
            session_role: SessionRole::PrimaryBot,
            chat_jid: "6289625345646@s.whatsapp.net".to_string(),
            chat_type: ChatType::DirectMessage,
            sender: Sender {
                jid: "6289625345646@s.whatsapp.net".to_string(),
                name: Some("Ihza Karunia".to_string()),
            },
            text: "Aina tolong periksa deadline dokumen ini".to_string(),
            timestamp: 1790335800,
            is_from_me: false,
            quoted_message: None,
            mentioned_jids: vec![],
            is_bot_mentioned: true,
            bot_lid: None,
            has_media: false,
            media_type: None,
            media_path: None,
        };

        let res = usecase.execute(msg).await;
        assert!(res.is_err());

        let msgs = sent.lock().await.clone();
        // Admin must receive the failure alert directly in their DM
        let admin_alert = msgs
            .iter()
            .find(|(target, _)| target == "6289625345646@s.whatsapp.net");
        assert!(
            admin_alert.is_some(),
            "Admin must receive the error notification in DM"
        );
        let (_, alert_text) = admin_alert.unwrap();
        assert!(alert_text.contains("🚨 *Laporan Kegagalan Pemrosesan Pesan Aina*"));
        assert!(alert_text.contains("Obrolan Pribadi"));
        assert!(alert_text.contains("Eligibility check failed"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    struct CountingSuccessAgent {
        pub calls: Arc<std::sync::atomic::AtomicUsize>,
    }

    #[async_trait::async_trait]
    impl AgentEnginePort for CountingSuccessAgent {
        async fn execute_with_model(
            &self,
            _conversation_id: Option<&str>,
            _prompt: &str,
            _model_override: Option<&str>,
        ) -> anyhow::Result<crate::core::ports::AgentResponse> {
            let n = self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
            Ok(crate::core::ports::AgentResponse {
                conversation_id: format!("conv_{}", n),
                response_text: format!("Balasan normal #{}", n),
                duration_seconds: 0.01,
            })
        }
        async fn get_model(&self) -> String {
            "gemini-3.8-flash".to_string()
        }
        async fn set_model(&self, _model: &str) -> anyhow::Result<()> {
            Ok(())
        }
        async fn is_authenticated(&self) -> bool {
            true
        }
        async fn save_auth_token(&self, _token_content: &str) -> anyhow::Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_phase19_guest_dm_flood_zero_pool_bleed_and_anti_ban() {
        use crate::adapters::driven::SqliteSessionStore;
        use crate::core::domain::{
            ActionAuditFilter, ChatType, IncomingMessage, Platform, Sender, SessionRole,
        };
        use std::sync::atomic::{AtomicUsize, Ordering};

        let dir = std::env::temp_dir().join(format!(
            "aina_test_phase19_rl_{}",
            rand::random::<u32>()
        ));
        let db_path = dir.join("test.db");
        let store = Arc::new(SqliteSessionStore::new(&db_path).unwrap());

        let agent_calls = Arc::new(AtomicUsize::new(0));
        let agent = Arc::new(CountingSuccessAgent {
            calls: Arc::clone(&agent_calls),
        });

        let sent = Arc::new(tokio::sync::Mutex::new(Vec::new()));
        let whatsapp = Arc::new(TestRecordingWhatsApp {
            sent_messages: Arc::clone(&sent),
        });

        let admin_jid = "6289625345646@s.whatsapp.net";
        let persona = Arc::new(PersonaEngine::new(
            "Persona text".to_string(),
            "Org text".to_string(),
            admin_jid.to_string(),
            "Asia/Jakarta".to_string(),
            7,
            "id-ID".to_string(),
            "http://127.0.0.1:8080".to_string(),
            "628982157341@s.whatsapp.net".to_string(),
            None,
        ));

        let usecase = ProcessIncomingMessageUseCase::new(
            Arc::clone(&store) as _,
            agent,
            Arc::clone(&whatsapp) as _,
            persona,
            "628982157341@s.whatsapp.net".to_string(),
            "Aina".to_string(),
            None,
        )
        .with_rate_limit_config(RateLimitConfig::default());

        let guest_jid = "628777888999@s.whatsapp.net";
        for i in 1..=50 {
            let msg = IncomingMessage {
                id: format!("SPAM_MSG_{}", i),
                platform: Platform::WhatsApp,
                session_role: SessionRole::PrimaryBot,
                chat_jid: guest_jid.to_string(),
                chat_type: ChatType::DirectMessage,
                sender: Sender {
                    jid: guest_jid.to_string(),
                    name: Some("Guest Spammer".to_string()),
                },
                text: format!("Halo tanya dong #{}", i),
                timestamp: 1_790_335_800,
                is_from_me: false,
                quoted_message: None,
                mentioned_jids: vec![],
                is_bot_mentioned: false,
                bot_lid: None,
                has_media: false,
                media_type: None,
                media_path: None,
            };
            usecase.execute(msg).await.unwrap();
        }

        // 1. Zero AccountPool / LLM bleed: max 3 agent calls out of 50 spam DMs
        assert_eq!(
            agent_calls.load(Ordering::SeqCst),
            3,
            "Guest spam burst MUST NOT invoke AgentEngine more than capacity (3)"
        );

        // 2. Anti-Ban invariant: exactly 1 polite cooldown notice sent (NOT 47!)
        let msgs = sent.lock().await.clone();
        let cooldown_notices = msgs
            .iter()
            .filter(|(to, body)| {
                to == guest_jid && body.contains("mohon tunggu sekitar")
            })
            .count();
        assert_eq!(
            cooldown_notices, 1,
            "Anti-ban invariant violated: expected exactly 1 cooldown notice, got {}",
            cooldown_notices
        );
        // 3 normal replies + 1 cooldown notice = 4 total outbound messages
        assert_eq!(msgs.len(), 4);

        // 3. Real SQLite `whatsapp_action_audits` records throttle & tombstone events
        let audits = store
            .query_action_audits(&ActionAuditFilter {
                sender_jid: Some(guest_jid.to_string()),
                decision: Some("rate_limited".to_string()),
                limit: Some(50),
                ..Default::default()
            })
            .await
            .unwrap();
        assert!(
            audits.iter().any(|a| a.decision_reason == "INGRESS_RATE_LIMIT_NOTIFIED"),
            "Expected INGRESS_RATE_LIMIT_NOTIFIED in whatsapp_action_audits"
        );
        assert!(
            audits.iter().any(|a| a.decision_reason == "INGRESS_RATE_LIMIT_TOMBSTONED"),
            "Expected INGRESS_RATE_LIMIT_TOMBSTONED in whatsapp_action_audits"
        );

        // 4. Admin DM completely bypasses rate limiting even during active guest attack
        for i in 1..=10 {
            let admin_msg = IncomingMessage {
                id: format!("ADMIN_MSG_{}", i),
                platform: Platform::WhatsApp,
                session_role: SessionRole::PrimaryBot,
                chat_jid: admin_jid.to_string(),
                chat_type: ChatType::DirectMessage,
                sender: Sender {
                    jid: admin_jid.to_string(),
                    name: Some("Admin".to_string()),
                },
                text: format!("Cek status sistem #{}", i),
                timestamp: 1_790_335_800,
                is_from_me: false,
                quoted_message: None,
                mentioned_jids: vec![],
                is_bot_mentioned: false,
                bot_lid: None,
                has_media: false,
                media_type: None,
                media_path: None,
            };
            usecase.execute(admin_msg).await.unwrap();
        }
        assert_eq!(
            agent_calls.load(Ordering::SeqCst),
            13,
            "Admin DM must completely bypass rate limiting"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
