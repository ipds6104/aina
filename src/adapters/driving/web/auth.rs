use super::state::WebhookServerState;
use axum::http::HeaderMap;
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use tokio::sync::Mutex;
use tracing::warn;

const AUTH_MAX_FAILURES_PER_WINDOW: u32 = 5;
const AUTH_WINDOW_SECS: i64 = 60;
const AUTH_BASE_LOCKOUT_SECS: i64 = 60;
const AUTH_MAX_LOCKOUT_SECS: i64 = 600;
const AUTH_LIMITER_MAX_ENTRIES: usize = 10_000;

/// Constant-time byte slice comparison to prevent timing side-channel attacks.
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (&x, &y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Fail-closed constant-time string comparison.
/// Empty or whitespace-only expected/candidate secrets ALWAYS return `false`.
pub fn constant_time_str_eq(candidate: &str, expected: &str) -> bool {
    let exp = expected.trim();
    let cand = candidate.trim();
    if exp.is_empty() || cand.is_empty() {
        return false;
    }
    constant_time_eq(cand.as_bytes(), exp.as_bytes())
}

/// Generates a lowercase hex string with `num_bytes` of CSPRNG entropy using `rand`.
pub fn generate_csprng_hex(num_bytes: usize) -> String {
    use std::fmt::Write;
    let mut out = String::with_capacity(num_bytes * 2);
    let mut remaining = num_bytes;
    while remaining >= 8 {
        let chunk: u64 = rand::random();
        let _ = write!(out, "{:016x}", chunk);
        remaining -= 8;
    }
    while remaining > 0 {
        let b: u8 = rand::random();
        let _ = write!(out, "{:02x}", b);
        remaining -= 1;
    }
    out
}

#[derive(Debug, Clone)]
struct AuthAttemptWindow {
    window_start: i64,
    failures: u32,
    lockout_count: u32,
    locked_until: i64,
    last_seen: i64,
}

/// Reverse-proxy-aware per-IP failure bucket for `/api/setup` and `/api/auth/verify`.
pub struct AuthAttemptLimiter {
    entries: Mutex<HashMap<IpAddr, AuthAttemptWindow>>,
}

impl Default for AuthAttemptLimiter {
    fn default() -> Self {
        Self::new()
    }
}

impl AuthAttemptLimiter {
    pub fn new() -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
        }
    }

    /// Returns `Some(retry_after_secs)` if the client IP is currently locked out.
    pub async fn check_locked(&self, ip: IpAddr, now_epoch: i64) -> Option<u64> {
        let map = self.entries.lock().await;
        if let Some(win) = map.get(&ip) {
            if now_epoch < win.locked_until {
                return Some((win.locked_until - now_epoch) as u64);
            }
        }
        None
    }

    /// Records a failed auth attempt and returns `Some(retry_after_secs)` if lockout triggered.
    pub async fn record_failure(&self, ip: IpAddr, now_epoch: i64) -> Option<u64> {
        let mut map = self.entries.lock().await;

        if map.len() >= AUTH_LIMITER_MAX_ENTRIES && !map.contains_key(&ip) {
            map.retain(|_, v| {
                now_epoch - v.last_seen < AUTH_MAX_LOCKOUT_SECS || now_epoch < v.locked_until
            });
            if map.len() >= AUTH_LIMITER_MAX_ENTRIES {
                if let Some(oldest_key) = map
                    .iter()
                    .min_by_key(|(_, v)| v.last_seen)
                    .map(|(k, _)| *k)
                {
                    map.remove(&oldest_key);
                }
            }
        }

        let entry = map.entry(ip).or_insert(AuthAttemptWindow {
            window_start: now_epoch,
            failures: 0,
            lockout_count: 0,
            locked_until: 0,
            last_seen: now_epoch,
        });

        entry.last_seen = now_epoch;

        if now_epoch < entry.locked_until {
            return Some((entry.locked_until - now_epoch) as u64);
        }

        if now_epoch - entry.window_start >= AUTH_WINDOW_SECS {
            entry.window_start = now_epoch;
            entry.failures = 0;
        }

        entry.failures = entry.failures.saturating_add(1);
        if entry.failures >= AUTH_MAX_FAILURES_PER_WINDOW {
            entry.lockout_count = entry.lockout_count.saturating_add(1);
            let exp_shift = (entry.lockout_count.saturating_sub(1)).min(4);
            let lockout_secs = (AUTH_BASE_LOCKOUT_SECS << exp_shift).min(AUTH_MAX_LOCKOUT_SECS);
            entry.locked_until = now_epoch + lockout_secs;
            entry.failures = 0;
            return Some(lockout_secs as u64);
        }

        None
    }

    /// Clears failure history for an IP upon successful authentication.
    pub async fn record_success(&self, ip: IpAddr) {
        let mut map = self.entries.lock().await;
        map.remove(&ip);
    }
}

