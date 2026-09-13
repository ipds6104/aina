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

pub struct AntigravityCliAdapter {
    binary_path: PathBuf,
    model: Arc<RwLock<String>>,
    workspace_dir: PathBuf,
    timeout_duration: Duration,
    whatsmeow_base_url: String,
    whatsmeow_api_key: String,
}

impl AntigravityCliAdapter {
    pub fn new(
        binary_path: impl Into<PathBuf>,
        model: impl Into<String>,
        workspace_dir: impl Into<PathBuf>,
        timeout_seconds: u64,
        whatsmeow_base_url: impl Into<String>,
        whatsmeow_api_key: impl Into<String>,
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
        }
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
        let mut cmd = Command::new(&bin_path);
        
        // Ensure workspace directory exists
        if !self.workspace_dir.exists() {
            tokio::fs::create_dir_all(&self.workspace_dir).await?;
        }
        cmd.current_dir(&self.workspace_dir);

        // Add conversation flag if continuing a session
        if let Some(conv_id) = conversation_id {
            if !conv_id.trim().is_empty() {
                cmd.arg("--conversation").arg(conv_id);
            }
        }

        // Add headless execution flags
        let print_timeout_sec = self.timeout_duration.as_secs().saturating_sub(5).max(30);
        cmd.arg("-p").arg(prompt);
        cmd.arg("--output-format").arg("json");
        cmd.arg("--print-timeout").arg(format!("{}s", print_timeout_sec));
        cmd.arg("--dangerously-skip-permissions");
        cmd.arg("--model").arg(&active_model);

        // Inject live WhatsApp gateway connection variables so CLI tools (wa_tool.py) work seamlessly
        cmd.env("WHATSMEOW_BASE_URL", &self.whatsmeow_base_url);
        cmd.env("WHATSMEOW_URL", &self.whatsmeow_base_url);
        cmd.env("WHATSMEOW_API_KEY", &self.whatsmeow_api_key);
        cmd.env("API_KEY", &self.whatsmeow_api_key);

        debug!(
            "Executing Antigravity CLI: {:?} (conv: {:?}, model: {})",
            bin_path, conversation_id, active_model
        );

        // Run with timeout
        let child_future = cmd.output();
        let output = match tokio::time::timeout(self.timeout_duration, child_future).await {
            Ok(res) => res?,
            Err(_) => {
                anyhow::bail!(
                    "Antigravity CLI execution timed out after {:?}",
                    self.timeout_duration
                );
            }
        };

        let stdout_raw = String::from_utf8_lossy(&output.stdout);
        let stderr_raw = String::from_utf8_lossy(&output.stderr);

        if !output.status.success() {
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
                // Fallback: extract substring between first '{' and last '}'
                let first = stdout_raw.find('{')?;
                let last = stdout_raw.rfind('}')?;
                if first < last {
                    Some(&stdout_raw[first..=last])
                } else {
                    None
                }
            });

        match json_line {
            Some(payload) => {
                let parsed: AgyJsonOutput = serde_json::from_str(payload)?;
                info!(
                    "Agent turn finished: conv={}, status={}, model={}",
                    parsed.conversation_id, parsed.status, active_model
                );
                let response_text = if !parsed.response.trim().is_empty() {
                    parsed.response.trim().to_string()
                } else if let Some(ref err) = parsed.error {
                    format!("⚠️ Mohon maaf, terjadi kendala saat memproses permintaan: {}", err)
                } else {
                    "(Tidak ada respons dari agen AI)".to_string()
                };

                Ok(AgentResponse {
                    conversation_id: parsed.conversation_id,
                    response_text,
                    duration_seconds: parsed.duration_seconds,
                })
            }
            None => {
                warn!("Could not find JSON in stdout, returning raw text: {}", stdout_raw);
                Ok(AgentResponse {
                    conversation_id: conversation_id.unwrap_or_default().to_string(),
                    response_text: stdout_raw.trim().to_string(),
                    duration_seconds: 0.0,
                })
            }
        }
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
