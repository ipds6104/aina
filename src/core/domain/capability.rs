//! Deterministic capability model for agent-initiated privileged actions.
//!
//! The agent (LLM) is allowed to *propose* an authorization change by calling
//! the `aina policy` CLI, but it never decides permissions. Every privileged
//! action must present a short-lived [`TurnCapability`] that the core minted for
//! the **verified** WhatsApp sender of the current turn.
//!
//! Invariants enforced here:
//! 1. Only an `admin` (the configured Companion / Root of Trust) may grant or
//!    revoke authority.
//! 2. Group trust cannot be granted for the wrong group: when the turn happens
//!    inside a group, only that same group may be trusted or revoked.
//! 3. A capability can never mint `admin`: administrator status is bound to the
//!    configured Companion JID, not to a database row.
//! 4. Capabilities expire, so a leaked token stops working after
//!    [`CAPABILITY_TTL_SECS`].
//!
//! This replaces brittle phrase matching ("!aina izinkan ..."). Intent is
//! understood by the agent; permission is decided by this module.

use serde::{Deserialize, Serialize};

/// How long a minted capability stays valid (10 minutes).
pub const CAPABILITY_TTL_SECS: i64 = 600;

/// Verified authority of a WhatsApp principal.
///
/// Ordering matters: `Guest < Staff < Atasan < Admin`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Authority {
    Guest,
    Staff,
    Atasan,
    Admin,
}

impl Authority {
    pub fn parse(raw: &str) -> Self {
        match raw.trim().to_lowercase().as_str() {
            "admin" | "administrator" | "owner" => Authority::Admin,
            "atasan" | "supervisor" | "pimpinan" | "kepala" => Authority::Atasan,
            "staff" | "member" | "rekan" | "coworker" => Authority::Staff,
            _ => Authority::Guest,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Authority::Admin => "admin",
            Authority::Atasan => "atasan",
            Authority::Staff => "staff",
            Authority::Guest => "guest",
        }
    }

    /// Only the Root of Trust may delegate access.
    pub fn can_delegate(self) -> bool {
        self == Authority::Admin
    }

    /// Internal office data requires at least staff-level verification.
    pub fn can_access_internal(self) -> bool {
        self >= Authority::Staff
    }

    /// Admin is reserved for the configured Companion JID.
    pub fn is_delegable_target(self) -> bool {
        self != Authority::Admin
    }
}

impl std::fmt::Display for Authority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Short-lived proof that the current agent turn was triggered by a verified
/// principal with a specific authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnCapability {
    pub token: String,
    pub principal_jid: String,
    pub authority: Authority,
    pub chat_jid: String,
    pub chat_type: String,
    pub message_id: String,
    pub issued_at: i64,
    pub expires_at: i64,
    /// Deterministic target JIDs/numbers explicitly present in the active turn message
    /// (mentions, quoted reply sender, or explicit numbers/JIDs in `msg.text`, excluding bot).
    /// Inside a group chat, `GrantUser` / `RevokeUser` MUST match one of these targets.
    #[serde(default)]
    pub explicit_targets: Vec<String>,
}

impl TurnCapability {
    /// Mints an unforgeable token (192 bits of CSPRNG entropy) bound to the
    /// verified sender, chat, message, and authority of this turn.
    pub fn mint(
        principal_jid: impl Into<String>,
        authority: Authority,
        chat_jid: impl Into<String>,
        chat_type: impl Into<String>,
        message_id: impl Into<String>,
        now: i64,
    ) -> Self {
        Self {
            token: random_token(),
            principal_jid: principal_jid.into(),
            authority,
            chat_jid: chat_jid.into(),
            chat_type: chat_type.into(),
            message_id: message_id.into(),
            issued_at: now,
            expires_at: now + CAPABILITY_TTL_SECS,
            explicit_targets: Vec::new(),
        }
    }

    pub fn with_explicit_targets(mut self, targets: Vec<String>) -> Self {
        self.explicit_targets = targets;
        self
    }

    pub fn is_expired(&self, now: i64) -> bool {
        now >= self.expires_at
    }
}

fn is_bot_identity(candidate: &str, bot_jid: &str, bot_lid: Option<&str>) -> bool {
    let trimmed = candidate.trim();
    if trimmed.is_empty() {
        return false;
    }
    if !bot_jid.trim().is_empty() && is_admin_identity(bot_jid, trimmed) {
        return true;
    }
    if let Some(lid) = bot_lid {
        if !lid.trim().is_empty() && is_admin_identity(lid, trimmed) {
            return true;
        }
    }
    false
}

