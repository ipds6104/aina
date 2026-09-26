//! Multi-account token pool, email extraction, cooldown tracking, and quota error classification.

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredAccountToken {
    pub id: usize,
    pub label: String,
    pub email: Option<String>,
    pub token_json: String,
}

#[derive(Debug, Clone)]
pub struct AccountToken {
    #[allow(dead_code)]
    pub id: usize,
    pub label: String,
    pub email: Option<String>,
    pub token_json: String,
    pub cooldown_until: Arc<RwLock<Option<std::time::Instant>>>,
}

pub fn extract_email_from_token(token_json: &str) -> Option<String> {
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(token_json) {
        if let Some(email) = val.get("email").and_then(|e| e.as_str()) {
            return Some(email.to_string());
        }
        if let Some(id_token) = val.get("id_token").and_then(|t| t.as_str()) {
            let parts: Vec<&str> = id_token.split('.').collect();
            if parts.len() >= 2 {
                use base64::Engine;
                let mut b64 = parts[1].to_string();
                while b64.len() % 4 != 0 {
                    b64.push('=');
                }
                if let Ok(decoded_bytes) = base64::engine::general_purpose::STANDARD
                    .decode(&b64)
                    .or_else(|_| base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(parts[1]))
                {
                    if let Ok(jwt_json) = serde_json::from_slice::<serde_json::Value>(&decoded_bytes) {
                        if let Some(email) = jwt_json.get("email").and_then(|e| e.as_str()) {
                            return Some(email.to_string());
                        }
                    }
                }
            }
        }
    }
    None
}

#[allow(dead_code)]
pub fn mask_email(email: &str) -> String {
    if let Some((user, domain)) = email.split_once('@') {
        if user.len() <= 3 {
            format!("{}***@{}", user, domain)
        } else {
            format!("{}***@{}", &user[..3], domain)
        }
    } else {
        email.to_string()
    }
}

/// Intelligently parses Google quota exhaustion and rate limit errors
/// to determine an accurate cooldown period (e.g. extracts "Resets in 65h1m18s" or assigns default buckets).
pub fn extract_quota_cooldown_duration(err: &str) -> Duration {
    let lower = err.to_lowercase();
    if let Some(pos) = lower.find("resets in ") {
        let rest = &lower[pos + "resets in ".len()..];
        let token = rest.split_whitespace().next().unwrap_or("").trim_end_matches('.');
        let mut total_secs = 0u64;
        let mut num = 0u64;
        for ch in token.chars() {
            if ch.is_ascii_digit() {
                num = num * 10 + (ch as u64 - '0' as u64);
            } else if ch == 'h' {
                total_secs += num * 3600;
                num = 0;
            } else if ch == 'm' {
                total_secs += num * 60;
                num = 0;
            } else if ch == 's' {
                total_secs += num;
                num = 0;
            }
        }
        if total_secs > 0 {
            // Cap at 72 hours for safety, minimum 60s
            return Duration::from_secs(total_secs.clamp(60, 72 * 3600));
        }
    }

    // Daily/subscription quota exhausted without explicit duration
    if lower.contains("individual quota reached")
        || lower.contains("please upgrade your subscription")
        || lower.contains("exceeded your current quota")
    {
        return Duration::from_secs(4 * 3600); // 4 hours
    }

    // Transient rate limit (RPM/TPM / 503)
    Duration::from_secs(300) // 5 minutes
}

pub fn is_transient_error(s: &str) -> bool {
    let lower = s.to_lowercase();
    lower.contains("subscriber fell behind updates")
        || lower.contains("connection to the agent was interrupted")
        || lower.contains("stalled for")
        || lower.contains("connection reset")
        || lower.contains("broken pipe")
        || lower.contains("stream error")
        || lower.contains("deadline has elapsed")
        || lower.contains("transport error")
        || lower.contains("temporarily unavailable")
        || lower.contains("client network socket disconnected")
        || lower.contains("econnreset")
        || lower.contains("etimedout")
        || lower.contains("timed out")
        || lower.contains("timeout")
}

pub fn is_quota_error(s: &str) -> bool {
    let lower = s.to_lowercase();
    lower.contains("503")
        || lower.contains("429")
        || lower.contains("quota")
        || lower.contains("resource has been exhausted")
        || lower.contains("resource_exhausted")
        || lower.contains("rate limit")
        || lower.contains("rate-limit")
        || lower.contains("too many requests")
        || lower.contains("exceeded your current quota")
        || lower.contains("capacity")
}

