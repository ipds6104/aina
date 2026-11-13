use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountPoolStatus {
    pub id: usize,
    pub label: String,
    pub email: Option<String>,
    pub is_cooldown: bool,
    pub cooldown_remaining_secs: u64,
    #[serde(default)]
    pub cooldown_reason: Option<String>,
}

impl AccountPoolStatus {
    pub fn masked_email(&self) -> Option<String> {
        self.email.as_ref().map(|email| {
            if let Some((user, domain)) = email.split_once('@') {
                if user.len() <= 3 {
                    format!("{}***@{}", user, domain)
                } else {
                    format!("{}***@{}", &user[..3], domain)
                }
            } else {
                email.to_string()
            }
        })
    }

    pub fn formatted_cooldown(&self) -> String {
        let secs = self.cooldown_remaining_secs;
        if secs >= 3600 {
            let h = secs / 3600;
            let m = (secs % 3600) / 60;
            if m > 0 {
                format!("{}h {}m", h, m)
            } else {
                format!("{}h", h)
            }
        } else if secs >= 60 {
            let m = secs / 60;
            let s = secs % 60;
            if s > 0 {
                format!("{}m {}s", m, s)
            } else {
                format!("{}m", m)
            }
        } else {
            format!("{}s", secs)
        }
    }
}

#[derive(Debug, Clone)]
pub struct AgentResponse {
    pub conversation_id: String,
    pub response_text: String,
    pub duration_seconds: f64,
}

/// A single model entry exposed by the agent engine (live or fallback catalog).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    /// Inferred family: "gemini" | "claude" | "gpt" | "other".
    pub category: String,
}

/// Result of a model-discovery call, including where the list came from.
///
/// `source` is one of:
/// - `"dynamic"` — parsed live from the agent CLI (`agy models`)
/// - `"static_fallback"` — built-in catalog used because the CLI was unreachable
/// - `"unavailable"` — the engine does not support discovery
#[derive(Debug, Clone)]
pub struct ModelCatalog {
    pub models: Vec<ModelInfo>,
    pub source: &'static str,
}

impl ModelCatalog {
    /// Formats the catalog as a WhatsApp-friendly bullet list, marking the
    /// currently-active model. Used by user-facing `/model status` commands so
    /// the reply always reflects live discovery instead of a hardcoded list.
    pub fn whatsapp_model_list(&self, current: &str) -> String {
        if self.models.is_empty() {
            return "• _Daftar model tidak dapat dimuat dari engine._".to_string();
        }
        self.models
            .iter()
            .map(|m| {
                let marker = if m.id == current { " *(aktif)*" } else { "" };
                format!("• `{}` — {}{}", m.id, m.name, marker)
            })
            .collect()
    }
}

#[async_trait]
pub trait AgentEnginePort: Send + Sync {
    /// Executes a prompt through the Antigravity agent CLI.
    /// If conversation_id is provided, continues that existing session thread.
    async fn execute(
        &self,
        conversation_id: Option<&str>,
        prompt: &str,
    ) -> anyhow::Result<AgentResponse> {
        self.execute_with_model(conversation_id, prompt, None).await
    }

    /// Executes a prompt through the Antigravity agent CLI with an optional model override.
    async fn execute_with_model(
        &self,
        conversation_id: Option<&str>,
        prompt: &str,
        model_override: Option<&str>,
    ) -> anyhow::Result<AgentResponse>;

    /// Executes a prompt while exposing the verified turn capability to the
    /// agent process (e.g. via `AINA_TURN_TOKEN`). Engines that do not support
    /// capability injection simply ignore it.
    async fn execute_with_capability(
        &self,
        conversation_id: Option<&str>,
        prompt: &str,
        _capability: Option<&crate::core::domain::capability::TurnCapability>,
    ) -> anyhow::Result<AgentResponse> {
        self.execute_with_model(conversation_id, prompt, None).await
    }

    /// Gets the current active default model name.
    async fn get_model(&self) -> String;

    /// Sets the current active default model name.
    async fn set_model(&self, model: &str) -> anyhow::Result<()>;

    /// Returns the live list of models available to the engine.
    ///
    /// Engines backed by the Antigravity CLI discover models by shelling out
    /// to `agy models` (source `"dynamic"`); any engine that does not support
    /// discovery returns an empty catalog marked `"unavailable"`.
    async fn list_models(&self) -> ModelCatalog {
        ModelCatalog { models: vec![], source: "unavailable" }
    }

    /// Checks if the agent has a valid authentication token stored.
    async fn is_authenticated(&self) -> bool;

    /// Saves and validates an OAuth token for the agent.
    async fn save_auth_token(&self, token_content: &str) -> anyhow::Result<()>;

    /// Removes an account from the pool by its ID.
    async fn remove_account(&self, account_id: usize) -> anyhow::Result<bool> {
        let _ = account_id;
        Ok(false)
    }

    /// Clears all accounts from the pool.
    async fn clear_account_pool(&self) -> anyhow::Result<usize> {
        Ok(0)
    }

    /// Returns the live status of the Antigravity account pool.
    async fn get_account_pool_status(&self) -> Vec<AccountPoolStatus> {
        vec![]
    }

    /// Initiates an interactive Google OAuth session for adding an account.
    /// Returns (session_id, auth_url).
    async fn init_oauth_session(&self) -> anyhow::Result<(String, String)> {
        anyhow::bail!("OAuth interactive session not supported by this engine")
    }

    /// Completes the OAuth session by supplying the authorization code pasted by the user.
    /// Returns the detected email or account label.
    async fn exchange_oauth_code(&self, session_id: &str, code: &str) -> anyhow::Result<String> {
        let _ = (session_id, code);
        anyhow::bail!("OAuth interactive session not supported by this engine")
    }
}
