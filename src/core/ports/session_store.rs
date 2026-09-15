use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserProfile {
    pub sender_jid: String,
    pub name: Option<String>,
    pub role: Option<String>,
    pub authority_level: String, // "admin", "staff", "guest", "external"
    pub notes: Option<String>,
}

#[async_trait]
pub trait SessionStorePort: Send + Sync {
    /// Retrieves the active Antigravity conversation UUID mapped to the given WhatsApp chat JID.
    async fn get_conversation_id(&self, chat_jid: &str) -> anyhow::Result<Option<String>>;

    /// Saves the mapping of a WhatsApp chat JID to an Antigravity conversation UUID.
    async fn save_conversation_id(&self, chat_jid: &str, conv_uuid: &str) -> anyhow::Result<()>;

    /// Deletes the mapping of a WhatsApp chat JID, resetting the active conversation.
    async fn delete_conversation_id(&self, chat_jid: &str) -> anyhow::Result<()>;

    /// Records an incoming or outgoing message for local logging or future recall.
    async fn record_message(
        &self,
        chat_jid: &str,
        sender_jid: &str,
        text: &str,
        is_from_me: bool,
    ) -> anyhow::Result<()>;

    /// Retrieves profiling memory for a sender JID.
    async fn get_user_profile(&self, sender_jid: &str) -> anyhow::Result<Option<UserProfile>>;

    /// Saves or updates profiling memory for a sender JID.
    async fn save_user_profile(&self, profile: &UserProfile) -> anyhow::Result<()>;

    /// Creates a new scheduled task in the database and returns its assigned ID.
    async fn create_scheduled_task(&self, task: &crate::core::domain::NewScheduledTask) -> anyhow::Result<i64>;

    /// Lists scheduled tasks, optionally filtering to active ones only.
    async fn list_scheduled_tasks(&self, active_only: bool) -> anyhow::Result<Vec<crate::core::domain::ScheduledTask>>;

    /// Retrieves scheduled tasks whose next_run_epoch is <= current_epoch and is_active is true.
    async fn get_due_scheduled_tasks(&self, current_epoch: i64) -> anyhow::Result<Vec<crate::core::domain::ScheduledTask>>;

    /// Updates task execution record (last_run, next_run, is_active).
    async fn update_scheduled_task_run(
        &self,
        id: i64,
        last_run: i64,
        next_run: Option<i64>,
        is_active: bool,
    ) -> anyhow::Result<()>;

    /// Deletes a scheduled task by ID.
    async fn delete_scheduled_task(&self, id: i64) -> anyhow::Result<bool>;

    /// Gets a single scheduled task by ID.
    async fn get_scheduled_task(&self, id: i64) -> anyhow::Result<Option<crate::core::domain::ScheduledTask>>;

    /// Records an execution run of a scheduled task into the audit log.
    async fn record_scheduled_task_run(
        &self,
        task_id: i64,
        task_title: &str,
        target_jid: &str,
        status: &str,
        duration_secs: f64,
        error_message: Option<&str>,
        output_preview: Option<&str>,
    ) -> anyhow::Result<i64>;

    /// Lists recent task run logs (up to `limit`).
    async fn list_scheduled_task_runs(&self, limit: usize) -> anyhow::Result<Vec<crate::core::domain::ScheduledTaskRun>>;

    /// Updates task execution record with final status, error message, and duration.
    async fn update_scheduled_task_result(
        &self,
        id: i64,
        status: &str,
        error_message: Option<&str>,
        duration_secs: f64,
    ) -> anyhow::Result<()>;

    /// Retrieves diagnostic summary of the scheduler subsystem.
    async fn get_scheduler_diagnostics(&self) -> anyhow::Result<crate::core::domain::SchedulerDiagnostics>;
}