fn push_unique_target(out: &mut Vec<String>, candidate: &str, bot_jid: &str, bot_lid: Option<&str>) {
    let norm = candidate.trim().to_ascii_lowercase();
    if norm.is_empty() || is_bot_identity(&norm, bot_jid, bot_lid) {
        return;
    }
    if !out.iter().any(|existing| existing == &norm) {
        out.push(norm);
    }
}

/// Deterministically extracts explicit user targets referenced ONLY in the active
/// incoming message (`mentioned_jids`, `quoted_message.sender_jid`, and explicit
/// JIDs or 8–15 digit phone/LID tokens in `msg.text`), excluding the bot itself.
pub fn extract_explicit_targets(
    msg: &crate::core::domain::message::IncomingMessage,
    bot_jid: &str,
    bot_lid: Option<&str>,
) -> Vec<String> {
    let mut targets = Vec::new();

    // 1. WhatsApp mentions on the active message
    for mentioned in &msg.mentioned_jids {
        push_unique_target(&mut targets, mentioned, bot_jid, bot_lid);
    }

    // 2. Quoted / replied message author
    if let Some(ref quoted) = msg.quoted_message {
        push_unique_target(&mut targets, &quoted.sender_jid, bot_jid, bot_lid);
    }

    // 3. Explicit JIDs or 8–15 digit phone/LID numbers written in the active text
    for raw_token in msg.text.split_whitespace() {
        let cleaned = raw_token.trim_matches(|c: char| {
            matches!(
                c,
                '@' | ',' | '.' | ';' | ':' | '!' | '?' | '(' | ')' | '[' | ']' | '<' | '>' | '"' | '\'' | '*' | '_' | '~' | '`'
            )
        });
        if cleaned.is_empty() {
            continue;
        }

        if let Some((local, domain)) = cleaned.split_once('@') {
            let domain_lower = domain.to_ascii_lowercase();
            let local_digits_only = !local.is_empty() && local.chars().all(|c| c.is_ascii_digit());
            if local_digits_only
                && (domain_lower == "s.whatsapp.net" || domain_lower == "lid")
            {
                push_unique_target(
                    &mut targets,
                    &format!("{}@{}", local, domain_lower),
                    bot_jid,
                    bot_lid,
                );
            }
            continue;
        }

        // Check phone-like token containing only digits and optional separators (+, -)
        let trimmed_phone = cleaned.trim_start_matches('+');
        if !trimmed_phone.is_empty()
            && trimmed_phone
                .chars()
                .all(|c| c.is_ascii_digit() || c == '-')
        {
            let digits: String = trimmed_phone.chars().filter(|c| c.is_ascii_digit()).collect();
            if (8..=15).contains(&digits.len()) {
                push_unique_target(&mut targets, &digits, bot_jid, bot_lid);
                // Convenience normalization for local Indonesian numbers (08... -> 628...)
                if let Some(rest) = digits.strip_prefix('0') {
                    if !rest.is_empty() {
                        let intl = format!("62{}", rest);
                        push_unique_target(&mut targets, &intl, bot_jid, bot_lid);
                    }
                }
            }
        }
    }

    targets
}

/// Checks whether a proposed `user_jid` matches any deterministically extracted
/// explicit target from the active turn message.
pub fn target_matches_explicit(user_jid: &str, explicit_targets: &[String]) -> bool {
    let candidate = user_jid.trim();
    if candidate.is_empty() || explicit_targets.is_empty() {
        return false;
    }
    explicit_targets
        .iter()
        .any(|target| is_admin_identity(target, candidate))
}

/// 192-bit opaque token rendered as hex. No cryptographic signature is needed:
/// the token is unforgeable because it is generated and stored by the core, and
/// it only ever grants the authority of the principal it was minted for.
fn random_token() -> String {
    use std::fmt::Write;
    let mut out = String::with_capacity(48);
    for _ in 0..3 {
        let chunk: u64 = rand::random();
        let _ = write!(out, "{:016x}", chunk);
    }
    out
}

