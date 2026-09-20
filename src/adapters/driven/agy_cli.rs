use crate::core::ports::{AgentEnginePort, AgentResponse};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;
use tokio::process::Command;
use tracing::{debug, error, info, warn};

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;

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

/// Normalizes and resolves model name aliases, strictly prioritizing Gemini as default
/// and allowing Claude Opus ONLY when explicitly requested.
pub fn resolve_model_name(raw: &str) -> anyhow::Result<String> {
    let lower = raw.trim().to_lowercase();
    match lower.as_str() {
        // Gemini 3.8 Family (Recommended Defaults)
        "gemini-3.8-flash-medium" | "flash-medium" | "medium" | "gemini-medium" | "gemini-flash-medium" | "default" => {
            Ok("gemini-3.8-flash-medium".to_string())
        }
        "gemini-3.8-flash-high" | "flash-high" | "high" | "gemini-high" | "gemini-flash-high" => {
            Ok("gemini-3.8-flash-high".to_string())
        }
        "gemini-3.8-flash-low" | "flash-low" | "low" | "gemini-low" | "gemini-flash-low" => {
            Ok("gemini-3.8-flash-low".to_string())
        }
        "flash" | "gemini-flash" => {
            Ok("gemini-3.8-flash-medium".to_string())
        }

        // Gemini 3.7 Family
        "gemini-3.7-flash-high" => Ok("gemini-3.7-flash-high".to_string()),
        "gemini-3.7-flash-medium" => Ok("gemini-3.7-flash-medium".to_string()),
        "gemini-3.7-flash-low" => Ok("gemini-3.7-flash-low".to_string()),

        // Gemini 3.6 Family
        "gemini-3.6-flash-high" => Ok("gemini-3.6-flash-high".to_string()),
        "gemini-3.6-flash-medium" => Ok("gemini-3.6-flash-medium".to_string()),
        "gemini-3.6-flash-low" => Ok("gemini-3.6-flash-low".to_string()),

        // Gemini 3.1 Pro (Deep Coding & Architecture)
        "gemini-3.1-pro-high" | "pro-high" | "pro" | "gemini-pro" => {
            Ok("gemini-3.1-pro-high".to_string())
        }
        "gemini-3.1-pro-low" | "pro-low" => {
            Ok("gemini-3.1-pro-low".to_string())
        }

        // Claude Sonnet
        "claude-sonnet-4-6" | "claude-sonnet" | "sonnet" => {
            Ok("claude-sonnet-4-6".to_string())
        }

        // Claude Opus (Strictly opt-in / explicitly requested)
        "claude-opus-4-6-thinking" | "claude-opus" | "opus" | "opus-thinking" => {
            Ok("claude-opus-4-6-thinking".to_string())
        }

        // GPT-OSS
        "gpt-oss-120b-medium" | "gpt-oss" => {
            Ok("gpt-oss-120b-medium".to_string())
        }

        other => {
            anyhow::bail!(
                "Model '{}' tidak didukung. Pilihan: gemini-3.8-flash-medium (default), gemini-3.8-flash-high, gemini-3.8-flash-low, gemini-3.1-pro-high, claude-opus-4-6-thinking, claude-sonnet-4-6.",
                other
            )
        }
    }
}

