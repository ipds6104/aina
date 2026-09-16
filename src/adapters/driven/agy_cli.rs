use crate::core::ports::{AgentEnginePort, AgentResponse};
use async_trait::async_trait;
use serde::Deserialize;
use std::path::PathBuf;
use std::time::Duration;
use tokio::process::Command;
use tracing::{debug, error, info, warn};

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

#[derive(Debug, Clone)]
pub struct AccountToken {
    #[allow(dead_code)]
    pub id: usize,
    pub label: String,
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
        }
    }

    fn load_initial_token_pool() -> Vec<AccountToken> {
        let mut pool = Vec::new();

        // 1. Check AINA_OAUTH_TOKENS (JSON array of token strings or objects)
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
                        pool.push(AccountToken {
                            id: idx + 1,
                            label: format!("Account-{}", idx + 1),
                            token_json: token_str,
                            cooldown_until: Arc::new(RwLock::new(None)),
                        });
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
                        pool.push(AccountToken {
                            id: i,
                            label: format!("Account-{}", i),
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
                    pool.push(AccountToken {
                        id: 1,
                        label: "Account-Primary".to_string(),
                        token_json: trimmed,
                        cooldown_until: Arc::new(RwLock::new(None)),
                    });
                }
            }
        }

        // 4. Fallback to existing token file on disk if available
        if pool.is_empty() {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
            let default_path = PathBuf::from(home).join(".gemini/antigravity-cli/antigravity-oauth-token");
            if default_path.exists() {
                if let Ok(content) = std::fs::read_to_string(&default_path) {
                    let trimmed = content.trim().to_string();
                    if !trimmed.is_empty() {
                        pool.push(AccountToken {
                            id: 1,
                            label: "Account-Default".to_string(),
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

        for attempt in 0..max_attempts {
            let active_acc = if !pool.is_empty() {
                let now = std::time::Instant::now();
                let mut candidate = None;
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
                let chosen = candidate.unwrap_or_else(|| pool[attempt % pool.len()].clone());

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

            // Run process (unlimited or with timeout)
            let child_future = cmd.output();
            let output = if is_unlimited {
                child_future.await?
            } else {
                match tokio::time::timeout(self.timeout_duration, child_future).await {
                    Ok(res) => res?,
                    Err(_) => {
                        anyhow::bail!(
                            "Antigravity CLI execution timed out after {:?}",
                            self.timeout_duration
                        );
                    }
                }
            };

            let stdout_raw = String::from_utf8_lossy(&output.stdout);
            let stderr_raw = String::from_utf8_lossy(&output.stderr);
            let combined = format!("{}\n{}", stderr_raw, stdout_raw);

            let is_quota = combined.contains("503")
                || combined.contains("429")
                || combined.contains("quota")
                || combined.contains("Resource has been exhausted")
                || combined.contains("rate limit");

            if !output.status.success() {
                if is_quota && pool.len() > 1 && attempt + 1 < max_attempts {
                    if let Some(ref acc) = active_acc {
                        *acc.cooldown_until.write().await = Some(std::time::Instant::now() + std::time::Duration::from_secs(300));
                        warn!(
                            "Account {} hit quota limit. Cooling down for 5m. Failing over (attempt {}/{})...",
                            acc.label, attempt + 1, max_attempts
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

        // Synchronize in-memory token pool
        {
            let mut pool = self.token_pool.write().await;
            if let Some(existing) = pool.iter_mut().find(|a| a.token_json == trimmed) {
                *existing.cooldown_until.write().await = None;
            } else {
                let next_id = pool.len() + 1;
                pool.push(AccountToken {
                    id: next_id,
                    label: format!("Account-{}", next_id),
                    token_json: trimmed.to_string(),
                    cooldown_until: Arc::new(RwLock::new(None)),
                });
            }
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
}
