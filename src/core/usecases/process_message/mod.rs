//! Core usecase for processing incoming messages from WhatsApp.

mod builtin_commands;
mod error_handler;
mod heartbeat;

pub use builtin_commands::try_handle_builtin_command;
pub use error_handler::ErrorHandler;
pub use heartbeat::start_presence_heartbeat;

// Re-export response formatting & conversational heuristics from domain layer for backward compatibility
pub use crate::core::domain::{detect_conversational_closing, split_response_into_bubbles};

use crate::core::domain::{
    AuditEngine, ChatType, Gatekeeper, GatekeeperDecision, IncomingMessage, NewWhatsAppActionAudit,
    PersonaEngine, PresenceState, UseCaseClassifier,
};
use crate::core::domain::metacognition::{AgentCapabilityManifest, AntiConfabGate, TaskTriageEngine, TriageDecision};
use crate::core::ports::{AgentEnginePort, SessionStorePort, UserProfile, WhatsAppPort};
use std::sync::Arc;
use tracing::{error, info, warn};

pub struct ProcessIncomingMessageUseCase {
    session_store: Arc<dyn SessionStorePort>,
    agent_engine: Arc<dyn AgentEnginePort>,
    whatsapp: Arc<dyn WhatsAppPort>,
    persona_engine: Arc<PersonaEngine>,
    bot_jid: String,
    bot_name: String,
    bot_lid: Option<String>,
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
        let now_epoch = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

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
                let initial_usecase = UseCaseClassifier::classify(
                    &msg.text,
                    msg.has_media,
                    msg.media_path.as_deref(),
                    &[],
                );
                let audit = NewWhatsAppActionAudit {
                    message_id: msg.id.clone(),
                    chat_jid: msg.chat_jid.clone(),
                    chat_type: chat_type_str.to_string(),
                    sender_jid: msg.sender.jid.clone(),
                    sender_name: msg.sender.name.clone(),
                    decision: "ignore".to_string(),
                    decision_reason: reason.clone(),
                    conversation_id: None,
                    status: "ignored".to_string(),
                    input_text: msg.text.clone(),
                    has_media: msg.has_media,
                    media_path: msg.media_path.clone(),
                    response_text: None,
                    error_message: None,
                    duration_seconds: Some(0.0),
                    tools_invoked: vec![],
                    usecase: initial_usecase.as_str().to_string(),
                    created_at_epoch: now_epoch,
                    completed_at_epoch: Some(now_epoch),
                };
                let _ = self.session_store.record_action_audit(&audit).await;
                Ok(())
            }
            GatekeeperDecision::RecordOnly { reason } => {
                info!(
                    "Recording ambient group message from {} in {}: {}",
                    msg.sender.jid, msg.chat_jid, reason
                );
                let record_text = if msg.has_media {
                    if let Some(ref path) = msg.media_path {
                        format!("{} [Media: {}]", msg.text, path)
                    } else {
                        format!("{} [Media]", msg.text)
                    }
                } else {
                    msg.text.clone()
                };
                self.session_store
                    .record_message(&msg.chat_jid, &msg.sender.jid, &record_text, false)
                    .await?;

                let initial_usecase = UseCaseClassifier::classify(
                    &record_text,
                    msg.has_media,
                    msg.media_path.as_deref(),
                    &[],
                );
                let audit = NewWhatsAppActionAudit {
                    message_id: msg.id.clone(),
                    chat_jid: msg.chat_jid.clone(),
                    chat_type: chat_type_str.to_string(),
                    sender_jid: msg.sender.jid.clone(),
                    sender_name: msg.sender.name.clone(),
                    decision: "record_only".to_string(),
                    decision_reason: reason.clone(),
                    conversation_id: None,
                    status: "recorded".to_string(),
                    input_text: record_text,
                    has_media: msg.has_media,
                    media_path: msg.media_path.clone(),
                    response_text: None,
                    error_message: None,
                    duration_seconds: Some(0.0),
                    tools_invoked: vec![],
                    usecase: initial_usecase.as_str().to_string(),
                    created_at_epoch: now_epoch,
                    completed_at_epoch: Some(now_epoch),
                };
                let _ = self.session_store.record_action_audit(&audit).await;
                Ok(())
            }
            GatekeeperDecision::Respond { reason } => {
                info!(
                    "Responding to message from {} in {}: {}",
                    msg.sender.jid, msg.chat_jid, reason
                );

                // 1. Record incoming message
                let record_text = if msg.has_media {
                    if let Some(ref path) = msg.media_path {
                        format!("{} [Media: {}]", msg.text, path)
                    } else {
                        format!("{} [Media]", msg.text)
                    }
                } else {
                    msg.text.clone()
                };
                self.session_store
                    .record_message(&msg.chat_jid, &msg.sender.jid, &record_text, false)
                    .await?;

                let initial_usecase = UseCaseClassifier::classify(
                    &record_text,
                    msg.has_media,
                    msg.media_path.as_deref(),
                    &[],
                );
                let audit = NewWhatsAppActionAudit {
                    message_id: msg.id.clone(),
                    chat_jid: msg.chat_jid.clone(),
                    chat_type: chat_type_str.to_string(),
                    sender_jid: msg.sender.jid.clone(),
                    sender_name: msg.sender.name.clone(),
                    decision: "respond".to_string(),
                    decision_reason: reason.clone(),
                    conversation_id: None,
                    status: "in_progress".to_string(),
                    input_text: record_text,
                    has_media: msg.has_media,
                    media_path: msg.media_path.clone(),
                    response_text: None,
                    error_message: None,
                    duration_seconds: None,
                    tools_invoked: vec![],
                    usecase: initial_usecase.as_str().to_string(),
                    created_at_epoch: now_epoch,
                    completed_at_epoch: None,
                };
                let audit_id = self.session_store.record_action_audit(&audit).await.ok();
                let start_instant = std::time::Instant::now();

                // 2. Intercept local built-in quick commands (/reset, /model, /token, media gate)
                if let Some(outcome) =
                    try_handle_builtin_command(&msg, &self.session_store, &self.agent_engine).await
                {
                    if let Some(aid) = audit_id {
                        let dur = start_instant.elapsed().as_secs_f64();
                        let status_str = if outcome.is_error { "failed" } else { "success" };
                        let _ = self
                            .session_store
                            .update_action_audit_result(
                                aid,
                                None,
                                Some(&outcome.reply),
                                outcome.error_detail.as_deref(),
                                status_str,
                                Some(dur),
                                &[outcome.tool_name.to_string()],
                                Some(outcome.usecase),
                            )
                            .await;
                    }
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

                // 3. Check for polite conversational closing / acknowledgment / gratitude
                if let Some(reaction_emoji) = detect_conversational_closing(&msg.text) {
                    info!(
                        "Detected conversational closing in message '{}' from {}, responding with reaction {}",
                        msg.text, msg.sender.jid, reaction_emoji
                    );
                    if let Err(e) = self
                        .whatsapp
                        .send_reaction_with_session(
                            &msg.chat_jid,
                            &msg.id,
                            reaction_emoji,
                            msg.session_role,
                        )
                        .await
                    {
                        warn!("Failed to send reaction {} to {}: {}", reaction_emoji, msg.chat_jid, e);
                    }
                    let reaction_note = format!("[Reaksi WhatsApp: {}]", reaction_emoji);
                    if let Some(aid) = audit_id {
                        let dur = start_instant.elapsed().as_secs_f64();
                        let _ = self
                            .session_store
                            .update_action_audit_result(
                                aid,
                                None,
                                Some(&reaction_note),
                                None,
                                "success",
                                Some(dur),
                                &["builtin:reaction".to_string()],
                                Some("casual_and_consultation"),
                            )
                            .await;
                    }
                    let _ = self
                        .session_store
                        .record_message(&msg.chat_jid, &self.bot_jid, &reaction_note, true)
                        .await;
                    return Ok(());
                }

                // 4. Send typing indicator immediately
                if let Err(e) = self
                    .whatsapp
                    .send_presence_with_session(&msg.chat_jid, PresenceState::Composing, msg.session_role)
                    .await
                {
                    warn!("Failed to send typing presence: {}", e);
                }

                // 5. Retrieve existing conversation ID for this chat
                let existing_conv_id = self
                    .session_store
                    .get_conversation_id(&msg.chat_jid)
                    .await?;

                // 6. Retrieve or auto-seed sender profile
                let profile = match self.session_store.get_user_profile(&msg.sender.jid).await {
                    Ok(Some(p)) => Some(p),
                    Ok(None) => {
                        let new_profile = UserProfile {
                            sender_jid: msg.sender.jid.clone(),
                            name: msg.sender.name.clone(),
                            role: Some("Rekan Kerja".to_string()),
                            authority_level: "staff".to_string(),
                            notes: Some("Terdaftar otomatis saat interaksi pertama".to_string()),
                        };
                        let _ = self.session_store.save_user_profile(&new_profile).await;
                        Some(new_profile)
                    }
                    Err(e) => {
                        warn!("Failed to fetch user profile for {}: {}", msg.sender.jid, e);
                        None
                    }
                };

                // 7. Build prompt incorporating persona, organization context, and profiling
                let mut prompt = self.persona_engine.build_prompt(&msg, profile.as_ref());

                // Epistemic Vigilance & Anti-Confabulation Gate
                let resolution = AntiConfabGate::evaluate_discrepancy(
                    "WhatsApp Story media upload with caption field",
                    &msg.text,
                );
                if let Some(guardrail) = AntiConfabGate::format_epistemic_guardrail(&resolution) {
                    prompt.push_str("\n\n---\n");
                    prompt.push_str(&guardrail);
                }

                // Task Triage for novel/held-out tasks
                let manifest = AgentCapabilityManifest::default_manifest();
                let triage = TaskTriageEngine::triage_task(&msg.text, &manifest);
                match triage {
                    TriageDecision::ElegantRejection {
                        reason,
                        missing_capabilities,
                    } => {
                        info!(
                            "Task triage rejected novel impossible task for chat {}: {}",
                            msg.chat_jid, reason
                        );
                        let reject_msg = format!(
                            "Aina belum bisa menjalankan permintaan ini yaa 🙏\n\n*Alasan:* {}\n*Batasan Teknis:* Kapabilitas {} belum tersedia di lingkungan saat ini.",
                            reason,
                            missing_capabilities.join(", ")
                        );
                        let _ = self
                            .session_store
                            .record_message(&msg.chat_jid, &self.bot_jid, &reject_msg, true)
                            .await;
                        let quote_id = match msg.chat_type {
                            ChatType::Group => Some(msg.id.as_str()),
                            ChatType::DirectMessage => None,
                        };
                        let _ = self
                            .whatsapp
                            .send_text_with_session(&msg.chat_jid, &reject_msg, quote_id, msg.session_role)
                            .await;
                        return Ok(());
                    }
                    TriageDecision::GracefulDegradation {
                        suggested_alternative,
                        reason,
                        ..
                    } => {
                        prompt.push_str(&format!(
                            "\n\n---\n[CATATAN TRIAGE KAPABILITAS]: Tugas ini melampaui kemampuan native ({}). Alihkan atau tawarkan alternatif elegan: {}.",
                            reason, suggested_alternative
                        ));
                    }
                    _ => {}
                }

                let quote_id = match msg.chat_type {
                    ChatType::Group => Some(msg.id.as_str()),
                    ChatType::DirectMessage => None,
                };

                // 8. Start background typing presence heartbeat + progressive reassurance
                let mut heartbeat_guard = start_presence_heartbeat(
                    Arc::clone(&self.whatsapp),
                    msg.chat_jid.clone(),
                    msg.session_role,
                    quote_id.map(|s| s.to_string()),
                );

                // 9. Execute Antigravity agent CLI
                let agent_res = match self
                    .agent_engine
                    .execute(existing_conv_id.as_deref(), &prompt)
                    .await
                {
                    Ok(res) => {
                        heartbeat_guard.dismiss();
                        let _ = self
                            .whatsapp
                            .send_presence_with_session(&msg.chat_jid, PresenceState::Paused, msg.session_role)
                            .await;
                        res
                    }
                    Err(e) => {
                        heartbeat_guard.dismiss();
                        let _ = self
                            .whatsapp
                            .send_presence_with_session(&msg.chat_jid, PresenceState::Paused, msg.session_role)
                            .await;
                        error!("Agent engine failed to execute for chat {}: {}", msg.chat_jid, e);

                        if let Some(aid) = audit_id {
                            let dur = start_instant.elapsed().as_secs_f64();
                            let partial_tools = if let Some(ref cid) = existing_conv_id {
                                AuditEngine::extract_tools_for_conversation(
                                    &AuditEngine::default_brain_path(),
                                    cid,
                                )
                            } else {
                                vec![]
                            };
                            let refined_usecase = UseCaseClassifier::classify(
                                &msg.text,
                                msg.has_media,
                                msg.media_path.as_deref(),
                                &partial_tools,
                            );
                            let _ = self
                                .session_store
                                .update_action_audit_result(
                                    aid,
                                    None,
                                    None,
                                    Some(&e.to_string()),
                                    "failed",
                                    Some(dur),
                                    &partial_tools,
                                    Some(refined_usecase.as_str()),
                                )
                                .await;
                        }

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

                // 10. Save or update conversation mapping
                if existing_conv_id.as_deref() != Some(&agent_res.conversation_id) {
                    self.session_store
                        .save_conversation_id(&msg.chat_jid, &agent_res.conversation_id)
                        .await?;
                }

                // 11. Check if agent mistakenly executed wa_tool.py send-text AND outputted an internal report
                let trimmed_res = agent_res.response_text.trim();
                let is_redundant_report = trimmed_res.starts_with("Pesan balasan sudah terkirim")
                    || trimmed_res.starts_with("Pesan tanggapan telah berhasil dikirim")
                    || trimmed_res.starts_with("Pesan telah berhasil dikirim")
                    || trimmed_res.starts_with("Pesan berhasil dikirim");

                if is_redundant_report {
                    info!(
                        "Suppressed redundant agent tool confirmation report to prevent double-posting: {}",
                        trimmed_res
                    );
                } else {
                    // Record bot reply locally
                    self.session_store
                        .record_message(&msg.chat_jid, &self.bot_jid, &agent_res.response_text, true)
                        .await?;

                    // Send reply back to WhatsApp (supports multi-bubble splitting)
                    let bubbles = split_response_into_bubbles(&agent_res.response_text);
                    for (i, bubble) in bubbles.iter().enumerate() {
                        let quote = if i == 0 { quote_id } else { None };
                        if i > 0 {
                            let _ = self
                                .whatsapp
                                .send_presence_with_session(
                                    &msg.chat_jid,
                                    PresenceState::Composing,
                                    msg.session_role,
                                )
                                .await;
                            tokio::time::sleep(tokio::time::Duration::from_millis(600)).await;
                        }

                        self.whatsapp
                            .send_text_with_session(&msg.chat_jid, bubble, quote, msg.session_role)
                            .await?;
                    }
                }

                // 12. If message had media, append AI response analysis to the companion .txt transcript sidecar
                if let Some(ref media_path) = msg.media_path {
                    let sidecar_txt_path = std::path::Path::new(media_path).with_extension("txt");
                    if sidecar_txt_path.exists() {
                        use tokio::io::AsyncWriteExt;
                        let append_content =
                            format!("\n--- Analisis & Respon Aina ---\n{}\n", agent_res.response_text);
                        if let Ok(mut file) = tokio::fs::OpenOptions::new()
                            .append(true)
                            .open(&sidecar_txt_path)
                            .await
                        {
                            let _ = file.write_all(append_content.as_bytes()).await;
                            info!("Appended AI response transcript to sidecar {:?}", sidecar_txt_path);
                        }
                    }
                }

                // 13. Reset presence
                let _ = self
                    .whatsapp
                    .send_presence_with_session(&msg.chat_jid, PresenceState::Paused, msg.session_role)
                    .await;

                let actual_duration = start_instant.elapsed().as_secs_f64();
                info!(
                    "Successfully replied to {} in {:.2}s",
                    msg.chat_jid, actual_duration
                );

                // 14. Record audit completion
                let tools_invoked = AuditEngine::extract_tools_for_conversation(
                    &AuditEngine::default_brain_path(),
                    &agent_res.conversation_id,
                );
                let refined_usecase = UseCaseClassifier::classify(
                    &msg.text,
                    msg.has_media,
                    msg.media_path.as_deref(),
                    &tools_invoked,
                );
                if let Some(aid) = audit_id {
                    let _ = self
                        .session_store
                        .update_action_audit_result(
                            aid,
                            Some(&agent_res.conversation_id),
                            Some(&agent_res.response_text),
                            None,
                            "success",
                            Some(actual_duration),
                            &tools_invoked,
                            Some(refined_usecase.as_str()),
                        )
                        .await;
                }

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
        // Crucial assertions:
        // 1. WhatsApp group MUST NOT receive error stacktraces
        for (target, _) in &msgs {
            assert_ne!(
                target, "120363377989532476@g.us",
                "Group must NEVER receive raw error stacktraces!"
            );
        }

        // 2. Admin MUST receive the private alert
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
