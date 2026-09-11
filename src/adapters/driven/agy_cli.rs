use crate::core::ports::{AgentEnginePort, AgentResponse};
use async_trait::async_trait;
use serde::Deserialize;
use std::path::PathBuf;
use std::time::Duration;
use tokio::process::Command;
use tracing::{debug, error, info, warn};

#[derive(Debug, Deserialize)]
struct AgyJsonOutput {
    pub conversation_id: String,
    pub status: String,
    pub response: String,
    #[serde(default)]
    pub duration_seconds: f64,
}

pub struct AntigravityCliAdapter {
    binary_path: PathBuf,
    model: String,
    workspace_dir: PathBuf,
    timeout_duration: Duration,
}

impl AntigravityCliAdapter {
    pub fn new(
        binary_path: impl Into<PathBuf>,
        model: impl Into<String>,
        workspace_dir: impl Into<PathBuf>,
        timeout_seconds: u64,
    ) -> Self {
        Self {
            binary_path: binary_path.into(),
            model: model.into(),
            workspace_dir: workspace_dir.into(),
            timeout_duration: Duration::from_secs(timeout_seconds),
        }
    }

    fn get_token_path(&self) -> PathBuf {
        if let Ok(val) = std::env::var("ANTIGRAVITY_TOKEN_PATH") {
            return PathBuf::from(val);
        }
        let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
        PathBuf::from(home).join(".gemini/antigravity-cli/antigravity-oauth-token")
    }
}

#[async_trait]
impl AgentEnginePort for AntigravityCliAdapter {
    async fn execute(
        &self,
        conversation_id: Option<&str>,
        prompt: &str,
    ) -> anyhow::Result<AgentResponse> {
        let mut cmd = Command::new(&self.binary_path);
        
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
        cmd.arg("-p").arg(prompt);
        cmd.arg("--output-format").arg("json");
        cmd.arg("--dangerously-skip-permissions");
        cmd.arg("--model").arg(&self.model);

        debug!(
            "Executing Antigravity CLI: {:?} (conv: {:?})",
            self.binary_path, conversation_id
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
                    "Agent turn finished: conv={}, status={}",
                    parsed.conversation_id, parsed.status
                );
                Ok(AgentResponse {
                    conversation_id: parsed.conversation_id,
                    response_text: parsed.response.trim().to_string(),
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
