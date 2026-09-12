use super::message::{ChatType, GatekeeperDecision, IncomingMessage, SessionRole};

pub struct Gatekeeper;

impl Gatekeeper {
    /// Pure domain decision logic to determine if Aina should respond to an incoming WhatsApp message.
    /// Supports both PrimaryBot (dedicated bot session) and UserCompanion (personal WhatsApp shadow sensor).
    pub fn evaluate(msg: &IncomingMessage, bot_jid: &str, bot_name: &str) -> GatekeeperDecision {
        // 1. Always ignore empty text
        let trimmed_text = msg.text.trim();
        if trimmed_text.is_empty() {
            return GatekeeperDecision::Ignore {
                reason: "Empty message ignored".to_string(),
            };
        }

        match msg.session_role {
            SessionRole::PrimaryBot => Self::evaluate_primary_bot(msg, trimmed_text, bot_jid, bot_name),
            SessionRole::UserCompanion => Self::evaluate_user_companion(msg, trimmed_text, bot_jid, bot_name),
        }
    }

    fn evaluate_primary_bot(
        msg: &IncomingMessage,
        trimmed_text: &str,
        bot_jid: &str,
        bot_name: &str,
    ) -> GatekeeperDecision {
        // Ignore own messages to prevent infinite loops on the dedicated bot number
        if msg.is_from_me {
            return GatekeeperDecision::Ignore {
                reason: "Self message ignored".to_string(),
            };
        }

        // Direct Message (1-on-1 private chat) to dedicated bot -> Always respond
        if msg.chat_type == ChatType::DirectMessage {
            return GatekeeperDecision::Respond {
                reason: "Direct message received".to_string(),
            };
        }

        // Group Chat -> Check if explicitly addressed
        if let Some(trigger_reason) = Self::detect_trigger(msg, trimmed_text, bot_jid, bot_name) {
            return GatekeeperDecision::Respond {
                reason: trigger_reason,
            };
        }

        // Default for group messages on primary bot: record for context memory
        GatekeeperDecision::RecordOnly {
            reason: "Group message without direct bot invocation recorded for context".to_string(),
        }
    }

    fn evaluate_user_companion(
        msg: &IncomingMessage,
        trimmed_text: &str,
        bot_jid: &str,
        bot_name: &str,
    ) -> GatekeeperDecision {
        match msg.chat_type {
            ChatType::DirectMessage => {
                if msg.is_from_me {
                    // Check if user is chatting with themselves (Saved Messages / Message Yourself)
                    let sender_clean = msg.sender.jid.split('@').next().unwrap_or(&msg.sender.jid);
                    let chat_clean = msg.chat_jid.split('@').next().unwrap_or(&msg.chat_jid);
                    let is_chat_to_self = sender_clean == chat_clean;

                    if is_chat_to_self {
                        return GatekeeperDecision::Respond {
                            reason: "Companion owner message in saved chat / chat-to-self".to_string(),
                        };
                    }

                    // In private DM with another contact, only respond if explicitly invoked
                    if let Some(trigger_reason) = Self::detect_trigger(msg, trimmed_text, bot_jid, bot_name) {
                        return GatekeeperDecision::Respond {
                            reason: format!("Companion owner explicit trigger in DM: {}", trigger_reason),
                        };
                    }

                    GatekeeperDecision::Ignore {
                        reason: "Companion owner message in private DM without trigger ignored".to_string(),
                    }
                } else {
                    // Critical Privacy Rule: NEVER interfere in private 1-on-1 chats from external contacts to user's personal number
                    GatekeeperDecision::Ignore {
                        reason: "Companion received DM from external contact; ignored for strict privacy".to_string(),
                    }
                }
            }
            ChatType::Group => {
                // In group chats where user companion is connected:
                if let Some(trigger_reason) = Self::detect_trigger(msg, trimmed_text, bot_jid, bot_name) {
                    let caller = if msg.is_from_me { "Companion owner" } else { "Group member" };
                    return GatekeeperDecision::Respond {
                        reason: format!("{} invoked Aina in group: {}", caller, trigger_reason),
                    };
                }

                // Ambient group message -> Ingest passively into knowledge base / FTS5
                GatekeeperDecision::RecordOnly {
                    reason: "Companion ambient group message recorded for context".to_string(),
                }
            }
        }
    }

