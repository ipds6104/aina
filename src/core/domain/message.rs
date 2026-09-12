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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Platform {
    WhatsApp,
    WebSimulator,
    Discord,
    Telegram,
    Custom(String),
}

impl std::fmt::Display for Platform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Platform::WhatsApp => write!(f, "WhatsApp"),
            Platform::WebSimulator => write!(f, "Web Dashboard / Simulator"),
            Platform::Discord => write!(f, "Discord"),
            Platform::Telegram => write!(f, "Telegram"),
            Platform::Custom(s) => write!(f, "{}", s),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionRole {
    PrimaryBot,
    UserCompanion,
}

impl Default for SessionRole {
    fn default() -> Self {
        SessionRole::PrimaryBot
    }
}

impl std::fmt::Display for SessionRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SessionRole::PrimaryBot => write!(f, "PrimaryBot"),
            SessionRole::UserCompanion => write!(f, "UserCompanion"),
        }
    }
}

fn default_platform() -> Platform {
    Platform::WhatsApp
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncomingMessage {
    pub id: String,
    #[serde(default = "default_platform")]
    pub platform: Platform,
    #[serde(default)]
    pub session_role: SessionRole,
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