/// Returns true when `sender_jid` is one of the configured Root-of-Trust
/// identities.
///
/// `admin_config` accepts a comma/semicolon/space separated list because one
/// human legitimately appears under several WhatsApp identifiers: the phone
/// number (`…@s.whatsapp.net`) in direct chats and, after WhatsApp's LID
/// migration, a privacy identifier (`…@lid`) inside groups. Binding the Root of
/// Trust to only one of them makes authorization silently fail half the time.
///
/// Matching rules:
/// * a configured entry with a domain must match both local part and domain;
/// * a bare number matches any domain (convenience for `ADMIN_JID=628…`).
pub fn is_admin_identity(admin_config: &str, sender_jid: &str) -> bool {
    fn split(jid: &str) -> (&str, Option<&str>) {
        match jid.split_once('@') {
            Some((local, domain)) => (local, Some(domain)),
            None => (jid, None),
        }
    }

    let sender = sender_jid.trim().to_lowercase();
    if sender.is_empty() {
        return false;
    }
    let (s_local, s_domain) = split(&sender);

    admin_config
        .split(|c: char| c == ',' || c == ';' || c.is_whitespace())
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(|entry| entry.to_lowercase())
        .any(|entry| {
            let (e_local, e_domain) = split(&entry);
            match (e_domain, s_domain) {
                // Both sides qualified: domain must agree too, so a LID never
                // matches a phone number that happens to share digits.
                (Some(a), Some(b)) => a == b && e_local == s_local,
                // Either side bare: compare the local part only.
                _ => e_local == s_local,
            }
        })
}

/// Privileged action proposed by the agent (or by a local operator).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyAction {
    TrustGroup { group_jid: String },
    UntrustGroup { group_jid: String },
    GrantUser { user_jid: String, authority: Authority },
    RevokeUser { user_jid: String },
    AccessInternalData,
}