    fn detect_trigger(
        msg: &IncomingMessage,
        trimmed_text: &str,
        bot_jid: &str,
        bot_name: &str,
    ) -> Option<String> {
        let bot_jid_clean = bot_jid.split('@').next().unwrap_or(bot_jid);
        let lower_text = trimmed_text.to_lowercase();
        let lower_bot_name = bot_name.to_lowercase();

        // 1. Command triggers: !aina, /aina, !ai, /ai, !help
        let cmd_prefixes = [
            format!("!{}", lower_bot_name),
            format!("/{}", lower_bot_name),
            "!ai".to_string(),
            "/ai".to_string(),
            "!aina".to_string(),
            "/aina".to_string(),
        ];
        for prefix in &cmd_prefixes {
            if lower_text == *prefix || lower_text.starts_with(&format!("{} ", prefix)) {
                return Some(format!("Command trigger '{}'", prefix));
            }
        }

        // 2. Mention check via mentioned_jids
        for mentioned in &msg.mentioned_jids {
            if mentioned.contains(bot_jid_clean) {
                return Some("Bot mentioned via JID in group".to_string());
            }
        }

        // 3. Quoted message reply check (someone replied to bot's previous message)
        if let Some(quoted) = &msg.quoted_message {
            if quoted.sender_jid.contains(bot_jid_clean) {
                return Some("Bot quoted/replied in group".to_string());
            }
        }

        // 4. Name prefix / address check in text
        let name_patterns = [
            format!("@{}", lower_bot_name),
            format!("{}:", lower_bot_name),
            format!("{},", lower_bot_name),
            format!("halo {}", lower_bot_name),
            format!("hai {}", lower_bot_name),
            format!("mbak {}", lower_bot_name),
            format!("kak {}", lower_bot_name),
            format!("tolong {}", lower_bot_name),
            format!("hey {}", lower_bot_name),
        ];

        for pattern in &name_patterns {
            if lower_text.starts_with(pattern) || lower_text.contains(pattern) {
                return Some(format!("Bot addressed by name pattern '{}'", pattern));
            }
        }

        // 5. Exact match or single word mention at start
        if lower_text == lower_bot_name || lower_text.starts_with(&format!("{} ", lower_bot_name)) {
            return Some("Bot name called at beginning of message".to_string());
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::domain::message::{ChatType, Platform, Sender, SessionRole};

    fn make_msg(
        chat_type: ChatType,
        text: &str,
        is_from_me: bool,
        session_role: SessionRole,
        chat_jid: &str,
        sender_jid: &str,
    ) -> IncomingMessage {
        IncomingMessage {
            id: "msg-1".to_string(),
            platform: Platform::WhatsApp,
            session_role,
            chat_jid: chat_jid.to_string(),
            chat_type,
            sender: Sender {
                jid: sender_jid.to_string(),
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
    fn test_primary_dm_always_responds() {
        let msg = make_msg(
            ChatType::DirectMessage,
            "Halo apa kabar?",
            false,
            SessionRole::PrimaryBot,
            "user-1@s.whatsapp.net",
            "user-1@s.whatsapp.net",
        );
        let dec = Gatekeeper::evaluate(&msg, "628999@s.whatsapp.net", "Aina");
        assert!(matches!(dec, GatekeeperDecision::Respond { .. }));
    }

    #[test]
    fn test_primary_group_unmentioned_records_only() {
        let msg = make_msg(
            ChatType::Group,
            "Meeting jam 2 ya semuanya",
            false,
            SessionRole::PrimaryBot,
            "group-123@g.us",
            "user-1@s.whatsapp.net",
        );
        let dec = Gatekeeper::evaluate(&msg, "628999@s.whatsapp.net", "Aina");
        assert!(matches!(dec, GatekeeperDecision::RecordOnly { .. }));
    }

    #[test]
    fn test_primary_group_name_addressed_responds() {
        let msg = make_msg(
            ChatType::Group,
            "Aina, tolong buatkan script backup db",
            false,
            SessionRole::PrimaryBot,
            "group-123@g.us",
            "user-1@s.whatsapp.net",
        );
        let dec = Gatekeeper::evaluate(&msg, "628999@s.whatsapp.net", "Aina");
        assert!(matches!(dec, GatekeeperDecision::Respond { .. }));
    }

    #[test]
    fn test_primary_self_message_ignored() {
        let msg = make_msg(
            ChatType::DirectMessage,
            "Pesan dari bot sendiri",
            true,
            SessionRole::PrimaryBot,
            "user-1@s.whatsapp.net",
            "628999@s.whatsapp.net",
        );
        let dec = Gatekeeper::evaluate(&msg, "628999@s.whatsapp.net", "Aina");
        assert!(matches!(dec, GatekeeperDecision::Ignore { .. }));
    }

    #[test]
    fn test_companion_external_dm_strictly_ignored() {
        // Someone sends a personal DM to user's companion WhatsApp account
        let msg = make_msg(
            ChatType::DirectMessage,
            "Bro ntar malam nongkrong gak?",
            false,
            SessionRole::UserCompanion,
            "friend@s.whatsapp.net",
            "friend@s.whatsapp.net",
        );
        let dec = Gatekeeper::evaluate(&msg, "628999@s.whatsapp.net", "Aina");
        assert!(matches!(dec, GatekeeperDecision::Ignore { .. }));
    }

    #[test]
    fn test_companion_chat_to_self_responds() {
        // User sends a message to their own number (Saved Messages)
        let msg = make_msg(
            ChatType::DirectMessage,
            "Ingatkan besok belanja server",
            true,
            SessionRole::UserCompanion,
            "628111@s.whatsapp.net",
            "628111@s.whatsapp.net",
        );
        let dec = Gatekeeper::evaluate(&msg, "628999@s.whatsapp.net", "Aina");
        assert!(matches!(dec, GatekeeperDecision::Respond { .. }));
    }

    #[test]
    fn test_companion_owner_group_trigger_responds() {
        // User speaks in group calling !aina
        let msg = make_msg(
            ChatType::Group,
            "!aina rangkum diskusi barusan",
            true,
            SessionRole::UserCompanion,
            "work-group@g.us",
            "628111@s.whatsapp.net",
        );
        let dec = Gatekeeper::evaluate(&msg, "628999@s.whatsapp.net", "Aina");
        assert!(matches!(dec, GatekeeperDecision::Respond { .. }));
    }

    #[test]
    fn test_companion_group_ambient_records_only() {
        // Other members chat in group without trigger -> ambient logging
        let msg = make_msg(
            ChatType::Group,
            "Oke sprint planning jam 10 pagi",
            false,
            SessionRole::UserCompanion,
            "work-group@g.us",
            "colleague@s.whatsapp.net",
        );
        let dec = Gatekeeper::evaluate(&msg, "628999@s.whatsapp.net", "Aina");
        assert!(matches!(dec, GatekeeperDecision::RecordOnly { .. }));
    }
}

