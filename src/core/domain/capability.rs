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
        }
    }

    pub fn is_expired(&self, now: i64) -> bool {
        now >= self.expires_at
    }
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
            PolicyDecision::allow("admin_verified")
        }
        PolicyAction::RevokeUser { user_jid } => {
            if !capability.authority.can_delegate() {
                return PolicyDecision::deny("only_admin_may_revoke");
            }
            if user_jid.trim().is_empty() {
                return PolicyDecision::deny("missing_user_jid");
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
}