/// Extracts client IP using `ConnectInfo` by default, or reverse-proxy headers
/// (`X-Forwarded-For` / `X-Real-IP`) ONLY when `trust_proxy_headers` is explicitly enabled.
pub fn extract_client_ip(
    headers: &HeaderMap,
    peer_addr: Option<SocketAddr>,
    trust_proxy_headers: bool,
) -> IpAddr {
    if trust_proxy_headers {
        if let Some(xff) = headers.get("x-forwarded-for").and_then(|v| v.to_str().ok()) {
            if let Some(first) = xff.split(',').next().map(str::trim) {
                if let Ok(ip) = first.parse::<IpAddr>() {
                    return ip;
                }
            }
        }
        if let Some(real_ip) = headers.get("x-real-ip").and_then(|v| v.to_str().ok()) {
            if let Ok(ip) = real_ip.trim().parse::<IpAddr>() {
                return ip;
            }
        }
    }

    peer_addr
        .map(|a| a.ip())
        .unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST))
}

fn extract_bearer(headers: &HeaderMap) -> Option<&str> {
    headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|auth| auth.strip_prefix("Bearer "))
}

/// Fail-closed constant-time admin key check via HTTP headers (`X-Admin-Key`, `X-API-Key`, `Authorization: Bearer`).
/// Never accepts empty expected codes and never accepts query parameters.
pub fn is_admin_authorized(headers: &HeaderMap, expected_code: &str) -> bool {
    let expected = expected_code.trim();
    if expected.is_empty() {
        return false;
    }

    if let Some(key) = headers.get("X-Admin-Key").and_then(|v| v.to_str().ok()) {
        if constant_time_str_eq(key, expected) {
            return true;
        }
    }

    if let Some(key) = headers.get("X-API-Key").and_then(|v| v.to_str().ok()) {
        if constant_time_str_eq(key, expected) {
            return true;
        }
    }

    if let Some(bearer) = extract_bearer(headers) {
        if constant_time_str_eq(bearer, expected) {
            return true;
        }
    }

    false
}

/// Fail-closed header-only & OIDC-session authorization for admin/audit/diagnostic web APIs.
/// Separated from `whatsmeow_api_key`: WhatsApp gateway credentials cannot authenticate web admin routes.
pub fn is_api_authorized(headers: &HeaderMap, state: &WebhookServerState) -> bool {
    if is_admin_authorized(headers, &state.setup_code) {
        return true;
    }

    // Check if an authenticated OIDC admin session cookie is active
    let secret = crate::adapters::driving::web::controllers::oidc::resolve_session_secret(state);
    if let Some(session) =
        crate::adapters::driving::web::controllers::oidc::extract_session_from_headers(
            headers, &secret,
        )
    {
        if session.is_admin {
            return true;
        }
    }

    false
}

pub fn is_admin_session_or_key_authorized(
    headers: &HeaderMap,
    state: &WebhookServerState,
) -> bool {
    is_api_authorized(headers, state)
}

/// Fail-closed authorization for `/api/v1/events` and `/api/events`.
/// Header-first by default (`X-Events-Key`, `X-API-Key`, `Authorization: Bearer`).
/// Query parameter (`?api_key=` / `?token=` / `?key=`) is ONLY accepted when
/// `state.events_allow_query_key` is explicitly opted in for legacy webhooks.
pub fn is_events_authorized(
    headers: &HeaderMap,
    query_key: Option<&str>,
    state: &WebhookServerState,
) -> bool {
    let effective_query_key = match (query_key, state.events_allow_query_key) {
        (Some(k), true) => {
            warn!(
                "Events ingress authenticated via URL query parameter (AINA_EVENTS_ALLOW_QUERY_KEY=true); prefer X-Events-Key / X-API-Key header"
            );
            Some(k)
        }
        (Some(_), false) => {
            warn!(
                "Rejected URL query credential on /api/v1/events (set AINA_EVENTS_ALLOW_QUERY_KEY=true only for legacy webhooks)"
            );
            None
        }
        (None, _) => None,
    };

    // 1. If explicit events_api_key is configured:
    if let Some(ref configured) = state.events_api_key {
        let expected = configured.trim();
        if !expected.is_empty() {
            if let Some(key) = effective_query_key {
                if constant_time_str_eq(key, expected) {
                    return true;
                }
            }
            if let Some(key) = headers.get("X-Events-Key").and_then(|v| v.to_str().ok()) {
                if constant_time_str_eq(key, expected) {
                    return true;
                }
            }
            if let Some(key) = headers.get("X-API-Key").and_then(|v| v.to_str().ok()) {
                if constant_time_str_eq(key, expected) {
                    return true;
                }
            }
            if let Some(bearer) = extract_bearer(headers) {
                if constant_time_str_eq(bearer, expected) {
                    return true;
                }
            }
            // Allow admin key / OIDC admin session fallback even when events_api_key is explicitly set
            return is_api_authorized(headers, state);
        }
    }

    // 2. Fallback to setup_code (fail-closed if empty) or OIDC admin session
    if let Some(key) = effective_query_key {
        if constant_time_str_eq(key, &state.setup_code) {
            return true;
        }
    }
    if let Some(key) = headers.get("X-Events-Key").and_then(|v| v.to_str().ok()) {
        if constant_time_str_eq(key, &state.setup_code) {
            return true;
        }
    }

    is_api_authorized(headers, state)
}

