//! Antigravity CLI Process Runner and AgentEnginePort adapter implementation.

use crate::core::ports::{AgentEnginePort, AgentResponse};
use async_trait::async_trait;
use serde::Deserialize;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::process::Command;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use super::account_pool::{
    extract_email_from_token, extract_quota_cooldown_duration, is_auth_error, is_quota_error,
    is_transient_error, AccountToken, StoredAccountToken,
};
use super::models::resolve_model_name;
use super::oauth::{get_oauth_session_dir, sanitize_oauth_code};
use super::sanitizer::sanitize_agent_response;

#[derive(Debug, Deserialize)]
struct AgyJsonOutput {
    pub conversation_id: String,
    pub status: String,
    #[serde(default)]
    pub response: String,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub duration_seconds: f64,
}

pub struct AntigravityCliAdapter {
    binary_path: PathBuf,
    model: Arc<RwLock<String>>,
    workspace_dir: PathBuf,
    timeout_duration: Duration,
    whatsmeow_base_url: String,
    whatsmeow_api_key: String,
    companion_base_url: Option<String>,
    companion_api_key: Option<String>,
    token_pool: Arc<RwLock<Vec<AccountToken>>>,
    round_robin_counter: Arc<AtomicUsize>,
}

impl AntigravityCliAdapter {
    #[allow(dead_code)]
    pub fn new(
        binary_path: impl Into<PathBuf>,
        model: impl Into<String>,
        workspace_dir: impl Into<PathBuf>,
        timeout_seconds: u64,
        whatsmeow_base_url: impl Into<String>,
        whatsmeow_api_key: impl Into<String>,
    ) -> Self {
        Self::with_companion(
            binary_path,
            model,
            workspace_dir,
            timeout_seconds,
            whatsmeow_base_url,
            whatsmeow_api_key,
            None,
            None,
        )
    }

    pub fn with_companion(
        binary_path: impl Into<PathBuf>,
        model: impl Into<String>,
        workspace_dir: impl Into<PathBuf>,
        timeout_seconds: u64,
        whatsmeow_base_url: impl Into<String>,
        whatsmeow_api_key: impl Into<String>,
        companion_base_url: Option<String>,
        companion_api_key: Option<String>,
    ) -> Self {
        let raw_model = model.into();
        let initial_model = resolve_model_name(&raw_model).unwrap_or(raw_model);
        let pool = Self::load_initial_token_pool();
        if !pool.is_empty() {
            info!("Initialized Antigravity account pool with {} account(s)", pool.len());
        }
        Self {
            binary_path: binary_path.into(),
            model: Arc::new(RwLock::new(initial_model)),
            workspace_dir: workspace_dir.into(),
            timeout_duration: Duration::from_secs(timeout_seconds),
            whatsmeow_base_url: whatsmeow_base_url.into(),
            whatsmeow_api_key: whatsmeow_api_key.into(),
            companion_base_url,
            companion_api_key,
            token_pool: Arc::new(RwLock::new(pool)),
            round_robin_counter: Arc::new(AtomicUsize::new(0)),
        }
    }

    fn get_persistent_pool_path() -> PathBuf {
        PathBuf::from("data/token_pool.json")
    }

