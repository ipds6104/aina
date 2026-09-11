use super::message::{ChatType, GatekeeperDecision, IncomingMessage};

pub struct Gatekeeper;

impl Gatekeeper {
    /// Pure domain decision logic to determine if Aina should respond to an incoming WhatsApp message.
    pub fn evaluate(msg: &IncomingMessage, bot_jid: &str, bot_name: &str) -> GatekeeperDecision {
        // 1. Ignore own messages to prevent infinite loops
        if msg.is_from_me {
            return GatekeeperDecision::Ignore {
                reason: "Self message ignored".to_string(),
            };
        }

        // 2. Ignore empty text
        let trimmed_text = msg.text.trim();
        if trimmed_text.is_empty() {
            return GatekeeperDecision::Ignore {
                reason: "Empty message ignored".to_string(),
            };
        }

        // 3. Direct Message (1-on-1 private chat) -> Always respond
        if msg.chat_type == ChatType::DirectMessage {
            return GatekeeperDecision::Respond {
                reason: "Direct message received".to_string(),
            };
        }

        // 4. Group Chat -> Check if explicitly addressed
        let bot_jid_clean = bot_jid.split('@').next().unwrap_or(bot_jid);
        
        // A. Mention check via mentioned_jids
        for mentioned in &msg.mentioned_jids {
            if mentioned.contains(bot_jid_clean) {
                return GatekeeperDecision::Respond {
                    reason: "Bot mentioned via JID in group".to_string(),
                };
            }
        }

        // B. Quoted message reply check (someone replied to bot's previous message)
        if let Some(quoted) = &msg.quoted_message {
            if quoted.sender_jid.contains(bot_jid_clean) {
                return GatekeeperDecision::Respond {
                    reason: "Bot quoted/replied in group".to_string(),
                };
            }
        }

        // C. Name prefix / address check in text
        let lower_text = trimmed_text.to_lowercase();
        let lower_bot_name = bot_name.to_lowercase();

        // Common Indonesian address patterns: "aina", "@aina", "halo aina", "hai aina", "mbak aina", "kak aina", "na,"
        let name_patterns = [
            format!("@{}", lower_bot_name),
            format!("{}:", lower_bot_name),
            format!("{},", lower_bot_name),
            format!("halo {}", lower_bot_name),
            format!("hai {}", lower_bot_name),
            format!("mbak {}", lower_bot_name),
            format!("kak {}", lower_bot_name),
            format!("tolong {}", lower_bot_name),
        ];

        for pattern in &name_patterns {
            if lower_text.starts_with(pattern) || lower_text.contains(pattern) {
                return GatekeeperDecision::Respond {
                    reason: format!("Bot addressed by name pattern '{}' in group", pattern),
                };
            }
        }

        // Exact match or single word mention
        if lower_text == lower_bot_name || lower_text.starts_with(&format!("{} ", lower_bot_name)) {
            return GatekeeperDecision::Respond {
                reason: "Bot name called at beginning of message".to_string(),
            };
        }

        // Default for group messages: ambient logging / record only
        GatekeeperDecision::RecordOnly {
            reason: "Group message without direct bot invocation recorded for context".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::domain::message::{Sender, ChatType, Platform};

    fn make_msg(chat_type: ChatType, text: &str, is_from_me: bool) -> IncomingMessage {
        IncomingMessage {
            id: "msg-1".to_string(),
            platform: Platform::WhatsApp,
            chat_jid: "group-123@g.us".to_string(),
            chat_type,
            sender: Sender {
                jid: "user-1@s.whatsapp.net".to_string(),
                name: Some("Budi".to_string()),
            },
            text: text.to_string(),
            timestamp: 1726040000,
            is_from_me,
            quoted_message: None,
            mentioned_jids: vec![],
        }
    }

    #[test]
    fn test_dm_always_responds() {
        let msg = make_msg(ChatType::DirectMessage, "Halo apa kabar?", false);
        let dec = Gatekeeper::evaluate(&msg, "628999@s.whatsapp.net", "Aina");
        assert!(matches!(dec, GatekeeperDecision::Respond { .. }));
    }

    #[test]
    fn test_group_unmentioned_records_only() {
        let msg = make_msg(ChatType::Group, "Meeting jam 2 ya semuanya", false);
        let dec = Gatekeeper::evaluate(&msg, "628999@s.whatsapp.net", "Aina");
        assert!(matches!(dec, GatekeeperDecision::RecordOnly { .. }));
    }

    #[test]
    fn test_group_name_addressed_responds() {
        let msg = make_msg(ChatType::Group, "Aina, tolong buatkan script backup db", false);
        let dec = Gatekeeper::evaluate(&msg, "628999@s.whatsapp.net", "Aina");
        assert!(matches!(dec, GatekeeperDecision::Respond { .. }));
    }

    #[test]
    fn test_self_message_ignored() {
        let msg = make_msg(ChatType::DirectMessage, "Pesan dari bot sendiri", true);
        let dec = Gatekeeper::evaluate(&msg, "628999@s.whatsapp.net", "Aina");
        assert!(matches!(dec, GatekeeperDecision::Ignore { .. }));
    }
}
