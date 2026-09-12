use crate::core::domain::{PresenceState, SessionRole};
use async_trait::async_trait;

#[async_trait]
pub trait WhatsAppPort: Send + Sync {
    /// Sends a text message to a WhatsApp user or group.
    async fn send_text(
        &self,
        to_jid: &str,
        text: &str,
        quoted_id: Option<&str>,
    ) -> anyhow::Result<()>;

    /// Sends a text message routed via a specific SessionRole (PrimaryBot or UserCompanion).
    async fn send_text_with_session(
        &self,
        to_jid: &str,
        text: &str,
        quoted_id: Option<&str>,
        session_role: SessionRole,
    ) -> anyhow::Result<()> {
        let _ = session_role;
        self.send_text(to_jid, text, quoted_id).await
    }

    /// Updates the typing presence state (e.g., composing, paused).
    async fn send_presence(&self, to_jid: &str, state: PresenceState) -> anyhow::Result<()>;

    /// Updates typing presence with explicit session routing.
    async fn send_presence_with_session(
        &self,
        to_jid: &str,
        state: PresenceState,
        session_role: SessionRole,
    ) -> anyhow::Result<()> {
        let _ = session_role;
        self.send_presence(to_jid, state).await
    }
}
