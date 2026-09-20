use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountPoolStatus {
    pub id: usize,
    pub label: String,
    pub email: Option<String>,
    pub is_cooldown: bool,
    pub cooldown_remaining_secs: u64,
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
}

#[derive(Debug, Clone)]
pub struct AgentResponse {
    pub conversation_id: String,
    pub response_text: String,
    pub duration_seconds: f64,
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

    /// Gets the current active default model name.
    async fn get_model(&self) -> String;

    /// Sets the current active default model name.
    async fn set_model(&self, model: &str) -> anyhow::Result<()>;

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
}