pub fn get_available_models() -> Vec<(&'static str, &'static str)> {
    vec![
        ("gemini-3.8-flash-medium", "Gemini 3.8 Flash (Medium) - Default Cepat & Seimbang"),
        ("gemini-3.8-flash-high", "Gemini 3.8 Flash (High) - Penalaran Tinggi / Deep Thinking"),
        ("gemini-3.8-flash-low", "Gemini 3.8 Flash (Low) - Respons Kilat & Kasual"),
        ("gemini-3.1-pro-high", "Gemini 3.1 Pro (High) - Deep Coding & Arsitektur"),
        ("claude-opus-4-6-thinking", "Claude Opus 4.6 (Thinking) - Khusus Tugas Kompleks Eksplisit"),
        ("claude-sonnet-4-6", "Claude Sonnet 4.6 (Thinking)"),
    ]
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredAccountToken {
    pub id: usize,
    pub label: String,
    pub email: Option<String>,
    pub token_json: String,
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
                if let Ok(decoded_bytes) = base64::engine::general_purpose::STANDARD.decode(&b64)
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

#[derive(Debug, Clone)]
pub struct AccountToken {
    #[allow(dead_code)]
    pub id: usize,
    pub label: String,
    pub email: Option<String>,
    pub token_json: String,
    pub cooldown_until: Arc<RwLock<Option<std::time::Instant>>>,
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
        let max_attempts = if pool.is_empty() { 1 } else { pool.len() };

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

            let is_quota = combined_lower.contains("503")
                || combined_lower.contains("429")
                || combined_lower.contains("quota")
                || combined_lower.contains("resource has been exhausted")
                || combined_lower.contains("resource_exhausted")
                || combined_lower.contains("rate limit")
                || combined_lower.contains("rate-limit")
                || combined_lower.contains("too many requests")
                || combined_lower.contains("exceeded your current quota")
                || combined_lower.contains("capacity");

            if !output.status.success() {
                if is_quota && pool.len() > 1 && attempt + 1 < max_attempts {
                    if let Some(ref acc) = active_acc {
                        let cooldown_dur = extract_quota_cooldown_duration(&combined_lower);
                        *acc.cooldown_until.write().await = Some(std::time::Instant::now() + cooldown_dur);
                        warn!(
                            "Account {} hit quota limit. Cooling down for {:?}. Failing over (attempt {}/{})...",
                            acc.label, cooldown_dur, attempt + 1, max_attempts
                        );
                        continue;
                    }
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
                        let is_err_quota = err.contains("503")
                            || err.contains("429")
                            || err.contains("quota")
                            || err.contains("exhausted");
                        if is_err_quota && pool.len() > 1 && attempt + 1 < max_attempts {
                            if let Some(ref acc) = active_acc {
                                *acc.cooldown_until.write().await = Some(std::time::Instant::now() + std::time::Duration::from_secs(300));
                                warn!(
                                    "Account {} returned quota error in JSON: {}. Cooling down for 5m. Failing over (attempt {}/{})...",
                                    acc.label, err, attempt + 1, max_attempts
                                );
                                continue;
                            }
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

        info!("Auth token saved to {:?}, verifying with quick test...", path);

        let test_res = self.execute(None, "Ping! Jawab 'PONG' saja.").await;
        match test_res {
            Ok(_) => {
                info!("Antigravity token verification succeeded!");
                Ok(())
            }
            Err(e) => {
                warn!("Verification test failed: {}", e);
                Err(anyhow::anyhow!("Token saved, but verification failed: {}", e))
            }
        }
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

        let output = cmd.output().await?;
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

pub fn get_oauth_session_dir(session_id: &str) -> PathBuf {
    let candidate_app = PathBuf::from(format!("/app/data/oauth_sessions/{}", session_id));
    let candidate_rel = PathBuf::from(format!("data/oauth_sessions/{}", session_id));
    let candidate_tmp = PathBuf::from(format!("/tmp/aina_oauth_{}", session_id));

    if candidate_app.exists() {
        candidate_app
    } else if candidate_rel.exists() {
        candidate_rel
    } else if candidate_tmp.exists() {
        candidate_tmp
    } else if std::path::Path::new("/app/data").is_dir() {
        candidate_app
    } else if std::path::Path::new("data").is_dir() || std::path::Path::new("Cargo.toml").exists() {
        candidate_rel
    } else {
        candidate_tmp
    }
}

fn simple_urldecode(s: &str) -> String {
    let mut res = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '%' {
            let h1 = chars.next();
            let h2 = chars.next();
            if let (Some(h1), Some(h2)) = (h1, h2) {
                let hex_str = format!("{}{}", h1, h2);
                if let Ok(byte) = u8::from_str_radix(&hex_str, 16) {
                    res.push(byte as char);
                    continue;
                } else {
                    res.push('%');
                    res.push(h1);
                    res.push(h2);
                    continue;
                }
            } else {
                res.push('%');
                if let Some(h1) = h1 { res.push(h1); }
                continue;
            }
        }
        res.push(c);
    }
    res
}

pub fn sanitize_oauth_code(raw: &str) -> String {
    let mut s = simple_urldecode(raw.trim());
    if let Some(idx) = s.find("code=") {
        s = s[idx + 5..].to_string();
    }
    for delim in &["&", "+http", " http", "userinfo.", "rinfo.", ".profile", "+", " "] {
        if let Some(idx) = s.find(delim) {
            s.truncate(idx);
        }
    }
    s.trim().to_string()
}

/// Sanitizes Antigravity CLI agent output by stripping out intermediate tool-waiting
/// logs, <SYSTEM_MESSAGE> blocks, and background task status lines that leak into multi-step JSON responses.
pub fn sanitize_agent_response(raw: &str) -> String {
    // 1. First, strip multi-line <SYSTEM_MESSAGE> blocks and system headers
    let stripped_system_blocks = strip_system_message_blocks(raw);

    let mut cleaned_lines = Vec::new();
    let mut skipping_header = true;

    for line in stripped_system_blocks.lines() {
        let trimmed = line.trim();

        let is_intermediate = is_intermediate_agent_status(trimmed);

        if skipping_header && is_intermediate {
            // Drop intermediate tool progress status at the beginning of the response
            continue;
        }

        if !trimmed.is_empty() && !is_intermediate {
            skipping_header = false;
        }

        if !is_intermediate {
            cleaned_lines.push(line);
        }
    }

    let result = cleaned_lines.join("\n").trim().to_string();
    if result.is_empty() {
        raw.trim().to_string()
    } else {
        result
    }
}

fn strip_system_message_blocks(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut cursor = 0;

    while cursor < text.len() {
        let slice = &text[cursor..];

        // Identify starting point of system message block
        let start_offset = if let Some(pos) = slice.find("The following is a <SYSTEM_MESSAGE>") {
            Some(pos)
        } else if let Some(pos) = slice.find("<SYSTEM_MESSAGE>") {
            Some(pos)
        } else if let Some(pos) = slice.find("<SYSTEM_MESSAGE") {
            Some(pos)
        } else {
            None
        };

        if let Some(start_pos) = start_offset {
            let abs_start = cursor + start_pos;
            result.push_str(&text[cursor..abs_start]);

            let after_start = &text[abs_start..];
            // Look for matching end tag: </SYSTEM_MESSAGE>} or </SYSTEM_MESSAGE>
            let end_offset = if let Some(pos) = after_start.find("</SYSTEM_MESSAGE>}") {
                Some(pos + "</SYSTEM_MESSAGE>}".len())
            } else if let Some(pos) = after_start.find("</SYSTEM_MESSAGE>") {
                Some(pos + "</SYSTEM_MESSAGE>".len())
            } else {
                None
            };

            if let Some(end_len) = end_offset {
                cursor = abs_start + end_len;
            } else {
                // If no closing tag found, skip the rest of this system message block
                break;
            }
        } else {
            result.push_str(slice);
            break;
        }
    }

    result
}

fn is_intermediate_agent_status(line: &str) -> bool {
    let lower = line.to_lowercase();

    // 1. Task waiting patterns
    if (lower.starts_with("i am waiting for ") || lower.starts_with("waiting for "))
        && (lower.contains("task") || lower.contains("finish") || lower.contains("complete"))
    {
        return true;
    }

    // 2. Indonesian task waiting patterns
    if lower.starts_with("sedang menunggu ")
        && (lower.contains("task") || lower.contains("selesai") || lower.contains("proses kompilasi"))
    {
        return true;
    }

    // 3. Task transition patterns
    if lower.starts_with("the task has almost completed")
        || lower.starts_with("let me inspect the final output")
        || lower.starts_with("tool is running as a background task")
        || lower.starts_with("task logs are available at:")
        || lower.starts_with("you must take one of the following two actions:")
    {
        return true;
    }

    // 4. Background task completion logs & system artifacts
    if lower.starts_with("task id \"") && lower.contains("\" finished with result") {
        return true;
    }
    if lower.starts_with("the command exited with code")
        || lower.starts_with("terminal id:")
        || lower.starts_with("log: file:///")
        || lower.starts_with("[message] timestamp=")
        || lower.starts_with("the following is a <system_message>")
        || lower.starts_with("<system_message")
        || lower.starts_with("</system_message")
    {
        return true;
    }

    // 5. Tool execution confirmation and diagnostic message leakages
    if (lower.starts_with("status: terkirim") || lower.starts_with("status: berhasil"))
        && (lower.contains("message_id") || lower.contains("3eb0"))
    {
        return true;
    }
    if lower.starts_with("pengingat sudah berhasil dikirimkan ke whatsapp")
        || lower.starts_with("pesan sudah berhasil dikirimkan ke whatsapp")
        || lower.starts_with("pesan telah berhasil dikirimkan ke whatsapp")
    {
        return true;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_agent_response_strips_intermediate_tasks() {
        let raw = r#"The task has almost completed, let me inspect the final output.
I am waiting for the metadata task to finish.
I am waiting for task-907 to complete.
I am waiting for task-914 to complete.
Bisa bangeett, Bang Ihzaa! Ini solusi yang pas banget supaya data hasil konfirmasi petugas di lapangan lewat AppSheet bisa langsung mengalir otomatis ke 9 spreadsheet sumber utama KCDA."#;

        let cleaned = sanitize_agent_response(raw);
        assert!(!cleaned.contains("task-907"));
        assert!(!cleaned.contains("The task has almost completed"));
        assert!(!cleaned.contains("metadata task to finish"));
        assert!(cleaned.starts_with("Bisa bangeett, Bang Ihzaa!"));
    }

    #[test]
    fn test_sanitize_agent_response_preserves_legitimate_waiting_text() {
        let raw = "Kami sedang menunggu konfirmasi resmi dari BPS terkait jadwal rilis KCDA.";
        let cleaned = sanitize_agent_response(raw);
        assert_eq!(cleaned, raw);
    }

    #[test]
    fn test_sanitize_agent_response_strips_system_message_blocks() {
        let raw = r#"The following is a <SYSTEM_MESSAGE> not actually sent by the user. It is provided by the system as important information to pay attention to.

<SYSTEM_MESSAGE>
[Message] timestamp=2026-09-16T13:17:10Z sender=f222b011-5303-48b6-90c7-edf593c884b5/task-1974 priority=MESSAGE_PRIORITY_HIGH content=Task id "f222b011-5303-48b6-90c7-edf593c884b5/task-1974" finished with result:

The command exited with code 0.
Output:
-rw-r--r-- 1 root root 224855 Sep 16 20:17 /tmp/monitoring_pml_wb2.png

Terminal ID: term_chrome

Log: file:///root/.gemini/antigravity-cli/brain/f222b011-5303-48b6-90c7-edf593c884b5/.system_generated/tasks/task-1974.log
</SYSTEM_MESSAGE>}
Ini yaa Bang Ihza @Ihza Karunia! Gambarnya barusan sudah langsung Aina kirimkan ke atas.

Tangkapan layar tersebut diambil langsung menggunakan browser headless bawaan pada tab *Perpml*."#;

        let cleaned = sanitize_agent_response(raw);
        assert!(!cleaned.contains("<SYSTEM_MESSAGE>"));
        assert!(!cleaned.contains("The following is a <SYSTEM_MESSAGE>"));
        assert!(!cleaned.contains("task-1974"));
        assert!(!cleaned.contains("term_chrome"));
        assert!(cleaned.starts_with("Ini yaa Bang Ihza @Ihza Karunia!"));
        assert!(cleaned.contains("Tangkapan layar tersebut diambil langsung"));
    }

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
    fn test_sanitize_oauth_code() {
        // 1. Clean code
        assert_eq!(
            sanitize_oauth_code("4/0ATsMZqCyxR6mxx8ph9vz1TH9kw"),
            "4/0ATsMZqCyxR6mxx8ph9vz1TH9kw"
        );

        // 2. User contaminated string with query parameters
        let contaminated = "4/0ATsMZqCyxR6mxx8ph9vz1TH9kw-WjiV4m2f1zhXeJu_iPCcCRmmHdVMAdwaRKCBg7Jbs9Arinfo.profile+https%3A%2F%2Fwww.googleapis.com%2Fauth%2Fcclog+https%3A%2F%2Fwww.googleapis.com%2Fauth%2Fexperimentsandconfigs+https%3A%2F%2Fwww.googleapis.com%2Fauth%2Fcloud-platform&authuser=0&prompt=consent";
        assert_eq!(
            sanitize_oauth_code(contaminated),
            "4/0ATsMZqCyxR6mxx8ph9vz1TH9kw-WjiV4m2f1zhXeJu_iPCcCRmmHdVMAdwaRKCBg7Jbs9A"
        );

        // 3. Full callback URL
        let url = "https://antigravity.google/oauth-callback?code=4/0ATsMZqCyxR6mxx8ph9vz1TH9kw&scope=email+profile";
        assert_eq!(
            sanitize_oauth_code(url),
            "4/0ATsMZqCyxR6mxx8ph9vz1TH9kw"
        );

        // 4. URL encoded code parameter
        let url_encoded = "code=4%2F0ATsMZqCyxR6mxx8ph9vz1TH9kw&state=xyz";
        assert_eq!(
            sanitize_oauth_code(url_encoded),
            "4/0ATsMZqCyxR6mxx8ph9vz1TH9kw"
        );
    }
}
