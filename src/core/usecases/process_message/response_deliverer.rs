//! Outbound message delivery pipeline: redundant report suppression, multi-bubble delivery, and sidecar transcript appending.

use crate::core::domain::{split_response_into_bubbles, IncomingMessage, PresenceState};
use crate::core::ports::{AgentResponse, SessionStorePort, WhatsAppPort};
use std::sync::Arc;
use tracing::info;

pub struct ResponseDeliverer;

impl ResponseDeliverer {
    /// Determines whether the agent mistakenly outputted an internal confirmation report after sending a text.
    pub fn is_redundant_report(response_text: &str) -> bool {
        let trimmed = response_text.trim();
        trimmed.starts_with("Pesan balasan sudah terkirim")
            || trimmed.starts_with("Pesan tanggapan telah berhasil dikirim")
            || trimmed.starts_with("Pesan telah berhasil dikirim")
            || trimmed.starts_with("Pesan berhasil dikirim")
    }

    /// Delivers the agent response back to WhatsApp and updates message history and companion media sidecars.
    pub async fn deliver_agent_response(
        whatsapp: &Arc<dyn WhatsAppPort>,
        session_store: &Arc<dyn SessionStorePort>,
        bot_jid: &str,
        msg: &IncomingMessage,
        agent_res: &AgentResponse,
        quote_id: Option<&str>,
    ) -> anyhow::Result<()> {
        if Self::is_redundant_report(&agent_res.response_text) {
            info!(
                "Suppressed redundant agent tool confirmation report to prevent double-posting: {}",
                agent_res.response_text.trim()
            );
            return Ok(());
        }

        // 1. Record bot reply in session store
        session_store
            .record_message(&msg.chat_jid, bot_jid, &agent_res.response_text, true)
            .await?;

        // 2. Deliver via multi-bubble WhatsApp messages
        let bubbles = split_response_into_bubbles(&agent_res.response_text);
        for (i, bubble) in bubbles.iter().enumerate() {
            let quote = if i == 0 { quote_id } else { None };
            if i > 0 {
                let _ = whatsapp
                    .send_presence_with_session(
                        &msg.chat_jid,
                        PresenceState::Composing,
                        msg.session_role,
                    )
                    .await;
                tokio::time::sleep(tokio::time::Duration::from_millis(600)).await;
            }

            whatsapp
                .send_text_with_session(&msg.chat_jid, bubble, quote, msg.session_role)
                .await?;
        }

        // 3. Append AI response transcript to companion media sidecar .txt if present
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

        Ok(())
    }
}