pub fn is_auth_error(s: &str) -> bool {
    let lower = s.to_lowercase();
    lower.contains("eligibility check failed")
        || lower.contains("not eligible for antigravity")
        || lower.contains("verify your account to continue")
        || lower.contains("signin/continue")
        || lower.contains("not logged into antigravity")
        || lower.contains("invalid_grant")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[tokio::test]
    async fn test_token_pool_round_robin_rotation_and_cooldown() {
        let pool = vec![
            AccountToken {
                id: 1,
                label: "Account-1".to_string(),
                email: None,
                token_json: "tok1".to_string(),
                cooldown_until: Arc::new(RwLock::new(None)),
            },
            AccountToken {
                id: 2,
                label: "Account-2".to_string(),
                email: None,
                token_json: "tok2".to_string(),
                cooldown_until: Arc::new(RwLock::new(None)),
            },
            AccountToken {
                id: 3,
                label: "Account-3".to_string(),
                email: None,
                token_json: "tok3".to_string(),
                cooldown_until: Arc::new(RwLock::new(None)),
            },
            AccountToken {
                id: 4,
                label: "Account-4".to_string(),
                email: None,
                token_json: "tok4".to_string(),
                cooldown_until: Arc::new(RwLock::new(None)),
            },
        ];

        let rr_counter = Arc::new(AtomicUsize::new(0));

        // Test normal round-robin
        let mut picked_labels = Vec::new();
        for _ in 0..4 {
            let start = rr_counter.fetch_add(1, Ordering::Relaxed);
            let idx = start % pool.len();
            picked_labels.push(pool[idx].label.clone());
        }
        assert_eq!(
            picked_labels,
            vec!["Account-1", "Account-2", "Account-3", "Account-4"]
        );

        // Account-2 in cooldown
        *pool[1].cooldown_until.write().await = Some(std::time::Instant::now() + std::time::Duration::from_secs(300));

        // When counter points to index 1 (Account-2), it should skip to Account-3
        let start = rr_counter.fetch_add(1, Ordering::Relaxed);
        assert_eq!(pool[start % pool.len()].label, "Account-1");

        let start2 = rr_counter.fetch_add(1, Ordering::Relaxed);
        let now = std::time::Instant::now();
        let mut candidate = None;
        let n = pool.len();
        for i in 0..n {
            let idx = (start2 + i) % n;
            let acc = &pool[idx];
            let cd = acc.cooldown_until.read().await;
            if let Some(until) = *cd {
                if now < until {
                    continue;
                }
            }
            candidate = Some(acc.clone());
            break;
        }
        assert_eq!(candidate.unwrap().label, "Account-3");
    }

    #[test]
    fn test_mask_email() {
        assert_eq!(mask_email("ihzathegodslayer@gmail.com"), "ihz***@gmail.com");
        assert_eq!(mask_email("ab@gmail.com"), "ab***@gmail.com");
        assert_eq!(mask_email("user@domain.co.id"), "use***@domain.co.id");
        assert_eq!(mask_email("plainstring"), "plainstring");
    }

    #[test]
    fn test_extract_email_from_jwt_payload() {
        use base64::Engine;
        let payload_json = r#"{"email":"testuser@example.com"}"#;
        let b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(payload_json);
        let fake_jwt = format!("eyJhbGciOiJSUzI1NiJ9.{}.fakesig", b64);
        let token_json = format!(r#"{{"token":"abc","id_token":"{}"}}"#, fake_jwt);

        let email = extract_email_from_token(&token_json);
        assert_eq!(email, Some("testuser@example.com".to_string()));
    }

    #[test]
    fn test_extract_quota_cooldown_duration() {
        // 1. Explicit resets in 65h1m18s
        let err1 = "RESOURCE_EXHAUSTED (code 429): Individual quota reached. Please upgrade your subscription to increase your limits. Resets in 65h1m18s.";
        let dur1 = extract_quota_cooldown_duration(err1);
        assert_eq!(dur1.as_secs(), 65 * 3600 + 1 * 60 + 18);

        // 2. Explicit resets in 2h30m
        let err2 = "Quota exceeded. Resets in 2h30m.";
        let dur2 = extract_quota_cooldown_duration(err2);
        assert_eq!(dur2.as_secs(), 2 * 3600 + 30 * 60);

        // 3. Subscription/daily limit without explicit time
        let err3 = "Individual quota reached. Please upgrade your subscription.";
        let dur3 = extract_quota_cooldown_duration(err3);
        assert_eq!(dur3.as_secs(), 4 * 3600);

        // 4. Transient rate limit
        let err4 = "Error: 429 Too Many Requests: Rate limit exceeded.";
        let dur4 = extract_quota_cooldown_duration(err4);
        assert_eq!(dur4.as_secs(), 300);
    }

    #[test]
    fn test_error_classification_transient_quota_auth() {
        // 1. Transient stream interruption
        let stream_stall = "Antigravity CLI failed: error: the connection to the agent was interrupted before the response finished: subscriber fell behind updates, stalled for 5s";
        assert!(is_transient_error(stream_stall));
        assert!(!is_quota_error(stream_stall));
        assert!(!is_auth_error(stream_stall));

        // 2. Connection reset & network drops
        assert!(is_transient_error("error: connection reset by peer"));
        assert!(is_transient_error("transport error: stream terminated"));
        assert!(is_transient_error("request timed out after 30s"));

        // 3. Quota errors
        let quota_err = "Error 429: Resource has been exhausted (rate limit exceeded).";
        assert!(is_quota_error(quota_err));
        assert!(!is_transient_error(quota_err));

        // 4. Auth errors
        let auth_err = "Eligibility check failed: Your account is not eligible for Antigravity.";
        assert!(is_auth_error(auth_err));
        assert!(!is_transient_error(auth_err));
    }
}
