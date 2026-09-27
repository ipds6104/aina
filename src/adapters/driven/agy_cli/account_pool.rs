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

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use tracing::{info, warn};

pub struct TokenPoolManager {
    token_pool: Arc<RwLock<Vec<AccountToken>>>,
    round_robin_counter: Arc<AtomicUsize>,
}

impl TokenPoolManager {
    pub fn new() -> Self {
        let pool = Self::load_initial_token_pool();
        if !pool.is_empty() {
            info!("Initialized Antigravity account pool with {} account(s)", pool.len());
        }
        Self {
            token_pool: Arc::new(RwLock::new(pool)),
            round_robin_counter: Arc::new(AtomicUsize::new(0)),
        }
    }

    #[allow(dead_code)]
    pub fn with_pool(pool: Vec<AccountToken>) -> Self {
        Self {
            token_pool: Arc::new(RwLock::new(pool)),
            round_robin_counter: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub fn get_persistent_pool_path() -> PathBuf {
        PathBuf::from("data/token_pool.json")
    }

    pub fn get_token_path() -> PathBuf {
        if let Ok(val) = std::env::var("ANTIGRAVITY_TOKEN_PATH") {
            return PathBuf::from(val);
        }
        let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
        PathBuf::from(home).join(".gemini/antigravity-cli/antigravity-oauth-token")
    }

    pub async fn persist(&self) -> anyhow::Result<()> {
        let path = Self::get_persistent_pool_path();
        if let Some(parent) = path.parent() {
            let _ = tokio::fs::create_dir_all(parent).await;
        }
        let pool = self.token_pool.read().await;
        let stored: Vec<StoredAccountToken> = pool
            .iter()
            .map(|a| StoredAccountToken {
                id: a.id,
                label: a.label.clone(),
                email: a.email.clone(),
                token_json: a.token_json.clone(),
            })
            .collect();
        let json = serde_json::to_string_pretty(&stored)?;
        tokio::fs::write(&path, json).await?;
        Ok(())
    }

    pub fn load_initial_token_pool() -> Vec<AccountToken> {
        let mut pool = Vec::new();

        // 0. Check data/token_pool.json (Persistent Volume - survives container restart/reboot)
        let pool_path = Self::get_persistent_pool_path();
        if pool_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&pool_path) {
                if let Ok(stored) = serde_json::from_str::<Vec<StoredAccountToken>>(&content) {
                    for acc in stored {
                        if !acc.token_json.trim().is_empty() {
                            pool.push(AccountToken {
                                id: acc.id,
                                label: acc.label,
                                email: acc.email,
                                token_json: acc.token_json,
                                cooldown_until: Arc::new(RwLock::new(None)),
                            });
                        }
                    }
                }
            }
        }

        // 1. Check AINA_OAUTH_TOKENS (JSON array of token strings or objects)
        if pool.is_empty() {
            if let Ok(val) = std::env::var("AINA_OAUTH_TOKENS") {
                let trimmed = val.trim();
                if let Ok(parsed_arr) = serde_json::from_str::<Vec<serde_json::Value>>(trimmed) {
                    for (idx, item) in parsed_arr.into_iter().enumerate() {
                        let token_str = if item.is_string() {
                            item.as_str().unwrap().to_string()
                        } else {
                            item.to_string()
                        };
                        if !token_str.trim().is_empty() {
                            let email = extract_email_from_token(&token_str);
                            pool.push(AccountToken {
                                id: idx + 1,
                                label: format!("Account-{}", idx + 1),
                                email,
                                token_json: token_str,
                                cooldown_until: Arc::new(RwLock::new(None)),
                            });
                        }
                    }
                }
            }
        }

        // 2. Check individual environment variables AINA_OAUTH_TOKEN_1 ... AINA_OAUTH_TOKEN_20
        if pool.is_empty() {
            for i in 1..=20 {
                if let Ok(val) = std::env::var(format!("AINA_OAUTH_TOKEN_{}", i)) {
                    let trimmed = val.trim().to_string();
                    if !trimmed.is_empty() {
                        let email = extract_email_from_token(&trimmed);
                        pool.push(AccountToken {
                            id: i,
                            label: format!("Account-{}", i),
                            email,
                            token_json: trimmed,
                            cooldown_until: Arc::new(RwLock::new(None)),
                        });
                    }
                }
            }
        }

        // 3. Fallback to single environment variable AINA_OAUTH_TOKEN or ANTIGRAVITY_OAUTH_TOKEN
        if pool.is_empty() {
            if let Ok(val) = std::env::var("AINA_OAUTH_TOKEN").or_else(|_| std::env::var("ANTIGRAVITY_OAUTH_TOKEN")) {
                let trimmed = val.trim().to_string();
                if !trimmed.is_empty() {
                    let email = extract_email_from_token(&trimmed);
                    pool.push(AccountToken {
                        id: 1,
                        label: "Account-Primary".to_string(),
                        email,
                        token_json: trimmed,
                        cooldown_until: Arc::new(RwLock::new(None)),
                    });
                }
            }
        }

        // 4. Fallback to existing token file on disk if available
        if pool.is_empty() {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
            let default_path = PathBuf::from(&home).join(".gemini/antigravity-cli/antigravity-oauth-token");
            let auth_json_path = PathBuf::from(&home).join(".gemini/antigravity-cli/auth.json");
            let target_path = if default_path.exists() {
                Some(default_path)
            } else if auth_json_path.exists() {
                Some(auth_json_path)
            } else {
                None
            };
            if let Some(path) = target_path {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    let trimmed = content.trim().to_string();
                    if !trimmed.is_empty() {
                        let email = extract_email_from_token(&trimmed);
                        pool.push(AccountToken {
                            id: 1,
                            label: "Account-Default".to_string(),
                            email,
                            token_json: trimmed,
                            cooldown_until: Arc::new(RwLock::new(None)),
                        });
                    }
                }
            }
        }