impl PolicyAction {
    /// Stable identifier used in audit logs.
    pub fn kind(&self) -> &'static str {
        match self {
            PolicyAction::TrustGroup { .. } => "trust_group",
            PolicyAction::UntrustGroup { .. } => "untrust_group",
            PolicyAction::GrantUser { .. } => "grant_user",
            PolicyAction::RevokeUser { .. } => "revoke_user",
            PolicyAction::AccessInternalData => "access_internal_data",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyDecision {
    pub allowed: bool,
    pub reason: String,
}

impl PolicyDecision {
    fn allow(reason: impl Into<String>) -> Self {
        Self { allowed: true, reason: reason.into() }
    }

    fn deny(reason: impl Into<String>) -> Self {
        Self { allowed: false, reason: reason.into() }
    }
}

/// The single authorization gate for agent-proposed privileged actions.
pub fn evaluate(capability: &TurnCapability, action: &PolicyAction, now: i64) -> PolicyDecision {
    if capability.is_expired(now) {
        return PolicyDecision::deny("capability_expired");
    }

    match action {
        PolicyAction::TrustGroup { group_jid } | PolicyAction::UntrustGroup { group_jid } => {
            if !capability.authority.can_delegate() {
                return PolicyDecision::deny("only_admin_may_change_group_trust");
            }
            if group_jid.trim().is_empty() {
                return PolicyDecision::deny("missing_group_jid");
            }
            // Inside a group the agent may only act on that same group. In a DM
            // the Companion may name any group explicitly.
            if capability.chat_type == "group" && capability.chat_jid != *group_jid {
                return PolicyDecision::deny("group_scope_violation");
            }
            PolicyDecision::allow("admin_verified")
        }
        PolicyAction::GrantUser { user_jid, authority } => {
            if !capability.authority.can_delegate() {
                return PolicyDecision::deny("only_admin_may_delegate");
            }
            if user_jid.trim().is_empty() {
                return PolicyDecision::deny("missing_user_jid");
            }
            if !authority.is_delegable_target() {
                return PolicyDecision::deny("admin_is_not_delegable");
            }
            if capability.chat_type == "group"
                && !target_matches_explicit(user_jid, &capability.explicit_targets)
            {
                return PolicyDecision::deny("target_not_in_active_turn_message");
            }
            PolicyDecision::allow("admin_verified")
        }
        PolicyAction::RevokeUser { user_jid } => {
            if !capability.authority.can_delegate() {
                return PolicyDecision::deny("only_admin_may_revoke");
            }
            if user_jid.trim().is_empty() {
                return PolicyDecision::deny("missing_user_jid");
            }
            if capability.chat_type == "group"
                && !target_matches_explicit(user_jid, &capability.explicit_targets)
            {
                return PolicyDecision::deny("target_not_in_active_turn_message");
            }
            PolicyDecision::allow("admin_verified")
        }
        PolicyAction::AccessInternalData => {
            if capability.authority.can_access_internal() {
                PolicyDecision::allow("staff_or_higher")
            } else {
                PolicyDecision::deny("guest_may_not_access_internal_data")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cap(authority: Authority, chat_jid: &str, chat_type: &str) -> TurnCapability {
        TurnCapability::mint("628111@s.whatsapp.net", authority, chat_jid, chat_type, "msg_1", 1_000)
    }

    #[test]
    fn admin_identity_supports_phone_and_lid() {
        let cfg = "6289625345646@s.whatsapp.net, 123456789012345@lid";
        // Direct chat (phone number)
        assert!(is_admin_identity(cfg, "6289625345646@s.whatsapp.net"));
        // Group chat after the LID migration
        assert!(is_admin_identity(cfg, "123456789012345@lid"));
        // Bare number in config still matches the phone JID
        assert!(is_admin_identity("6289625345646", "6289625345646@s.whatsapp.net"));
        // Same local part but different domain must NOT match
        assert!(!is_admin_identity(
            "6289625345646@s.whatsapp.net",
            "6289625345646@lid"
        ));
        // Strangers and empty config never match
        assert!(!is_admin_identity(cfg, "628111222333@s.whatsapp.net"));
        assert!(!is_admin_identity("", "6289625345646@s.whatsapp.net"));
        assert!(!is_admin_identity(cfg, "  "));
    }

    #[test]
    fn tokens_are_unique_and_long_enough() {
        let a = random_token();
        let b = random_token();
        assert_eq!(a.len(), 48);
        assert_ne!(a, b);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn authority_parsing_is_total() {
        assert_eq!(Authority::parse("ADMIN"), Authority::Admin);
        assert_eq!(Authority::parse("atasan"), Authority::Atasan);
        assert_eq!(Authority::parse("member"), Authority::Staff);
        assert_eq!(Authority::parse("whatever"), Authority::Guest);
        assert_eq!(Authority::parse(""), Authority::Guest);
    }

    #[test]
    fn admin_can_trust_current_group() {
        let c = cap(Authority::Admin, "123@g.us", "group");
        let d = evaluate(&c, &PolicyAction::TrustGroup { group_jid: "123@g.us".into() }, 1_001);
        assert!(d.allowed, "{}", d.reason);
    }

    #[test]
    fn staff_cannot_trust_group() {
        let c = cap(Authority::Staff, "123@g.us", "group");
        let d = evaluate(&c, &PolicyAction::TrustGroup { group_jid: "123@g.us".into() }, 1_001);
        assert!(!d.allowed);
        assert_eq!(d.reason, "only_admin_may_change_group_trust");
    }

    #[test]
    fn guest_cannot_grant_user() {
        let c = cap(Authority::Guest, "dm@s.whatsapp.net", "direct");
        let d = evaluate(
            &c,
            &PolicyAction::GrantUser { user_jid: "x@s.whatsapp.net".into(), authority: Authority::Staff },
            1_001,
        );
        assert!(!d.allowed);
        assert_eq!(d.reason, "only_admin_may_delegate");
    }

    #[test]
    fn group_scope_cannot_be_bypassed_inside_a_group() {
        let c = cap(Authority::Admin, "123@g.us", "group");
        let d = evaluate(&c, &PolicyAction::TrustGroup { group_jid: "999@g.us".into() }, 1_001);
        assert!(!d.allowed);
        assert_eq!(d.reason, "group_scope_violation");
    }

    #[test]
    fn admin_may_trust_any_group_from_dm() {
        let c = cap(Authority::Admin, "628111@s.whatsapp.net", "direct");
        let d = evaluate(&c, &PolicyAction::TrustGroup { group_jid: "999@g.us".into() }, 1_001);
        assert!(d.allowed, "{}", d.reason);
    }

    #[test]
    fn admin_cannot_be_granted_through_tooling() {
        let c = cap(Authority::Admin, "dm@s.whatsapp.net", "direct");
        let d = evaluate(
            &c,
            &PolicyAction::GrantUser { user_jid: "x@s.whatsapp.net".into(), authority: Authority::Admin },
            1_001,
        );
        assert!(!d.allowed);
        assert_eq!(d.reason, "admin_is_not_delegable");
    }

    #[test]
    fn expired_capability_is_rejected() {
        let c = cap(Authority::Admin, "123@g.us", "group");
        let d = evaluate(&c, &PolicyAction::TrustGroup { group_jid: "123@g.us".into() }, c.expires_at);
        assert!(!d.allowed);
        assert_eq!(d.reason, "capability_expired");
    }

    #[test]
    fn internal_data_requires_staff_or_higher() {
        let guest = cap(Authority::Guest, "dm", "direct");
        let staff = cap(Authority::Staff, "dm", "direct");
        assert!(!evaluate(&guest, &PolicyAction::AccessInternalData, 1_001).allowed);
        assert!(evaluate(&staff, &PolicyAction::AccessInternalData, 1_001).allowed);
    }

    #[test]
    fn group_grant_and_revoke_deny_target_not_in_active_turn_message() {
        let c = cap(Authority::Admin, "123@g.us", "group");
        // No explicit targets in Admin's active message ("siang Aina")
        let d_grant = evaluate(
            &c,
            &PolicyAction::GrantUser {
                user_jid: "6289999999999@s.whatsapp.net".into(),
                authority: Authority::Staff,
            },
            1_001,
        );
        assert!(!d_grant.allowed);
        assert_eq!(d_grant.reason, "target_not_in_active_turn_message");

        let d_revoke = evaluate(
            &c,
            &PolicyAction::RevokeUser {
                user_jid: "6289999999999@s.whatsapp.net".into(),
            },
            1_001,
        );
        assert!(!d_revoke.allowed);
        assert_eq!(d_revoke.reason, "target_not_in_active_turn_message");
    }

    #[test]
    fn group_grant_allows_only_explicit_target_and_blocks_injected_attacker() {
        let c = cap(Authority::Admin, "123@g.us", "group").with_explicit_targets(vec![
            "628111222333@s.whatsapp.net".to_string(),
        ]);

        let allowed = evaluate(
            &c,
            &PolicyAction::GrantUser {
                user_jid: "628111222333@s.whatsapp.net".into(),
                authority: Authority::Staff,
            },
            1_001,
        );
        assert!(allowed.allowed, "{}", allowed.reason);

        let denied = evaluate(
            &c,
            &PolicyAction::GrantUser {
                user_jid: "6289999999999@s.whatsapp.net".into(),
                authority: Authority::Staff,
            },
            1_001,
        );
        assert!(!denied.allowed);
        assert_eq!(denied.reason, "target_not_in_active_turn_message");
    }

    #[test]
    fn dm_grant_user_allows_any_target_without_explicit_targets() {
        let c = cap(Authority::Admin, "628111@s.whatsapp.net", "direct");
        let d = evaluate(
            &c,
            &PolicyAction::GrantUser {
                user_jid: "6289999999999@s.whatsapp.net".into(),
                authority: Authority::Staff,
            },
            1_001,
        );
        assert!(d.allowed, "{}", d.reason);
    }

    #[test]
    fn extract_explicit_targets_parses_mentions_replies_phones_and_excludes_bot() {
        use crate::core::domain::message::{
            ChatType, IncomingMessage, Platform, QuotedMessage, Sender, SessionRole,
        };

        let msg = IncomingMessage {
            id: "msg_target_1".into(),
            platform: Platform::WhatsApp,
            session_role: SessionRole::PrimaryBot,
            chat_jid: "123@g.us".into(),
            chat_type: ChatType::Group,
            sender: Sender {
                jid: "6289625345646@s.whatsapp.net".into(),
                name: Some("Admin".into()),
            },
            text: "@Aina tolong beri akses staff ke 0812-3456-7890 dan @628555444333@s.whatsapp.net".into(),
            timestamp: 1_700_000_000,
            is_from_me: false,
            quoted_message: Some(QuotedMessage {
                id: "q1".into(),
                sender_jid: "628777888999@s.whatsapp.net".into(),
                text: "halo".into(),
            }),
            mentioned_jids: vec![
                "628982157341@s.whatsapp.net".into(), // bot JID -> excluded
                "109389310636200@lid".into(),         // bot LID -> excluded
                "628111222333@s.whatsapp.net".into(), // explicit mentioned colleague
            ],
            is_bot_mentioned: true,
            bot_lid: Some("109389310636200@lid".into()),
            has_media: false,
            media_type: None,
            media_path: None,
        };

        let targets = extract_explicit_targets(
            &msg,
            "628982157341@s.whatsapp.net",
            Some("109389310636200@lid"),
        );

        assert!(target_matches_explicit("628111222333@s.whatsapp.net", &targets));
        assert!(target_matches_explicit("628777888999@s.whatsapp.net", &targets));
        assert!(target_matches_explicit("6281234567890@s.whatsapp.net", &targets));
        assert!(target_matches_explicit("628555444333@s.whatsapp.net", &targets));
        // Bot JID/LID and unrelated attacker must NOT match
        assert!(!target_matches_explicit("628982157341@s.whatsapp.net", &targets));
        assert!(!target_matches_explicit("109389310636200@lid", &targets));
        assert!(!target_matches_explicit("6289999999999@s.whatsapp.net", &targets));
    }
}
