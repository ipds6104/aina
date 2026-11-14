use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserProfile {
    pub sender_jid: String,
    pub name: Option<String>,
    pub role: Option<String>,
    pub authority_level: String, // "admin", "atasan", "staff", "guest"
    pub notes: Option<String>,
    #[serde(default)]
    pub granted_by: Option<String>,
    #[serde(default)]
    pub granted_at: Option<i64>,
    #[serde(default)]
    pub role_hierarchy: Option<String>,
    #[serde(default)]
    pub communication_prefs: Option<String>,
    #[serde(default)]
    pub active_projects: Option<String>,
    #[serde(default)]
    pub durable_constraints: Option<String>,
    #[serde(default)]
    pub confidence_score: Option<f64>,
}

impl Default for UserProfile {
    fn default() -> Self {
        Self {
            sender_jid: String::new(),
            name: None,
            role: None,
            authority_level: "guest".to_string(),
            notes: None,
            granted_by: None,
            granted_at: None,
            role_hierarchy: None,
            communication_prefs: None,
            active_projects: None,
            durable_constraints: None,
            confidence_score: None,
        }
    }
}

impl UserProfile {
    pub fn new(
        sender_jid: String,
        name: Option<String>,
        role: Option<String>,
        authority_level: String,
        notes: Option<String>,
    ) -> Self {
        Self {
            sender_jid,
            name,
            role,
            authority_level,
            notes,
            granted_by: None,
            granted_at: None,
            role_hierarchy: None,
            communication_prefs: None,
            active_projects: None,
            durable_constraints: None,
            confidence_score: None,
        }
    }
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
    /// returning None if the conversation has expired due to inactivity exceeding max_inactivity_secs
    /// or turn count exceeding max_turns.
    async fn get_active_conversation_id(
        &self,
        chat_jid: &str,
        max_inactivity_secs: u64,
        max_turns: u32,
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

    /// Deletes profiling memory for a sender JID (Self-service Right to be Forgotten / UU PDP).
    async fn delete_user_profile(&self, sender_jid: &str) -> anyhow::Result<bool>;

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

    /// Gets a single scheduled task by unique slug.
    async fn get_scheduled_task_by_slug(&self, slug: &str) -> anyhow::Result<Option<crate::core::domain::ScheduledTask>>;

    /// Toggles active status of a scheduled task.
    async fn toggle_scheduled_task(&self, id: i64) -> anyhow::Result<bool>;

    /// Sets active status of a scheduled task explicitly.
    async fn set_scheduled_task_active(&self, id: i64, is_active: bool) -> anyhow::Result<()>;

    /// Updates existing scheduled task fields.
    async fn update_scheduled_task(
        &self,
        id: i64,
        title: Option<&str>,
        target_jid: Option<&str>,
        payload: Option<&str>,
        schedule_type: Option<&str>,
        schedule_expr: Option<&str>,
        next_run_epoch: Option<i64>,
        category: Option<&str>,
    ) -> anyhow::Result<bool>;

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

    /// Checks if a WhatsApp group is explicitly designated as trusted by Companion.
    async fn is_group_trusted(&self, group_jid: &str) -> anyhow::Result<bool>;

    /// Designates a WhatsApp group as trusted or untrusted by Companion.
    async fn set_group_trusted(&self, group_jid: &str, is_trusted: bool, authorized_by: Option<&str>) -> anyhow::Result<()>;

    /// Authorizes a user profile with an explicit authority level granted by Companion.
    async fn authorize_user(&self, user_jid: &str, role: &str, authority: &str, notes: Option<&str>, granted_by: &str) -> anyhow::Result<()>;

    /// Persists a short-lived turn capability minted for a verified sender.
    async fn create_turn_capability(
        &self,
        capability: &crate::core::domain::capability::TurnCapability,
    ) -> anyhow::Result<()>;

    /// Looks up a turn capability by its opaque token.
    async fn get_turn_capability(
        &self,
        token: &str,
    ) -> anyhow::Result<Option<crate::core::domain::capability::TurnCapability>>;

    /// Deletes expired capabilities and returns how many rows were removed.
    async fn purge_expired_turn_capabilities(&self, now: i64) -> anyhow::Result<usize>;

    /// Records a new experience reflection (Harness² format: cue, lesson, evidence).
    async fn record_experience(&self, exp: &crate::core::domain::NewExperienceRecord) -> anyhow::Result<String>;

    /// Retrieves an experience record by ID.
    async fn get_experience(&self, id: &str) -> anyhow::Result<Option<crate::core::domain::ExperienceRecord>>;

    /// Searches past experience records matching query keywords.
    async fn search_experiences(
        &self,
        query: &str,
        domain: Option<&str>,
        limit: usize,
    ) -> anyhow::Result<Vec<crate::core::domain::ExperienceRecord>>;

    /// Lists recent experience records, optionally filtered by domain.
    async fn list_experiences(
        &self,
        domain: Option<&str>,
        limit: usize,
    ) -> anyhow::Result<Vec<crate::core::domain::ExperienceRecord>>;

    /// Records a tamper-evident, hash-chained tool execution (NIST AU-10 & ISO 42001).
    async fn record_tool_execution(
        &self,
        exec: &crate::core::domain::NewAgentToolExecution,
    ) -> anyhow::Result<crate::core::domain::AgentToolExecution>;

    /// Queries tool executions attributed to a specific WhatsApp sender.
    async fn get_tool_executions_by_sender(
        &self,
        sender_jid: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<crate::core::domain::AgentToolExecution>>;

    /// Queries tool executions by tool name.
    async fn get_tool_executions_by_tool(
        &self,
        tool_name: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<crate::core::domain::AgentToolExecution>>;

    /// Deterministically verifies the cryptographic SHA-256 hash chain of the tool execution audit trail.
    async fn verify_audit_hash_chain(&self) -> anyhow::Result<bool>;
}
