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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessageRecord {
    pub id: i64,
    pub chat_jid: String,
    pub sender_jid: String,
    pub text: String,
    pub is_from_me: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupMemberRecord {
    pub group_jid: String,
    pub user_jid: String,
    pub user_name: Option<String>,
    pub role_in_group: String,
    pub last_synced_at: String,
}

#[async_trait]
pub trait SessionStorePort: Send + Sync {
    /// Retrieves the active Antigravity conversation UUID mapped to the given WhatsApp chat JID (unconditionally).
    async fn get_conversation_id(&self, chat_jid: &str) -> anyhow::Result<Option<String>>;

    /// Retrieves the active Antigravity conversation UUID mapped to the given WhatsApp chat JID,
    /// returning None if the conversation has expired due to inactivity exceeding max_inactivity_secs.
    async fn get_active_conversation_id(
        &self,
        chat_jid: &str,
        max_inactivity_secs: u64,
    ) -> anyhow::Result<Option<String>>;

    /// Refreshes the last activity timestamp for the active conversation of a chat.
    async fn touch_conversation_activity(&self, chat_jid: &str) -> anyhow::Result<()>;

    /// Searches past message history for a chat matching query keywords.
    /// If sender_jid is provided (e.g. in DM), expands search scope across conversations the sender has access to.
    async fn search_message_history(
        &self,
        chat_jid: &str,
        sender_jid: Option<&str>,
        query: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<ChatMessageRecord>>;

    /// Retrieves recent messages for a chat.
    /// If sender_jid is provided (e.g. in DM), expands retrieval scope across conversations the sender has access to.
    async fn get_recent_messages(
        &self,
        chat_jid: &str,
        sender_jid: Option<&str>,
        limit: usize,
    ) -> anyhow::Result<Vec<ChatMessageRecord>>;

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

    /// Records an incoming message action audit entry.
    async fn record_action_audit(&self, audit: &crate::core::domain::NewWhatsAppActionAudit) -> anyhow::Result<i64>;

    /// Updates an existing action audit entry with final results.
    async fn update_action_audit_result(
        &self,
        id: i64,
        conversation_id: Option<&str>,
        response_text: Option<&str>,
        error_message: Option<&str>,
        status: &str,
        duration_seconds: Option<f64>,
        tools_invoked: &[String],
        usecase: Option<&str>,
    ) -> anyhow::Result<()>;

    /// Queries action audit records according to filter parameters.
    async fn query_action_audits(
        &self,
        filter: &crate::core::domain::ActionAuditFilter,
    ) -> anyhow::Result<Vec<crate::core::domain::WhatsAppActionAudit>>;

    /// Retrieves an action audit record by its primary key ID.
    async fn get_action_audit_by_id(&self, id: i64) -> anyhow::Result<Option<crate::core::domain::WhatsAppActionAudit>>;

    /// Retrieves an action audit record by WhatsApp message ID.
    async fn get_action_audit_by_message_id(&self, message_id: &str) -> anyhow::Result<Option<crate::core::domain::WhatsAppActionAudit>>;

    /// Aggregates an audit summary report across all recorded actions.
    async fn get_action_audit_summary(&self) -> anyhow::Result<crate::core::domain::AuditSummaryReport>;

    /// Records an ex-ante metacognitive prediction before task execution.
    async fn record_metacognitive_prediction(&self, pred: &crate::core::domain::NewMetacognitivePrediction) -> anyhow::Result<i64>;

    /// Resolves an ex-ante metacognitive prediction with actual ground truth outcome and calculates Brier score.
    async fn resolve_metacognitive_prediction(
        &self,
        prediction_id: &str,
        actual_outcome: f64,
        duration_secs: f64,
        error_detail: Option<&str>,
    ) -> anyhow::Result<()>;

    /// Lists recent metacognitive predictions, optionally filtered by domain type.
    async fn list_metacognitive_predictions(
        &self,
        limit: usize,
        domain_filter: Option<&str>,
    ) -> anyhow::Result<Vec<crate::core::domain::MetacognitivePrediction>>;

    /// Calculates aggregate metacognitive calibration statistics (Brier score, BSS, reliability buckets).
    async fn get_metacognitive_calibration_stats(&self) -> anyhow::Result<crate::core::domain::MetacognitiveCalibrationStats>;

    /// Records or updates a user's membership in a WhatsApp group.
    async fn record_group_membership(
        &self,
        group_jid: &str,
        user_jid: &str,
        user_name: Option<&str>,
        role_in_group: Option<&str>,
    ) -> anyhow::Result<()>;

    /// Retrieves all group JIDs where the given user is known to be a member.
    async fn get_user_groups(&self, user_jid: &str) -> anyhow::Result<Vec<String>>;

    /// Retrieves all members of a specific WhatsApp group.
    async fn get_group_members(&self, group_jid: &str) -> anyhow::Result<Vec<GroupMemberRecord>>;

    /// Checks if a user is a member of a specific WhatsApp group.
    async fn is_user_in_group(&self, user_jid: &str, group_jid: &str) -> anyhow::Result<bool>;
}
