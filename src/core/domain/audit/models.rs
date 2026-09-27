//! Data models and DTO structures for action audits, presence tracking, and system diagnostics.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditStep {
    pub session_id: String,
    pub step_index: Option<i64>,
    pub step_type: Option<String>,
    pub source: Option<String>,
    pub status: Option<String>,
    pub created_at: Option<String>,
    pub summary: String,
    pub tool_calls_count: usize,
}

fn default_usecase() -> String {
    "casual_and_consultation".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhatsAppActionAudit {
    pub id: i64,
    pub message_id: String,
    pub chat_jid: String,
    pub chat_type: String, // "direct" or "group"
    pub sender_jid: String,
    pub sender_name: Option<String>,
    pub decision: String, // "respond", "record_only", "ignore"
    pub decision_reason: String,
    pub conversation_id: Option<String>,
    pub status: String, // "success", "failed", "ignored", "recorded", "in_progress"
    pub input_text: String,
    pub has_media: bool,
    pub media_path: Option<String>,
    pub response_text: Option<String>,
    pub error_message: Option<String>,
    pub duration_seconds: Option<f64>,
    pub tools_invoked: Vec<String>,
    #[serde(default = "default_usecase")]
    pub usecase: String,
    pub created_at_epoch: i64,
    pub completed_at_epoch: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewWhatsAppActionAudit {
    pub message_id: String,
    pub chat_jid: String,
    pub chat_type: String,
    pub sender_jid: String,
    pub sender_name: Option<String>,
    pub decision: String,
    pub decision_reason: String,
    pub conversation_id: Option<String>,
    pub status: String,
    pub input_text: String,
    pub has_media: bool,
    pub media_path: Option<String>,
    pub response_text: Option<String>,
    pub error_message: Option<String>,
    pub duration_seconds: Option<f64>,
    pub tools_invoked: Vec<String>,
    pub usecase: String,
    pub created_at_epoch: i64,
    pub completed_at_epoch: Option<i64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ActionAuditFilter {
    pub chat_jid: Option<String>,
    pub sender_jid: Option<String>,
    pub decision: Option<String>,
    pub status: Option<String>,
    pub usecase: Option<String>,
    pub tool: Option<String>,
    pub query: Option<String>,
    pub since_epoch: Option<i64>,
    pub until_epoch: Option<i64>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallDetail {
    pub name: String,
    pub tool_action: Option<String>,
    pub tool_summary: Option<String>,
    pub arguments: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptStepDetail {
    pub step_index: Option<i64>,
    pub step_type: Option<String>,
    pub source: Option<String>,
    pub status: Option<String>,
    pub created_at: Option<String>,
    pub summary: String,
    pub tool_calls: Vec<ToolCallDetail>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceProvenance {
    pub name: String,
    pub path: String,
    pub is_git_repo: bool,
    pub git_commit: Option<String>,
    pub git_branch: Option<String>,
    pub is_dirty: bool,
    pub scripts_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetailedActionAudit {
    pub audit: WhatsAppActionAudit,
    pub transcript_path: Option<String>,
    pub transcript_steps: Vec<TranscriptStepDetail>,
    pub workspace_provenance: Vec<WorkspaceProvenance>,
    #[serde(default)]
    pub has_running_tasks: bool,
    #[serde(default)]
    pub running_tasks_count: usize,
    #[serde(default)]
    pub total_transcript_steps: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CountMetric {
    pub key: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditSummaryReport {
    pub total_actions: i64,
    pub total_responses: i64,
    pub total_recorded_only: i64,
    pub total_ignored: i64,
    pub total_errors: i64,
    pub avg_duration_seconds: f64,
    pub most_active_chats: Vec<CountMetric>,
    pub most_active_senders: Vec<CountMetric>,
    pub decision_breakdown: Vec<CountMetric>,
    pub status_breakdown: Vec<CountMetric>,
    pub top_tools_used: Vec<CountMetric>,
    #[serde(default)]
    pub top_usecases: Vec<CountMetric>,
    #[serde(default)]
    pub usecase_breakdown: Vec<CountMetric>,
    #[serde(default)]
    pub tool_call_frequency: Vec<CountMetric>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemDiagnostics {
    pub uptime_seconds: u64,
    pub memory_rss_bytes: u64,
    pub memory_virt_bytes: u64,
    pub memory_rss_human: String,
    pub memory_virt_human: String,
    pub db_size_bytes: u64,
    pub brain_dir_size_bytes: u64,
    pub total_conversations_in_brain: usize,
    pub active_model: String,
    pub bot_jid: String,
    pub bot_name: String,
    pub workspaces: Vec<WorkspaceProvenance>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scheduler: Option<crate::core::domain::SchedulerDiagnostics>,
    pub timestamp_epoch: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveTypingStatus {
    pub chat_jid: String,
    pub session_role: String,
    pub started_at_epoch: i64,
    pub last_beat_epoch: i64,
    pub duration_seconds: i64,
    pub heartbeat_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresenceAuditRecord {
    pub timestamp_epoch: i64,
    pub chat_jid: String,
    pub state: String, // "composing" or "paused"
    pub session_role: String,
    pub trigger: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PresenceAuditSnapshot {
    pub active_typing_count: usize,
    pub active_typing: Vec<ActiveTypingStatus>,
    pub recent_events: Vec<PresenceAuditRecord>,
}
