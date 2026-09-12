use crate::core::domain::{
    Gatekeeper, GatekeeperDecision, IncomingMessage, PersonaEngine, PresenceState,
};
use crate::core::ports::{AgentEnginePort, SessionStorePort, WhatsAppPort};
use std::sync::Arc;
use tracing::{error, info, warn};

pub struct ProcessIncomingMessageUseCase {
    session_store: Arc<dyn SessionStorePort>,
    agent_engine: Arc<dyn AgentEnginePort>,
    whatsapp: Arc<dyn WhatsAppPort>,
    persona_engine: Arc<PersonaEngine>,
    bot_jid: String,
    bot_name: String,
}

impl ProcessIncomingMessageUseCase {
    pub fn new(
        session_store: Arc<dyn SessionStorePort>,
        agent_engine: Arc<dyn AgentEnginePort>,
        whatsapp: Arc<dyn WhatsAppPort>,
        persona_engine: Arc<PersonaEngine>,
        bot_jid: String,
        bot_name: String,
    ) -> Self {
        Self {
            session_store,
            agent_engine,
            whatsapp,
            persona_engine,
            bot_jid,
            bot_name,
        }
    }

    pub async fn execute(&self, msg: IncomingMessage) -> anyhow::Result<()> {
        let decision = Gatekeeper::evaluate(&msg, &self.bot_jid, &self.bot_name);

        match decision {
            GatekeeperDecision::Ignore { reason } => {
                info!("Ignoring message {}: {}", msg.id, reason);
                Ok(())
            }
            GatekeeperDecision::RecordOnly { reason } => {
                info!(
                    "Recording ambient group message from {} in {}: {}",
                    msg.sender.jid, msg.chat_jid, reason
                );
                self.session_store
                    .record_message(&msg.chat_jid, &msg.sender.jid, &msg.text, false)
                    .await?;
                Ok(())
            }
            GatekeeperDecision::Respond { reason } => {
                info!(
                    "Responding to message from {} in {}: {}",
                    msg.sender.jid, msg.chat_jid, reason
                );

                // 1. Record incoming message
                self.session_store
                    .record_message(&msg.chat_jid, &msg.sender.jid, &msg.text, false)
                    .await?;

                // Check for built-in quick command: /reset, /clear, /new
                let trimmed_text = msg.text.trim();
                if trimmed_text.eq_ignore_ascii_case("/reset")
                    || trimmed_text.eq_ignore_ascii_case("/clear")
                    || trimmed_text.eq_ignore_ascii_case("/new")
                    || trimmed_text.eq_ignore_ascii_case("/restart")
                {
                    let _ = self.session_store.delete_conversation_id(&msg.chat_jid).await;
                    let _ = std::fs::remove_file(std::env::temp_dir().join("aina_gh_device_session.json"));
                    let reply = "🔄 *Sesi Percakapan Berhasil Direset*\n\nMemori konteks percakapan untuk ruang obrolan ini telah dibersihkan. Sesi berikutnya akan dimulai sebagai percakapan baru yang segar. Silakan ajukan pertanyaan atau instruksi baru Anda!".to_string();
                    self.session_store.record_message(&msg.chat_jid, &self.bot_jid, &reply, true).await?;
                    self.whatsapp.send_text_with_session(&msg.chat_jid, &reply, Some(&msg.id), msg.session_role).await?;
                    return Ok(());
                }

                // Check for built-in quick command: /model
                if trimmed_text.starts_with("/model") {
                    let parts: Vec<&str> = trimmed_text.split_whitespace().collect();
                    if parts.len() == 1 || (parts.len() >= 2 && (parts[1] == "status" || parts[1] == "list")) {
                        let current = self.agent_engine.get_model().await;
                        let reply = format!(
                            "🤖 *Status Model AI Aina*\n\nModel aktif saat ini: *{}*\n\n*Pilihan Model Tersedia:*\n• `gemini-3.8-flash-medium` (Default Cepat & Seimbang)\n• `gemini-3.8-flash-high` (Penalaran Tinggi / Deep Thinking)\n• `gemini-3.8-flash-low` (Respons Kilat & Kasual)\n• `gemini-3.1-pro-high` (Deep Coding & Arsitektur)\n• `claude-opus-4-6-thinking` (Claude Opus Thinking - Khusus Eksplisit)\n• `claude-sonnet-4-6` (Claude Sonnet 4.6)\n\n_Untuk mengganti model, ketik:_ `/model <nama_model>`",
                            current
                        );
                        self.session_store.record_message(&msg.chat_jid, &self.bot_jid, &reply, true).await?;
                        self.whatsapp.send_text_with_session(&msg.chat_jid, &reply, Some(&msg.id), msg.session_role).await?;
                        return Ok(());
                    } else if parts.len() >= 2 {
                        let target_model = parts[1];
                        match self.agent_engine.set_model(target_model).await {
                            Ok(_) => {
                                let new_model = self.agent_engine.get_model().await;
                                let reply = format!(
                                    "✅ *Model AI Berhasil Diubah*\n\nAina sekarang menggunakan model: *{}*.\nRespons berikutnya akan diproses menggunakan mesin ini.",
                                    new_model
                                );
                                self.session_store.record_message(&msg.chat_jid, &self.bot_jid, &reply, true).await?;
                                self.whatsapp.send_text_with_session(&msg.chat_jid, &reply, Some(&msg.id), msg.session_role).await?;
                                return Ok(());
                            }
                            Err(e) => {
                                let reply = format!(
                                    "⚠️ *Gagal Mengganti Model*\n\n{}\n\nContoh: `/model gemini-3.8-flash-medium`",
                                    e
                                );
                                self.session_store.record_message(&msg.chat_jid, &self.bot_jid, &reply, true).await?;
                                self.whatsapp.send_text_with_session(&msg.chat_jid, &reply, Some(&msg.id), msg.session_role).await?;
                                return Ok(());
                            }
                        }
                    }
                }

                // 2. Send 'typing...' indicator immediately
                if let Err(e) = self
                    .whatsapp
                    .send_presence_with_session(&msg.chat_jid, PresenceState::Composing, msg.session_role)
                    .await
                {
                    warn!("Failed to send typing presence: {}", e);
                }

                // 3. Retrieve conversation ID for this chat
                let existing_conv_id = self
                    .session_store
                    .get_conversation_id(&msg.chat_jid)
                    .await?;

                // 4. Retrieve or auto-seed sender profile from profiling memory
                let profile = match self.session_store.get_user_profile(&msg.sender.jid).await {
                    Ok(Some(p)) => Some(p),
                    Ok(None) => {
                        let new_profile = crate::core::ports::UserProfile {
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

                // 5. Build prompt incorporating persona, organization context, and profiling
                let prompt = self.persona_engine.build_prompt(&msg, profile.as_ref());

                // Start async presence heartbeat + fast ack timer if execution takes long
                let whatsapp = Arc::clone(&self.whatsapp);
                let chat_jid = msg.chat_jid.clone();
                let msg_id = msg.id.clone();
                let session_role = msg.session_role;

                let heartbeat_handle = tokio::spawn(async move {
                    let mut elapsed_secs = 0;
                    let mut ack_sent = false;
                    loop {
                        tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;
                        elapsed_secs += 4;

                        // Keep typing presence alive on WhatsApp
                        let _ = whatsapp
                            .send_presence_with_session(&chat_jid, PresenceState::Composing, session_role)
                            .await;

                        // If task takes longer than 6 seconds, send a friendly interim ack once
                        if elapsed_secs >= 6 && !ack_sent {
                            let ack_text = "okee sebentarr...".to_string();
                            let _ = whatsapp
                                .send_text_with_session(&chat_jid, &ack_text, Some(&msg_id), session_role)
                                .await;
                            ack_sent = true;
                        }
                    }
                });

                // 6. Execute Antigravity agent CLI
                let agent_res = match self
                    .agent_engine
                    .execute(existing_conv_id.as_deref(), &prompt)
                    .await
                {
                    Ok(res) => {
                        heartbeat_handle.abort();
                        res
                    }
                    Err(e) => {
                        heartbeat_handle.abort();
                        error!("Agent engine failed to execute: {}", e);
                        let err_reply = "Maaf, terjadi kesalahan saat memproses permintaan Anda. Silakan coba sesaat lagi.";
                        let _ = self
                            .whatsapp
                            .send_text_with_session(&msg.chat_jid, err_reply, Some(&msg.id), msg.session_role)
                            .await;
                        return Err(e);
                    }
                };

                // 6. Save or update conversation mapping
                if existing_conv_id.as_deref() != Some(&agent_res.conversation_id) {
                    self.session_store
                        .save_conversation_id(&msg.chat_jid, &agent_res.conversation_id)
                        .await?;
                }

                // 7. Record bot reply locally
                self.session_store
                    .record_message(&msg.chat_jid, &self.bot_jid, &agent_res.response_text, true)
                    .await?;

                // 8. Send reply back to WhatsApp
                self.whatsapp
                    .send_text_with_session(&msg.chat_jid, &agent_res.response_text, Some(&msg.id), msg.session_role)
                    .await?;

                // 9. Reset presence
                let _ = self
                    .whatsapp
                    .send_presence_with_session(&msg.chat_jid, PresenceState::Paused, msg.session_role)
                    .await;

                info!(
                    "Successfully replied to {} in {:.2}s",
                    msg.chat_jid, agent_res.duration_seconds
                );
                Ok(())
            }
        }
    }
}
