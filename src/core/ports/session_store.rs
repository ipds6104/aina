use async_trait::async_trait;

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
}
