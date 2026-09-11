use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChatType {
    DirectMessage,
    Group,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sender {
    pub jid: String,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotedMessage {
    pub id: String,
    pub sender_jid: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncomingMessage {
    pub id: String,
    pub chat_jid: String,
    pub chat_type: ChatType,
    pub sender: Sender,
    pub text: String,
    pub timestamp: i64,
    pub is_from_me: bool,
    pub quoted_message: Option<QuotedMessage>,
    pub mentioned_jids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GatekeeperDecision {
    Respond { reason: String },
    Ignore { reason: String },
    RecordOnly { reason: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PresenceState {
    Composing,
    Paused,
}