    async fn persist_token_pool(pool: &[AccountToken]) -> anyhow::Result<()> {
        let path = Self::get_persistent_pool_path();
        if let Some(parent) = path.parent() {
            let _ = tokio::fs::create_dir_all(parent).await;
        }
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

    fn load_initial_token_pool() -> Vec<AccountToken> {
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

    fn get_token_path(&self) -> PathBuf {
        if let Ok(val) = std::env::var("ANTIGRAVITY_TOKEN_PATH") {
            return PathBuf::from(val);
        }
        let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
        PathBuf::from(home).join(".gemini/antigravity-cli/antigravity-oauth-token")
    }

    /// Intelligently resolves the Antigravity CLI binary across Linux, Termux, Docker, and macOS
    pub fn resolve_binary(&self) -> PathBuf {
        // 1. If explicit binary path exists and is an executable file
        if self.binary_path.exists() && self.binary_path.is_file() {
            return self.binary_path.clone();
        }

        // 2. Check AGENT_BINARY_PATH or AGY_BINARY_PATH environment variables
        if let Ok(env_path) = std::env::var("AGENT_BINARY_PATH").or_else(|_| std::env::var("AGY_BINARY_PATH")) {
            let p = PathBuf::from(env_path);
            if p.exists() && p.is_file() {
                return p;
            }
        }

        // 3. Search standard Antigravity CLI installation paths
        let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
        let candidates = [
            PathBuf::from("/usr/local/bin/agy"),
            PathBuf::from("/root/.local/bin/agy"),
            PathBuf::from(&home).join(".local/bin/agy"),
            PathBuf::from("/data/data/com.termux/files/usr/bin/agy"),
        ];

        for cand in &candidates {
            if cand.exists() && cand.is_file() {
                return cand.clone();
            }
        }

        // 4. Fallback to system PATH lookup
        if let Some(file_name) = self.binary_path.file_name() {
            PathBuf::from(file_name)
        } else {
            PathBuf::from("agy")
        }
    }
}

#[async_trait]
impl AgentEnginePort for AntigravityCliAdapter {
    async fn execute_with_model(
        &self,
        conversation_id: Option<&str>,
        prompt: &str,
        model_override: Option<&str>,
    ) -> anyhow::Result<AgentResponse> {
        let active_model = match model_override {
            Some(m) if !m.trim().is_empty() => resolve_model_name(m)?,
            _ => self.model.read().await.clone(),
        };

        let bin_path = self.resolve_binary();
        
        // Ensure workspace directory exists
        if !self.workspace_dir.exists() {
            tokio::fs::create_dir_all(&self.workspace_dir).await?;
        }

        let pool = self.token_pool.read().await.clone();
        let max_attempts = if pool.is_empty() { 2 } else { pool.len().max(2) };

        let strategy = std::env::var("AINA_TOKEN_STRATEGY")
            .unwrap_or_else(|_| "round_robin".to_string())
            .to_lowercase();
        let is_round_robin = strategy == "round_robin" || strategy == "rr";

        // For round-robin, increment counter once at start of request
        let rr_start_idx = if is_round_robin && !pool.is_empty() {
            self.round_robin_counter.fetch_add(1, Ordering::Relaxed)
        } else {
            0
        };

        for attempt in 0..max_attempts {
            let active_acc = if !pool.is_empty() {
                let now = std::time::Instant::now();
                let mut candidate = None;
                let n = pool.len();

                if is_round_robin {
                    // Try each account starting from (rr_start_idx + attempt) % n
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
                    // Sticky / Priority: always try from index 0 unless in cooldown
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

                let token_path = self.get_token_path();
                if let Some(parent) = token_path.parent() {
                    let _ = tokio::fs::create_dir_all(parent).await;
                }
                if let Err(e) = tokio::fs::write(&token_path, chosen.token_json.trim()).await {
                    warn!("Failed to write active token for {}: {}", chosen.label, e);
                }
                Some(chosen)
            } else {
                None
            };

            let mut cmd = Command::new(&bin_path);
            cmd.kill_on_drop(true);
            cmd.current_dir(&self.workspace_dir);

            // Add conversation flag if continuing a session
            if let Some(conv_id) = conversation_id {
                if !conv_id.trim().is_empty() {
                    cmd.arg("--conversation").arg(conv_id);
                }
            }

            // Add headless execution flags
            let is_unlimited = self.timeout_duration.is_zero() || self.timeout_duration.as_secs() >= 86400;
            let print_timeout_str = if is_unlimited {
                "24h".to_string()
            } else {
                let print_timeout_sec = self.timeout_duration.as_secs().saturating_sub(5).max(30);
                format!("{}s", print_timeout_sec)
            };
            cmd.arg("-p").arg(prompt);
            cmd.arg("--output-format").arg("json");
            cmd.arg("--print-timeout").arg(&print_timeout_str);
            cmd.arg("--dangerously-skip-permissions");
            cmd.arg("--model").arg(&active_model);

            // Inject live WhatsApp gateway connection variables so CLI tools (wa_tool.py) work seamlessly
            cmd.env("WHATSMEOW_BASE_URL", &self.whatsmeow_base_url);
            cmd.env("WHATSMEOW_URL", &self.whatsmeow_base_url);
            cmd.env("WHATSMEOW_API_KEY", &self.whatsmeow_api_key);
            cmd.env("API_KEY", &self.whatsmeow_api_key);

            if let Some(ref comp_url) = self.companion_base_url {
                cmd.env("WHATSMEOW_COMPANION_BASE_URL", comp_url);
                cmd.env("WHATSMEOW_COMPANION_URL", comp_url);
            }
            if let Some(ref comp_key) = self.companion_api_key {
                cmd.env("WHATSMEOW_COMPANION_API_KEY", comp_key);
            }

            debug!(
                "Executing Antigravity CLI (attempt {}/{}): {:?} (conv: {:?}, model: {}, acc: {:?}, unlimited: {})",
                attempt + 1, max_attempts, bin_path, conversation_id, active_model, active_acc.as_ref().map(|a| &a.label), is_unlimited
            );

            // Run process with safe ceiling (kill_on_drop will terminate child if timeout occurs)
            let effective_timeout = if is_unlimited {
                // Interactive safe timeout ceiling (10 minutes max to prevent zombie process hang)
                Duration::from_secs(600)
            } else {
                self.timeout_duration
            };

            let output = match tokio::time::timeout(effective_timeout, cmd.output()).await {
                Ok(res) => res?,
                Err(_) => {
                    anyhow::bail!(
                        "Antigravity CLI execution timed out after {:?}",
                        effective_timeout
                    );
                }
            };

            let stdout_raw = String::from_utf8_lossy(&output.stdout);
            let stderr_raw = String::from_utf8_lossy(&output.stderr);
            let combined = format!("{}\n{}", stderr_raw, stdout_raw);
            let combined_lower = combined.to_lowercase();

            let is_quota_detected = is_quota_error(&combined_lower);
            let is_auth_detected = is_auth_error(&combined_lower);
            let is_transient_detected = is_transient_error(&combined_lower);

            if !output.status.success() {
                if (is_quota_detected || is_auth_detected) && pool.len() > 1 && attempt + 1 < max_attempts {
                    if let Some(ref acc) = active_acc {
                        let cooldown_dur = if is_auth_detected {
                            std::time::Duration::from_secs(6 * 3600)
                        } else {
                            extract_quota_cooldown_duration(&combined_lower)
                        };
                        *acc.cooldown_until.write().await = Some(std::time::Instant::now() + cooldown_dur);
                        warn!(
                            "Account {} encountered {} (cooling down for {:?}). Failing over (attempt {}/{})...",
                            acc.label,
                            if is_auth_detected { "eligibility/auth checkpoint" } else { "quota limit" },
                            cooldown_dur,
                            attempt + 1,
                            max_attempts
                        );
                        continue;
                    }
                }

                if is_transient_detected && attempt + 1 < max_attempts {
                    warn!(
                        "Antigravity CLI encountered transient stream/network glitch (attempt {}/{}): {}. Backing off and retrying...",
                        attempt + 1,
                        max_attempts,
                        stderr_raw.lines().next().unwrap_or(&stderr_raw)
                    );
                    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
                    continue;
                }

                error!(
                    "Antigravity CLI failed with code {:?}. Stderr: {}",
                    output.status.code(),
                    stderr_raw
                );
                anyhow::bail!("Antigravity CLI failed: {}", stderr_raw);
            }

            if !stderr_raw.is_empty() {
                debug!("Antigravity CLI stderr: {}", stderr_raw);
            }

            // Find and parse the JSON block in stdout
            let json_line = stdout_raw
                .lines()
                .rev()
                .find(|line| {
                    let trimmed = line.trim();
                    trimmed.starts_with('{') && trimmed.ends_with('}')
                })
                .or_else(|| {
                    let first = stdout_raw.find('{')?;
                    let last = stdout_raw.rfind('}')?;
                    if first < last {
                        Some(&stdout_raw[first..=last])
                    } else {
                        None
                    }
                });

            if let Some(payload) = json_line {
                let parsed: AgyJsonOutput = serde_json::from_str(payload)?;
                info!(
                    "Agent turn finished: conv={}, status={}, model={}",
                    parsed.conversation_id, parsed.status, active_model
                );

                if let Some(ref err) = parsed.error {
                    if parsed.response.trim().is_empty() {
                        let is_err_quota = is_quota_error(err);
                        let is_err_auth = is_auth_error(err);
                        let is_err_transient = is_transient_error(err);

                        if (is_err_quota || is_err_auth) && pool.len() > 1 && attempt + 1 < max_attempts {
                            if let Some(ref acc) = active_acc {
                                let cd_secs = if is_err_auth { 6 * 3600 } else { 300 };
                                *acc.cooldown_until.write().await = Some(std::time::Instant::now() + std::time::Duration::from_secs(cd_secs));
                                warn!(
                                    "Account {} returned {} in JSON: {}. Cooling down for {}s. Failing over (attempt {}/{})...",
                                    acc.label,
                                    if is_err_auth { "auth/eligibility error" } else { "quota error" },
                                    err,
                                    cd_secs,
                                    attempt + 1,
                                    max_attempts
                                );
                                continue;
                            }
                        }

                        if is_err_transient && attempt + 1 < max_attempts {
                            warn!(
                                "Antigravity CLI returned transient stream/network error in JSON (attempt {}/{}): {}. Retrying...",
                                attempt + 1,
                                max_attempts,
                                err
                            );
                            tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
                            continue;
                        }
                        error!("Antigravity agent failed with error in payload: {}", err);
                        anyhow::bail!("Antigravity agent error: {}", err);
                    }
                }

                let response_text = sanitize_agent_response(parsed.response.trim());
                return Ok(AgentResponse {
                    conversation_id: parsed.conversation_id,
                    response_text,
                    duration_seconds: parsed.duration_seconds,
                });
            } else {
                return Ok(AgentResponse {
                    conversation_id: conversation_id.unwrap_or_default().to_string(),
                    response_text: sanitize_agent_response(stdout_raw.trim()),
                    duration_seconds: 0.0,
                });
            }
        }

        anyhow::bail!("All accounts in token pool failed or exhausted quota.")
    }

    async fn get_model(&self) -> String {
        self.model.read().await.clone()
    }

    async fn set_model(&self, model: &str) -> anyhow::Result<()> {
        let validated = resolve_model_name(model)?;
        let mut lock = self.model.write().await;
        *lock = validated;
        Ok(())
    }

    async fn is_authenticated(&self) -> bool {
        let path = self.get_token_path();
        if path.exists() {
            if let Ok(meta) = tokio::fs::metadata(&path).await {
                return meta.len() > 10;
            }
        }
        false
    }

    async fn save_auth_token(&self, token_content: &str) -> anyhow::Result<()> {
        let trimmed = token_content.trim();
        let _parsed: serde_json::Value = serde_json::from_str(trimmed)
            .map_err(|e| anyhow::anyhow!("Token must be valid JSON: {}", e))?;

        let path = self.get_token_path();
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
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
        let pool_snapshot = {
            let mut pool = self.token_pool.write().await;
            let mut merged = false;

            // Check if account with same email already exists in pool
            if let Some(ref email) = new_email {
                if let Some(existing) = pool.iter_mut().find(|a| a.email.as_ref() == Some(email)) {
                    existing.token_json = trimmed.to_string();
                    *existing.cooldown_until.write().await = None;
                    info!("Account {} ({}) updated in pool with refreshed token", existing.label, email);
                    merged = true;
                }
            }

            // Fallback check: same token_json
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

            pool.clone()
        };

        // Persist token pool to data/token_pool.json so it survives container restart / power outage
        if let Err(e) = Self::persist_token_pool(&pool_snapshot).await {
            warn!("Failed to persist token pool to data/token_pool.json: {}", e);
        }

        info!(
            "Auth token saved to {:?} and registered in pool successfully for account {:?}",
            path, new_email
        );
        Ok(())
    }

    async fn remove_account(&self, account_id: usize) -> anyhow::Result<bool> {
        let (removed, pool_snapshot) = {
            let mut pool = self.token_pool.write().await;
            let initial_len = pool.len();
            pool.retain(|a| a.id != account_id);
            let removed = pool.len() < initial_len;
            if removed {
                // Re-index remaining accounts
                for (idx, acc) in pool.iter_mut().enumerate() {
                    acc.id = idx + 1;
                    if acc.label.starts_with("Account-") && acc.label != "Account-Default" && acc.label != "Account-Primary" {
                        acc.label = format!("Account-{}", idx + 1);
                    }
                }
            }
            (removed, pool.clone())
        };

        if removed {
            if let Err(e) = Self::persist_token_pool(&pool_snapshot).await {
                warn!("Failed to update persistent token pool after removal: {}", e);
            }
            info!("Removed account #{} from pool. Remaining: {}", account_id, pool_snapshot.len());
        }

        Ok(removed)
    }

    async fn clear_account_pool(&self) -> anyhow::Result<usize> {
        let (count, pool_snapshot) = {
            let mut pool = self.token_pool.write().await;
            let count = pool.len().saturating_sub(1);
            if pool.len() > 1 {
                pool.truncate(1);
            }
            (count, pool.clone())
        };

        if let Err(e) = Self::persist_token_pool(&pool_snapshot).await {
            warn!("Failed to update persistent token pool after clearing: {}", e);
        }
        info!("Cleared {} secondary accounts from pool", count);

        Ok(count)
    }

    async fn get_account_pool_status(&self) -> Vec<crate::core::ports::AccountPoolStatus> {
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

    async fn init_oauth_session(&self) -> anyhow::Result<(String, String)> {
        let session_id = format!(
            "{}_{:08x}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0),
            rand::random::<u32>()
        );
        let base_dir = get_oauth_session_dir(&session_id);
        let _ = tokio::fs::create_dir_all(&base_dir).await;

        let script_candidates = [
            PathBuf::from("scripts/oauth_helper.py"),
            PathBuf::from("/root/projects/aina/scripts/oauth_helper.py"),
            PathBuf::from("/app/scripts/oauth_helper.py"),
        ];
        let script_path = script_candidates
            .into_iter()
            .find(|p| p.exists())
            .unwrap_or_else(|| PathBuf::from("scripts/oauth_helper.py"));

        let bin_path = self.resolve_binary();

        let mut cmd = tokio::process::Command::new("python3");
        cmd.arg(&script_path)
            .arg(&session_id)
            .env("AGY_BINARY_PATH", bin_path.to_string_lossy().to_string());

        let mut child = cmd.spawn()?;
        tokio::spawn(async move {
            let _ = child.wait().await;
        });

        let url_file = base_dir.join("auth_url.txt");
        let status_file = base_dir.join("status.txt");
        let start = std::time::Instant::now();
        loop {
            if let Ok(url) = tokio::fs::read_to_string(&url_file).await {
                let trimmed = url.trim().to_string();
                if !trimmed.is_empty() {
                    return Ok((session_id, trimmed));
                }
            }
            if let Ok(status) = tokio::fs::read_to_string(&status_file).await {
                let s = status.trim();
                if s == "FAILED" || s == "TIMEOUT" {
                    let err = tokio::fs::read_to_string(base_dir.join("error.txt"))
                        .await
                        .unwrap_or_else(|_| "Gagal menghasilkan URL OAuth".to_string());
                    let _ = tokio::fs::remove_dir_all(&base_dir).await;
                    anyhow::bail!("Gagal memulai sesi login Google: {}", err);
                }
            }
            if start.elapsed().as_secs() > 10 {
                let _ = tokio::fs::remove_dir_all(&base_dir).await;
                anyhow::bail!("Timeout menunggu URL login Google dari CLI Antigravity");
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;
        }
    }

    async fn exchange_oauth_code(&self, session_id: &str, code: &str) -> anyhow::Result<String> {
        let base_dir = get_oauth_session_dir(session_id);
        if !tokio::fs::try_exists(&base_dir).await.unwrap_or(false) {
            anyhow::bail!(
                "Sesi login '{}' tidak ditemukan atau telah kadaluarsa. Silakan klik 'Mulai Ulang / Akun Lain'.",
                session_id
            );
        }

        let clean_code = sanitize_oauth_code(code);

        let script_candidates = [
            PathBuf::from("scripts/oauth_helper.py"),
            PathBuf::from("/root/projects/aina/scripts/oauth_helper.py"),
            PathBuf::from("/app/scripts/oauth_helper.py"),
        ];
        let script_path = script_candidates
            .into_iter()
            .find(|p| p.exists())
            .unwrap_or_else(|| PathBuf::from("scripts/oauth_helper.py"));

        // Direct sub-second PKCE token exchange
        let mut cmd = tokio::process::Command::new("python3");
        cmd.arg(&script_path)
            .arg("exchange")
            .arg(session_id)
            .arg(&clean_code);

        let output = match tokio::time::timeout(std::time::Duration::from_secs(15), cmd.output()).await {
            Ok(res) => res?,
            Err(_) => {
                let _ = tokio::fs::remove_dir_all(&base_dir).await;
                anyhow::bail!("Timeout saat menukar kode otorisasi ke Google (15 detik). Silakan coba lagi.");
            }
        };
        let token_file = base_dir.join("token.json");
        let error_file = base_dir.join("error.txt");

        if output.status.success() && tokio::fs::try_exists(&token_file).await.unwrap_or(false) {
            let token_content = tokio::fs::read_to_string(&token_file).await?;
            let _ = tokio::fs::remove_dir_all(&base_dir).await;

            self.save_auth_token(&token_content).await?;
            let email = extract_email_from_token(&token_content)
                .unwrap_or_else(|| "Akun Baru".to_string());
            return Ok(email);
        }

        let err = if let Ok(err_str) = tokio::fs::read_to_string(&error_file).await {
            err_str
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            if stderr.is_empty() {
                "Verifikasi kode otorisasi gagal atau ditolak oleh Google.".to_string()
            } else {
                stderr
            }
        };
        let _ = tokio::fs::remove_dir_all(&base_dir).await;
        anyhow::bail!("{}", err.trim());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_pool_remove_and_clear() {
        let adapter = AntigravityCliAdapter::new(
            "/bin/true",
            "gemini-2.5-flash",
            "/tmp",
            60,
            "http://localhost:8080",
            "secret",
        );

        {
            let mut pool = adapter.token_pool.write().await;
            pool.clear();
            pool.push(AccountToken {
                id: 1,
                label: "Account-Default".to_string(),
                email: Some("default@gmail.com".to_string()),
                token_json: "tok1".to_string(),
                cooldown_until: Arc::new(RwLock::new(None)),
            });
            pool.push(AccountToken {
                id: 2,
                label: "Account-2".to_string(),
                email: Some("second@gmail.com".to_string()),
                token_json: "tok2".to_string(),
                cooldown_until: Arc::new(RwLock::new(None)),
            });
            pool.push(AccountToken {
                id: 3,
                label: "Account-3".to_string(),
                email: Some("third@gmail.com".to_string()),
                token_json: "tok3".to_string(),
                cooldown_until: Arc::new(RwLock::new(None)),
            });
        }

        // Test remove Account #2
        let removed = adapter.remove_account(2).await.unwrap();
        assert!(removed);

        let status = adapter.get_account_pool_status().await;
        assert_eq!(status.len(), 2);
        assert_eq!(status[0].id, 1);
        assert_eq!(status[0].label, "Account-Default");
        assert_eq!(status[1].id, 2);
        assert_eq!(status[1].label, "Account-2"); // Reindexed from 3 to 2

        // Test clear (retains primary account)
        let cleared = adapter.clear_account_pool().await.unwrap();
        assert_eq!(cleared, 1);

        let status = adapter.get_account_pool_status().await;
        assert_eq!(status.len(), 1);
        assert_eq!(status[0].id, 1);
        assert_eq!(status[0].label, "Account-Default");
    }
}
