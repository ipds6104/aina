use crate::core::domain::{
    Gatekeeper, GatekeeperDecision, IncomingMessage, PersonaEngine, PresenceState,
};
use crate::core::ports::{AgentEnginePort, SessionStorePort, WhatsAppPort};
use std::sync::Arc;
use tracing::{info, warn};

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

                // 2. Send 'typing...' indicator immediately
                if let Err(e) = self
                    .whatsapp
                    .send_presence(&msg.chat_jid, PresenceState::Composing)
                    .await
                {
                    warn!("Failed to send typing presence: {}", e);
                }

                // 3. Retrieve conversation ID for this chat
                let existing_conv_id = self
                    .session_store
                    .get_conversation_id(&msg.chat_jid)
                    .await?;

                // 4. Build prompt incorporating persona and context
                let prompt = self.persona_engine.build_prompt(&msg);

                // 5. Execute Antigravity agent CLI
                let agent_res = match self
                    .agent_engine
                    .execute(existing_conv_id.as_deref(), &prompt)
                    .await
                {
                    Ok(res) => res,
                    Err(e) => {
                        // Reset presence on error
                        let _ = self
                            .whatsapp
                            .send_presence(&msg.chat_jid, PresenceState::Paused)
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
                    .send_text(&msg.chat_jid, &agent_res.response_text, Some(&msg.id))
                    .await?;

                // 9. Reset presence
                let _ = self
                    .whatsapp
                    .send_presence(&msg.chat_jid, PresenceState::Paused)
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