        pool
    }

    pub async fn len(&self) -> usize {
        self.token_pool.read().await.len()
    }

    pub async fn is_authenticated(&self) -> bool {
        let pool = self.token_pool.read().await;
        if !pool.is_empty() {
            return true;
        }
        let token_path = Self::get_token_path();
        if token_path.exists() {
            return true;
        }
        let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
        let auth_json_path = PathBuf::from(home).join(".gemini/antigravity-cli/auth.json");
        auth_json_path.exists()
    }

    pub async fn select_next_token(&self, attempt: usize) -> Option<AccountToken> {
        let pool = self.token_pool.read().await.clone();
        if pool.is_empty() {
            return None;
        }

        let strategy = std::env::var("AINA_TOKEN_STRATEGY")
            .unwrap_or_else(|_| "round_robin".to_string())
            .to_lowercase();
        let is_round_robin = strategy == "round_robin" || strategy == "rr";

        let rr_start_idx = if is_round_robin {
            self.round_robin_counter.fetch_add(1, Ordering::Relaxed)
        } else {
            0
        };

        let now = std::time::Instant::now();
        let mut candidate = None;
        let n = pool.len();

        if is_round_robin {
            for i in 0..n {
                let idx = (rr_start_idx + attempt + i) % n;
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
        } else {
            for acc in &pool {
                let cd = acc.cooldown_until.read().await;
                if let Some(until) = *cd {
                    if now < until {
                        continue;
                    }
                }
                candidate = Some(acc.clone());
                break;
            }
        }

        let chosen = candidate.unwrap_or_else(|| pool[(rr_start_idx + attempt) % pool.len()].clone());

        // Write chosen token to disk for agy CLI binary
        let token_path = Self::get_token_path();
        if let Some(parent) = token_path.parent() {
            let _ = tokio::fs::create_dir_all(parent).await;
        }
        if let Err(e) = tokio::fs::write(&token_path, chosen.token_json.trim()).await {
            warn!("Failed to write active token for {}: {}", chosen.label, e);
        }

        Some(chosen)
    }

    pub async fn mark_cooldown(&self, account_id: usize, duration: Duration) {
        let pool = self.token_pool.read().await;
        if let Some(acc) = pool.iter().find(|a| a.id == account_id) {
            let cooldown_time = std::time::Instant::now() + duration;
            *acc.cooldown_until.write().await = Some(cooldown_time);
            info!(
                "Account #{} ({}) placed on cooldown for {}s",
                acc.id, acc.label, duration.as_secs()
            );
        }
    }

    pub async fn save_auth_token(&self, token_str: &str) -> anyhow::Result<()> {
        let trimmed = token_str.trim();
        if trimmed.is_empty() {
            anyhow::bail!("Token tidak boleh kosong.");
        }

        serde_json::from_str::<serde_json::Value>(trimmed)
            .map_err(|e| anyhow::anyhow!("Format token tidak valid (harus berupa JSON): {}", e))?;

        let path = Self::get_token_path();
        if let Some(parent) = path.parent() {
            let _ = tokio::fs::create_dir_all(parent).await;
        }
        tokio::fs::write(&path, trimmed).await?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o600);
            let _ = tokio::fs::set_permissions(&path, perms).await;
        }

        let new_email = extract_email_from_token(trimmed);

        // Synchronize in-memory token pool with smart deduplication
        {
            let mut pool = self.token_pool.write().await;
            let mut merged = false;

            if let Some(ref email) = new_email {
                if let Some(existing) = pool.iter_mut().find(|a| a.email.as_ref() == Some(email)) {
                    existing.token_json = trimmed.to_string();
                    *existing.cooldown_until.write().await = None;
                    info!("Account {} ({}) updated in pool with refreshed token", existing.label, email);
                    merged = true;
                }
            }

            if !merged {
                if let Some(existing) = pool.iter_mut().find(|a| a.token_json == trimmed) {
                    *existing.cooldown_until.write().await = None;
                    if existing.email.is_none() {
                        existing.email = new_email.clone();
                    }
                    info!("Account {} refreshed in pool", existing.label);
                    merged = true;
                }
            }

            if !merged {
                let next_id = pool.len() + 1;
                pool.push(AccountToken {
                    id: next_id,
                    label: format!("Account-{}", next_id),
                    email: new_email.clone(),
                    token_json: trimmed.to_string(),
                    cooldown_until: Arc::new(RwLock::new(None)),
                });
                info!("New account added to pool: Account-{} (email: {:?})", next_id, new_email);
            }
        }

        self.persist().await?;
        Ok(())
    }

    pub async fn remove_account(&self, account_id: usize) -> anyhow::Result<bool> {
        let removed = {
            let mut pool = self.token_pool.write().await;
            let initial_len = pool.len();
            pool.retain(|a| a.id != account_id);
            let removed = pool.len() < initial_len;
            if removed {
                for (idx, acc) in pool.iter_mut().enumerate() {
                    acc.id = idx + 1;
                    if acc.label.starts_with("Account-") && acc.label != "Account-Default" && acc.label != "Account-Primary" {
                        acc.label = format!("Account-{}", idx + 1);
                    }
                }
            }
            removed
        };

        if removed {
            self.persist().await?;
            info!("Removed account #{} from pool.", account_id);
        }

        Ok(removed)
    }

    pub async fn clear_account_pool(&self) -> anyhow::Result<usize> {
        let count = {
            let mut pool = self.token_pool.write().await;
            let count = pool.len().saturating_sub(1);
            if pool.len() > 1 {
                pool.truncate(1);
            }
            count
        };

        self.persist().await?;
        info!("Cleared {} secondary accounts from pool", count);
        Ok(count)
    }

    pub async fn get_account_pool_status(&self) -> Vec<crate::core::ports::AccountPoolStatus> {
        let pool = self.token_pool.read().await;
        let now = std::time::Instant::now();
        let mut res = Vec::new();
        for acc in pool.iter() {
            let cd = acc.cooldown_until.read().await;
            let (is_cooldown, remaining) = if let Some(until) = *cd {
                if now < until {
                    (true, (until - now).as_secs())
                } else {
                    (false, 0)
                }
            } else {
                (false, 0)
            };
            res.push(crate::core::ports::AccountPoolStatus {
                id: acc.id,
                label: acc.label.clone(),
                email: acc.email.clone(),
                is_cooldown,
                cooldown_remaining_secs: remaining,
            });
        }
        res
    }
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
