//! Antigravity CLI Process Runner and AgentEnginePort adapter implementation.

use crate::core::ports::{AgentEnginePort, AgentResponse};
use async_trait::async_trait;
use serde::Deserialize;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::process::Command;
use tokio::sync::RwLock;
use tracing::{error, warn};

use super::account_pool::{
    extract_email_from_token, extract_quota_cooldown_duration, is_auth_error, is_quota_error,
    is_transient_error, TokenPoolManager,
};
use super::models::resolve_model_name;
use super::oauth;
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
    pool_manager: Arc<TokenPoolManager>,
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
        Self {
            binary_path: binary_path.into(),
            model: Arc::new(RwLock::new(initial_model)),
            workspace_dir: workspace_dir.into(),
            timeout_duration: Duration::from_secs(timeout_seconds),
            whatsmeow_base_url: whatsmeow_base_url.into(),
            whatsmeow_api_key: whatsmeow_api_key.into(),
            companion_base_url,
            companion_api_key,
            pool_manager: Arc::new(TokenPoolManager::new()),
        }
    }

    #[allow(dead_code)]
    pub fn with_pool_manager(
        binary_path: impl Into<PathBuf>,
        model: impl Into<String>,
        workspace_dir: impl Into<PathBuf>,
        timeout_seconds: u64,
        whatsmeow_base_url: impl Into<String>,
        whatsmeow_api_key: impl Into<String>,
        pool_manager: Arc<TokenPoolManager>,
    ) -> Self {
        let raw_model = model.into();
        let initial_model = resolve_model_name(&raw_model).unwrap_or(raw_model);
        Self {
            binary_path: binary_path.into(),
            model: Arc::new(RwLock::new(initial_model)),
            workspace_dir: workspace_dir.into(),
            timeout_duration: Duration::from_secs(timeout_seconds),
            whatsmeow_base_url: whatsmeow_base_url.into(),
            whatsmeow_api_key: whatsmeow_api_key.into(),
            companion_base_url: None,
            companion_api_key: None,
            pool_manager,
        }
    }

    /// Intelligently resolves the Antigravity CLI binary across Linux, Termux, Docker, and macOS
    pub fn resolve_binary(&self) -> PathBuf {
        // 1. If explicit binary path exists and is an executable file
        if self.binary_path.exists() && self.binary_path.is_file() {
            return self.binary_path.clone();
        }

        // 2. Check override environment variable
        if let Ok(val) = std::env::var("AGY_BINARY_PATH") {
            let p = PathBuf::from(val);
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

        let pool_len = self.pool_manager.len().await;
        let max_attempts = if pool_len == 0 { 2 } else { pool_len.max(2) };

        for attempt in 0..max_attempts {
            let active_acc = self.pool_manager.select_next_token(attempt).await;

            let mut cmd = Command::new(&bin_path);
            cmd.kill_on_drop(true);
            cmd.current_dir(&self.workspace_dir);

            if let Some(conv_id) = conversation_id {
                if !conv_id.trim().is_empty() {
                    cmd.arg("--conversation").arg(conv_id);
                }
            }

            let effort_tier = if active_model.ends_with("-high") || active_model.contains("-high") {
                Some("high")
            } else if active_model.ends_with("-low") || active_model.contains("-low") {
                Some("low")
            } else if active_model.ends_with("-medium") || active_model.contains("-medium") {
                Some("medium")
            } else if active_model.contains("thinking") {
                Some("high")
            } else {
                None
            };

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
            if let Some(eff) = effort_tier {
                cmd.arg("--effort").arg(eff);
            }

            // Injected environment variables for skills & tools
            cmd.env("WHATSMEOW_BASE_URL", &self.whatsmeow_base_url);
            cmd.env("WHATSMEOW_API_KEY", &self.whatsmeow_api_key);
            if let Some(ref comp_url) = self.companion_base_url {
                cmd.env("WHATSMEOW_COMPANION_URL", comp_url);
            }
            if let Some(ref comp_key) = self.companion_api_key {
                cmd.env("WHATSMEOW_COMPANION_KEY", comp_key);
            }

            let start_instant = std::time::Instant::now();
            let output_res: Result<std::io::Result<std::process::Output>, tokio::time::error::Elapsed> =
                if self.timeout_duration.is_zero() {
                    Ok(cmd.output().await)
                } else {
                    tokio::time::timeout(self.timeout_duration, cmd.output()).await
                };

            let output = match output_res {
                Ok(Ok(out)) => out,
                Ok(Err(e)) => {
                    error!("Failed to spawn Antigravity CLI binary: {}", e);
                    anyhow::bail!("Failed to execute Antigravity CLI binary: {}", e);
                }
                Err(_) => {
                    let acc_desc = active_acc.as_ref()
                        .map(|a| format!("#{} ({})", a.id, a.masked_email().unwrap_or_else(|| a.label.clone())))
                        .unwrap_or_else(|| "unknown".to_string());
                    error!(
                        "Antigravity CLI timed out after {}s on {} (attempt {}/{})",
                        self.timeout_duration.as_secs(),
                        acc_desc,
                        attempt + 1,
                        max_attempts
                    );
                    if let Some(ref acc) = active_acc {
                        self.pool_manager.mark_cooldown(acc.id, Duration::from_secs(300), Some("CLI Execution Timeout (300s)")).await;
                    }
                    if attempt + 1 < max_attempts {
                        continue;
                    }
                    let acc_tag = active_acc.as_ref()
                        .map(|a| format!("[Akun #{} ({})] ", a.id, a.masked_email().unwrap_or_else(|| a.label.clone())))
                        .unwrap_or_default();
                    anyhow::bail!(
                        "{}Agent engine timed out after {} seconds.",
                        acc_tag,
                        self.timeout_duration.as_secs()
                    );
                }
            };

            let duration = start_instant.elapsed().as_secs_f64();
            let stdout_str = String::from_utf8_lossy(&output.stdout);
            let stderr_str = String::from_utf8_lossy(&output.stderr);

            if !output.status.success() {
                let err_msg = if !stderr_str.trim().is_empty() {
                    stderr_str.trim().to_string()
                } else if !stdout_str.trim().is_empty() {
                    stdout_str.trim().to_string()
                } else {
                    format!("Process exited with status code {:?}", output.status.code())
                };

                let is_transient = is_transient_error(&err_msg);
                let is_quota = is_quota_error(&err_msg);
                let is_auth = is_auth_error(&err_msg);

                if let Some(ref acc) = active_acc {
                    if is_quota {
                        let cooldown = extract_quota_cooldown_duration(&err_msg);
                        self.pool_manager.mark_cooldown(acc.id, cooldown, Some("Quota / Rate Limit Exceeded")).await;
                    } else if is_auth {
                        self.pool_manager.mark_cooldown(acc.id, Duration::from_secs(72 * 3600), Some("Eligibility Check / Auth Required")).await;
                    } else if is_transient {
                        self.pool_manager.mark_cooldown(acc.id, Duration::from_secs(60), Some("Transient Network / Stream Interruption")).await;
                    }
                }

                if (is_transient || is_quota || is_auth) && attempt + 1 < max_attempts {
                    let acc_desc = active_acc.as_ref()
                        .map(|a| format!("#{} ({})", a.id, a.masked_email().unwrap_or_else(|| a.label.clone())))
                        .unwrap_or_else(|| "unknown".to_string());
                    warn!(
                        "Attempt {}/{} failed on {} with recoverable error (quota={}, auth={}, transient={}). Retrying with next account...",
                        attempt + 1, max_attempts, acc_desc, is_quota, is_auth, is_transient
                    );
                    tokio::time::sleep(Duration::from_millis(500)).await;
                    continue;
                }

                let acc_tag = active_acc.as_ref()
                    .map(|a| format!("[Akun #{} ({})] ", a.id, a.masked_email().unwrap_or_else(|| a.label.clone())))
                    .unwrap_or_default();
                anyhow::bail!("{}Antigravity CLI failed: {}", acc_tag, err_msg);
            }

            // Find JSON line from output
            let mut parsed: Option<AgyJsonOutput> = None;
            for line in stdout_str.lines().rev() {
                let trimmed = line.trim();
                if trimmed.starts_with('{') && trimmed.ends_with('}') {
                    if let Ok(val) = serde_json::from_str::<AgyJsonOutput>(trimmed) {
                        parsed = Some(val);
                        break;
                    }
                }
            }

            let result = match parsed {
                Some(p) => {
                    if p.status.to_lowercase() == "error" {
                        let err_text = p.error.unwrap_or_else(|| "Unknown error from CLI".to_string());
                        let is_err_quota = is_quota_error(&err_text);
                        let is_err_auth = is_auth_error(&err_text);
                        let is_err_transient = is_transient_error(&err_text);

                        if let Some(ref acc) = active_acc {
                            if is_err_quota {
                                let cooldown = extract_quota_cooldown_duration(&err_text);
                                self.pool_manager.mark_cooldown(acc.id, cooldown, Some("Quota / Rate Limit Exceeded")).await;
                            } else if is_err_auth {
                                self.pool_manager.mark_cooldown(acc.id, Duration::from_secs(72 * 3600), Some("Eligibility Check / Auth Required")).await;
                            } else if is_err_transient {
                                self.pool_manager.mark_cooldown(acc.id, Duration::from_secs(60), Some("Transient Service Error")).await;
                            }
                        }
                        if attempt + 1 < max_attempts && (is_err_quota || is_err_auth || is_err_transient) {
                            let acc_desc = active_acc.as_ref()
                                .map(|a| format!("#{} ({})", a.id, a.masked_email().unwrap_or_else(|| a.label.clone())))
                                .unwrap_or_else(|| "unknown".to_string());
                            warn!(
                                "Attempt {}/{} returned error payload on {}. Retrying with next account...",
                                attempt + 1, max_attempts, acc_desc
                            );
                            tokio::time::sleep(Duration::from_millis(500)).await;
                            continue;
                        }
                        let acc_tag = active_acc.as_ref()
                            .map(|a| format!("[Akun #{} ({})] ", a.id, a.masked_email().unwrap_or_else(|| a.label.clone())))
                            .unwrap_or_default();
                        anyhow::bail!("{}Antigravity CLI execution error: {}", acc_tag, err_text);
                    }
                    AgentResponse {
                        conversation_id: p.conversation_id,
                        response_text: sanitize_agent_response(&p.response),
                        duration_seconds: if p.duration_seconds > 0.0 {
                            p.duration_seconds
                        } else {
                            duration
                        },
                    }
                }
                None => {
                    let sanitized = sanitize_agent_response(stdout_str.trim());
                    if sanitized.is_empty() {
                        anyhow::bail!("Antigravity CLI completed without returning a response text.");
                    }
                    AgentResponse {
                        conversation_id: conversation_id.unwrap_or("default").to_string(),
                        response_text: sanitized,
                        duration_seconds: duration,
                    }
                }
            };

            return Ok(result);
        }

        anyhow::bail!("All account attempts exhausted.");
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
        self.pool_manager.is_authenticated().await
    }

    async fn save_auth_token(&self, token_content: &str) -> anyhow::Result<()> {
        self.pool_manager.save_auth_token(token_content).await
    }

    async fn remove_account(&self, account_id: usize) -> anyhow::Result<bool> {
        self.pool_manager.remove_account(account_id).await
    }

    async fn clear_account_pool(&self) -> anyhow::Result<usize> {
        self.pool_manager.clear_account_pool().await
    }

    async fn get_account_pool_status(&self) -> Vec<crate::core::ports::AccountPoolStatus> {
        self.pool_manager.get_account_pool_status().await
    }

    async fn init_oauth_session(&self) -> anyhow::Result<(String, String)> {
        oauth::init_oauth_session().await
    }

    async fn exchange_oauth_code(&self, session_id: &str, code: &str) -> anyhow::Result<String> {
        let token_content = oauth::exchange_oauth_code(session_id, code).await?;
        self.save_auth_token(&token_content).await?;
        let email = extract_email_from_token(&token_content)
            .unwrap_or_else(|| "Akun Baru".to_string());
        Ok(email)
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::adapters::driven::agy_cli::account_pool::AccountToken;


    #[tokio::test]
    async fn test_pool_remove_and_clear() {
        let pool = vec![
            AccountToken::new(1, "Account-Default".to_string(), Some("default@gmail.com".to_string()), "tok1".to_string()),
            AccountToken::new(2, "Account-2".to_string(), Some("second@gmail.com".to_string()), "tok2".to_string()),
            AccountToken::new(3, "Account-3".to_string(), Some("third@gmail.com".to_string()), "tok3".to_string()),
        ];

        let pool_manager = Arc::new(TokenPoolManager::with_pool(pool));
        let adapter = AntigravityCliAdapter::with_pool_manager(
            "/bin/true",
            "gemini-2.5-flash",
            "/tmp",
            60,
            "http://localhost:8080",
            "secret",
            pool_manager,
        );

        // Test remove Account #2
        let removed = adapter.remove_account(2).await.unwrap();
        assert!(removed);

        let status = adapter.get_account_pool_status().await;
        assert_eq!(status.len(), 2);
        assert_eq!(status[0].id, 1);
        assert_eq!(status[0].label, "Account-Default");
        assert_eq!(status[1].id, 2);
        assert_eq!(status[1].label, "Account-2");

        // Test clear (retains primary account)
        let cleared = adapter.clear_account_pool().await.unwrap();
        assert_eq!(cleared, 1);

        let status = adapter.get_account_pool_status().await;
        assert_eq!(status.len(), 1);
        assert_eq!(status[0].id, 1);
        assert_eq!(status[0].label, "Account-Default");
    }

    #[tokio::test]
    async fn test_unlimited_zero_timeout_does_not_timeout_immediately() {
        let pool = vec![
            AccountToken::new(1, "Account-Default".to_string(), Some("default@gmail.com".to_string()), "tok1".to_string()),
        ];
        let pool_manager = Arc::new(TokenPoolManager::with_pool(pool));
        // Using /bin/echo to simulate a successful CLI call returning valid JSON
        let adapter = AntigravityCliAdapter::with_pool_manager(
            "/bin/echo",
            "gemini-2.5-flash",
            "/tmp",
            0, // 0 = unlimited
            "http://localhost:8080",
            "secret",
            pool_manager,
        );

        // When timeout is 0, execution must not fail with timeout
        let res = adapter.execute(Some("c1"), "{\"conversation_id\":\"c1\",\"status\":\"DONE\",\"response\":\"hello\"}").await;
        // Even if echo doesn't parse full prompt flags as real agy, it shouldn't fail with "timed out after 0 seconds"
        if let Err(e) = res {
            assert!(!e.to_string().contains("timed out after 0 seconds"), "Must not time out immediately on zero timeout: {}", e);
        }
    }

    #[tokio::test]
    async fn test_error_attribution_contains_account_info() {
        let pool = vec![
            AccountToken::new(1, "Account-Primary".to_string(), Some("testuser@gmail.com".to_string()), "tok1".to_string()),
        ];
        let pool_manager = Arc::new(TokenPoolManager::with_pool(pool));
        // Using /bin/false to simulate a failed CLI process
        let adapter = AntigravityCliAdapter::with_pool_manager(
            "/bin/false",
            "gemini-2.5-flash",
            "/tmp",
            10,
            "http://localhost:8080",
            "secret",
            pool_manager,
        );

        let res = adapter.execute(None, "hello").await;
        assert!(res.is_err());
        let err_msg = res.unwrap_err().to_string();
        // The error must explicitly contain the account tag [Akun #1 (tes***@gmail.com)]
        assert!(err_msg.contains("[Akun #1 (tes***@gmail.com)]"), "Expected account attribution tag in error, got: {}", err_msg);
    }
}

