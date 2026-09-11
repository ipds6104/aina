use async_trait::async_trait;

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
    ) -> anyhow::Result<AgentResponse>;

    /// Checks if the agent has a valid authentication token stored.
    async fn is_authenticated(&self) -> bool;

    /// Saves and validates an OAuth token for the agent.
    async fn save_auth_token(&self, token_content: &str) -> anyhow::Result<()>;
}
