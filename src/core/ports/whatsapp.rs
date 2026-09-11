use crate::core::domain::PresenceState;
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

    /// Updates the typing presence state (e.g., composing, paused).
    async fn send_presence(&self, to_jid: &str, state: PresenceState) -> anyhow::Result<()>;
}