/// Hybrid upstream-compatible authentication for `POST /webhook`.
/// - When `AINA_WEBHOOK_SECRET` is configured (non-empty): strictly fail-closed via
///   `X-Webhook-Secret`, `X-API-Key`, or `Authorization: Bearer` using constant-time comparison.
/// - When unset: allows traffic for out-of-the-box internal Docker/Whatsmeow setups.
pub fn is_webhook_authorized(headers: &HeaderMap, state: &WebhookServerState) -> bool {
    let Some(ref configured) = state.webhook_secret else {
        return true;
    };
    let expected = configured.trim();
    if expected.is_empty() {
        return true;
    }

    if let Some(key) = headers.get("X-Webhook-Secret").and_then(|v| v.to_str().ok()) {
        if constant_time_str_eq(key, expected) {
            return true;
        }
    }

    if let Some(key) = headers.get("X-API-Key").and_then(|v| v.to_str().ok()) {
        if constant_time_str_eq(key, expected) {
            return true;
        }
    }

    if let Some(bearer) = extract_bearer(headers) {
        if constant_time_str_eq(bearer, expected) {
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_admin_authorized_fail_closed_on_empty_expected_code() {
        let mut headers = HeaderMap::new();
        assert!(!is_admin_authorized(&headers, ""));
        assert!(!is_admin_authorized(&headers, "   "));

        headers.insert("X-Admin-Key", "".parse().unwrap());
        assert!(!is_admin_authorized(&headers, ""));
        headers.insert("X-Admin-Key", "any-key".parse().unwrap());
        assert!(!is_admin_authorized(&headers, ""));
    }

    #[test]
    fn test_constant_time_str_eq_behavior() {
        assert!(constant_time_str_eq("secret-123", "secret-123"));
        assert!(constant_time_str_eq("  secret-123  ", "secret-123"));
        assert!(!constant_time_str_eq("secret-124", "secret-123"));
        assert!(!constant_time_str_eq("secret-12", "secret-123"));
        assert!(!constant_time_str_eq("", ""));
        assert!(!constant_time_str_eq("   ", "   "));
    }

    #[test]
    fn test_generate_csprng_hex_length_and_uniqueness() {
        let a = generate_csprng_hex(16);
        let b = generate_csprng_hex(16);
        assert_eq!(a.len(), 32);
        assert_ne!(a, b);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[tokio::test]
    async fn test_auth_attempt_limiter_lockout_and_reset() {
        let limiter = AuthAttemptLimiter::new();
        let ip: IpAddr = "10.0.0.5".parse().unwrap();

        for i in 0..4 {
            assert!(
                limiter.record_failure(ip, 1_000 + i).await.is_none(),
                "Attempts 1..=4 must not lock out yet"
            );
        }

        let locked = limiter.record_failure(ip, 1_005).await;
        assert_eq!(locked, Some(60));
        assert!(limiter.check_locked(ip, 1_030).await.is_some());
        assert!(limiter.check_locked(ip, 1_066).await.is_none());

        limiter.record_success(ip).await;
        assert!(limiter.check_locked(ip, 1_030).await.is_none());
    }

    #[test]
    fn test_extract_client_ip_respects_trust_proxy_flag() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", "203.0.113.9, 10.0.0.1".parse().unwrap());
        let peer: SocketAddr = "172.18.0.2:44321".parse().unwrap();

        // Untrusted proxy: ignore X-Forwarded-For to prevent spoofing
        assert_eq!(
            extract_client_ip(&headers, Some(peer), false),
            "172.18.0.2".parse::<IpAddr>().unwrap()
        );
        // Trusted proxy: honor client IP from X-Forwarded-For
        assert_eq!(
            extract_client_ip(&headers, Some(peer), true),
            "203.0.113.9".parse::<IpAddr>().unwrap()
        );
    }
}
