//! Core orchestrator use case for processing incoming WhatsApp messages.

mod audit_tracker;
mod builtin_commands;
mod context_preparer;
mod error_handler;
mod heartbeat;
mod response_deliverer;

pub use audit_tracker::AuditTracker;
pub use builtin_commands::try_handle_builtin_command;
pub use context_preparer::{ContextPreparation, ContextPreparer};
pub use error_handler::ErrorHandler;
pub use heartbeat::start_presence_heartbeat;
pub use response_deliverer::ResponseDeliverer;

// Re-export response formatting & conversational heuristics from domain layer for backward compatibility
#[allow(unused_imports)]
pub use crate::core::domain::{detect_conversational_closing, split_response_into_bubbles};

use crate::core::domain::{ChatType, Gatekeeper, GatekeeperDecision, IncomingMessage, PersonaEngine, PresenceState};
use crate::core::ports::{AgentEnginePort, SessionStorePort, WhatsAppPort};
use std::sync::Arc;
use tracing::{error, info};

pub struct ProcessIncomingMessageUseCase {
    session_store: Arc<dyn SessionStorePort>,
    agent_engine: Arc<dyn AgentEnginePort>,
    whatsapp: Arc<dyn WhatsAppPort>,
    persona_engine: Arc<PersonaEngine>,
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
            bot_jid,
            bot_name,
            bot_lid,
        }
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
                info!(
                    "Responding to message from {} in {}: {}",
                    msg.sender.jid, msg.chat_jid, reason
                );

                let record_text =
                    format_record_text(&msg.text, msg.has_media, msg.media_path.as_deref());
                self.session_store
                    .record_message(&msg.chat_jid, &msg.sender.jid, &record_text, false)
                    .await?;

                let audit_tracker = AuditTracker::start_tracking(
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
                    try_handle_builtin_command(&msg, &self.session_store, &self.agent_engine).await
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

                // 4. Retrieve existing conversation ID for this chat
                let existing_conv_id = self
                    .session_store
                    .get_conversation_id(&msg.chat_jid)
                    .await?;

                let quote_id = match msg.chat_type {
                    ChatType::Group => Some(msg.id.as_str()),
                    ChatType::DirectMessage => None,
                };

                // 5. Prepare Cognitive Context (Profiling, Persona, Epistemic Gate, Task Triage)
                let prompt = match ContextPreparer::prepare(
                    &msg,
                    &self.persona_engine,
                    &self.session_store,
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
                    ContextPreparation::Ready { prompt } => prompt,
                };

                // 6. Background typing presence heartbeat + progressive reassurance
                let mut heartbeat_guard = start_presence_heartbeat(
                    Arc::clone(&self.whatsapp),
                    msg.chat_jid.clone(),
                    msg.session_role,
                    quote_id.map(|s| s.to_string()),
                );

                // 7. Execute Antigravity agent CLI
                let agent_res = match self
                    .agent_engine
                    .execute(existing_conv_id.as_deref(), &prompt)
                    .await
                {
                    Ok(res) => {
                        heartbeat_guard.dismiss();
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
                            .whatsapp
                            .send_presence_with_session(
                                &msg.chat_jid,
                                PresenceState::Paused,
                                msg.session_role,
                            )
                            .await;
                        error!("Agent engine failed to execute for chat {}: {}", msg.chat_jid, e);

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

                // 8. Update conversation mapping
                if existing_conv_id.as_deref() != Some(&agent_res.conversation_id) {
                    self.session_store
                        .save_conversation_id(&msg.chat_jid, &agent_res.conversation_id)
                        .await?;
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
}
