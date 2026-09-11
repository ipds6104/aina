use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserProfile {
    pub sender_jid: String,
    pub name: Option<String>,
    pub role: Option<String>,
    pub authority_level: String, // "admin", "staff", "guest", "external"
    pub notes: Option<String>,
}

#[async_trait]
pub trait SessionStorePort: Send + Sync {
    /// Retrieves the active Antigravity conversation UUID mapped to the given WhatsApp chat JID.
    async fn get_conversation_id(&self, chat_jid: &str) -> anyhow::Result<Option<String>>;

    /// Saves the mapping of a WhatsApp chat JID to an Antigravity conversation UUID.
    async fn save_conversation_id(&self, chat_jid: &str, conv_uuid: &str) -> anyhow::Result<()>;

    /// Records an incoming or outgoing message for local logging or future recall.
    async fn record_message(
        &self,
        chat_jid: &str,
        sender_jid: &str,
        text: &str,
        is_from_me: bool,
    ) -> anyhow::Result<()>;

    /// Retrieves profiling memory for a sender JID.
    async fn get_user_profile(&self, sender_jid: &str) -> anyhow::Result<Option<UserProfile>>;

    /// Saves or updates profiling memory for a sender JID.
    async fn save_user_profile(&self, profile: &UserProfile) -> anyhow::Result<()>;
}
