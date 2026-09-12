#![allow(dead_code)]

use super::message::Platform;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SensorRole {
    InteractiveBot,
    PassiveSensor,
    MirrorBackup,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrivacyTier {
    StrictPrivate,
    TeamAmbient,
    PublicDomain,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestionSourceSpec {
    pub id: String,
    pub platform: Platform,
    pub transport_id: String,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default = "default_sensor_role")]
    pub role: SensorRole,
    #[serde(default = "default_workspace")]
    pub default_workspace: String,
    #[serde(default = "default_privacy_tier")]
    pub privacy_tier: PrivacyTier,
}

fn default_sensor_role() -> SensorRole {
    SensorRole::InteractiveBot
}

fn default_workspace() -> String {
    "default".to_string()
}

fn default_privacy_tier() -> PrivacyTier {
    PrivacyTier::PublicDomain
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttachmentMetadata {
    pub filename: String,
    pub mime_type: String,
    pub size_bytes: u64,
    #[serde(default)]
    pub local_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawContextItem {
    pub source_id: String,
    pub platform: Platform,
    pub origin_channel: String,
    pub author_id: String,
    #[serde(default)]
    pub author_name: Option<String>,
    pub content: String,
    pub timestamp: i64,
    pub is_from_owner: bool,
    #[serde(default)]
    pub attachments: Vec<AttachmentMetadata>,
    #[serde(default)]
    pub raw_metadata: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum IngestionOutcome {
    Ignored { reason: String },
    Archived { workspace: String, record_id: Option<i64> },
    ActionRequired { workspace: String, prompt: String, reply_channel: String },
}

impl IngestionSourceSpec {
    /// Evaluates if an incoming context item should be ingested or ignored based on its privacy tier
    pub fn evaluate_privacy(&self, item: &RawContextItem) -> bool {
        match self.privacy_tier {
            PrivacyTier::PublicDomain => true,
            PrivacyTier::TeamAmbient => {
                let is_group = item.origin_channel.ends_with("@g.us") 
                    || item.origin_channel.starts_with("group:") 
                    || item.origin_channel.contains("channel");
                is_group || item.is_from_owner
            }
            PrivacyTier::StrictPrivate => {
                item.is_from_owner
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_privacy_evaluation() {
        let spec = IngestionSourceSpec {
            id: "companion_1".to_string(),
            platform: Platform::WhatsApp,
            transport_id: "628111@s.whatsapp.net".to_string(),
            session_id: Some("companion".to_string()),
            role: SensorRole::PassiveSensor,
            default_workspace: "engineering".to_string(),
            privacy_tier: PrivacyTier::TeamAmbient,
        };

        let group_item = RawContextItem {
            source_id: "companion_1".to_string(),
            platform: Platform::WhatsApp,
            origin_channel: "120363@g.us".to_string(),
            author_id: "628999@s.whatsapp.net".to_string(),
            author_name: Some("Rekan".to_string()),
            content: "Meeting jam 10 ya".to_string(),
            timestamp: 1726000000,
            is_from_owner: false,
            attachments: vec![],
            raw_metadata: serde_json::Value::Null,
        };
        assert!(spec.evaluate_privacy(&group_item));

        let dm_item = RawContextItem {
            source_id: "companion_1".to_string(),
            platform: Platform::WhatsApp,
            origin_channel: "628999@s.whatsapp.net".to_string(),
            author_id: "628999@s.whatsapp.net".to_string(),
            author_name: Some("Teman".to_string()),
            content: "Halo bro".to_string(),
            timestamp: 1726000000,
            is_from_owner: false,
            attachments: vec![],
            raw_metadata: serde_json::Value::Null,
        };
        assert!(!spec.evaluate_privacy(&dm_item));
    }
}
