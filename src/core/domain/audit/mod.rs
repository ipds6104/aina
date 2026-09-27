//! Domain audit module: models, presence tracking, system telemetry, and transcript log parsing.

mod models;
mod presence_tracker;
mod system_telemetry;
mod transcript_parser;

pub use models::*;
pub use presence_tracker::*;
pub use system_telemetry::{get_process_uptime_secs, SystemTelemetry};
pub use transcript_parser::TranscriptParser;

use std::path::{Path, PathBuf};

/// Unified facade for audit trail queries, transcript extraction, and system telemetry.
pub struct AuditEngine;

impl AuditEngine {
    pub fn default_brain_path() -> PathBuf {
        TranscriptParser::default_brain_path()
    }

    pub fn find_transcripts<P: AsRef<Path>>(base_path: P) -> Vec<PathBuf> {
        TranscriptParser::find_transcripts(base_path)
    }

    pub fn query_audit_trail<P: AsRef<Path>>(
        brain_path: P,
        query: Option<&str>,
        errors_only: bool,
        limit: usize,
    ) -> Vec<AuditStep> {
        TranscriptParser::query_audit_trail(brain_path, query, errors_only, limit)
    }

    pub fn find_transcript_path<P: AsRef<Path>>(brain_path: P, conversation_id: &str) -> Option<PathBuf> {
        TranscriptParser::find_transcript_path(brain_path, conversation_id)
    }

    #[allow(dead_code)]
    pub fn parse_transcript_file<P: AsRef<Path>>(path: P) -> (Vec<TranscriptStepDetail>, Vec<String>) {
        TranscriptParser::parse_transcript_file(path)
    }

    pub fn load_transcript_for_conversation<P: AsRef<Path>>(
        brain_path: P,
        conversation_id: &str,
    ) -> (Vec<TranscriptStepDetail>, Vec<String>) {
        TranscriptParser::load_transcript_for_conversation(brain_path, conversation_id)
    }

    pub fn extract_tools_for_conversation<P: AsRef<Path>>(
        brain_path: P,
        conversation_id: &str,
    ) -> Vec<String> {
        TranscriptParser::extract_tools_for_conversation(brain_path, conversation_id)
    }

    #[allow(dead_code)]
    pub fn extract_tool_frequencies_for_conversation<P: AsRef<Path>>(
        brain_path: P,
        conversation_id: &str,
    ) -> Vec<CountMetric> {
        TranscriptParser::extract_tool_frequencies_for_conversation(brain_path, conversation_id)
    }

    pub fn inspect_workspaces<P: AsRef<Path>>(workspace_dir: P) -> Vec<WorkspaceProvenance> {
        SystemTelemetry::inspect_workspaces(workspace_dir)
    }

    #[allow(dead_code)]
    pub fn get_git_info_for_dir<P: AsRef<Path>>(dir: P) -> (bool, Option<String>, Option<String>, bool) {
        SystemTelemetry::get_git_info_for_dir(dir)
    }

    pub fn read_process_memory() -> (u64, u64) {
        SystemTelemetry::read_process_memory()
    }

    pub fn format_bytes(bytes: u64) -> String {
        SystemTelemetry::format_bytes(bytes)
    }

    pub fn compute_dir_size<P: AsRef<Path>>(path: P) -> u64 {
        SystemTelemetry::compute_dir_size(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_bytes() {
        assert_eq!(AuditEngine::format_bytes(500), "500 B");
        assert_eq!(AuditEngine::format_bytes(2048), "2.00 KB");
        assert_eq!(AuditEngine::format_bytes(10 * 1024 * 1024), "10.00 MB");
        assert_eq!(AuditEngine::format_bytes(3 * 1024 * 1024 * 1024), "3.00 GB");
    }

    #[test]
    fn test_parse_transcript_file() {
        let temp_dir = std::env::temp_dir().join(format!("aina_test_audit_parse_{}", rand::random::<u32>()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let log_path = temp_dir.join("transcript.jsonl");

        let sample_jsonl = r#"{"step_index":0,"source":"USER_EXPLICIT","type":"USER_INPUT","status":"DONE","created_at":"2026-09-20T00:00:00Z","content":"Audit test request"}
{"step_index":1,"source":"MODEL","type":"PLANNER_RESPONSE","status":"DONE","created_at":"2026-09-20T00:00:01Z","tool_calls":[{"name":"run_command","args":{"CommandLine":"\"ls -la\"","toolAction":"\"Listing files\"","toolSummary":"\"File list\""}},{"name":"view_file","args":{"AbsolutePath":"/root/test.txt"}}]}
"#;
        std::fs::write(&log_path, sample_jsonl).unwrap();

        let (steps, tools) = AuditEngine::parse_transcript_file(&log_path);
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0].summary, "Audit test request");
        assert_eq!(steps[1].tool_calls.len(), 2);
        assert_eq!(steps[1].tool_calls[0].name, "run_command");
        assert_eq!(steps[1].tool_calls[0].tool_action.as_deref(), Some("Listing files"));
        assert_eq!(steps[1].tool_calls[0].tool_summary.as_deref(), Some("File list"));
        assert_eq!(tools, vec!["run_command".to_string(), "view_file".to_string()]);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_read_process_memory() {
        let (rss, virt) = AuditEngine::read_process_memory();
        assert!(rss > 0);
        assert!(virt >= rss);
    }

    #[tokio::test]
    async fn test_presence_tracker_lifecycle() {
        let tracker = PresenceTracker::new();

        // 1. Record composing
        tracker.record("user1@s.whatsapp.net", "composing", "PrimaryBot", "start").await;
        tracker.record("user1@s.whatsapp.net", "composing", "PrimaryBot", "heartbeat").await;

        let snap = tracker.snapshot().await;
        assert_eq!(snap.active_typing_count, 1);
        assert_eq!(snap.active_typing[0].chat_jid, "user1@s.whatsapp.net");
        assert_eq!(snap.active_typing[0].heartbeat_count, 2);
        assert_eq!(snap.recent_events.len(), 2);

        // 2. Record paused
        tracker.record("user1@s.whatsapp.net", "paused", "PrimaryBot", "completed").await;
        let snap2 = tracker.snapshot().await;
        assert_eq!(snap2.active_typing_count, 0);
        assert_eq!(snap2.recent_events.len(), 3);
        assert_eq!(snap2.recent_events[0].state, "paused");

        // 3. Clear active
        tracker.record("user2@s.whatsapp.net", "composing", "PrimaryBot", "start").await;
        let cleared = tracker.clear_active(Some("user2@s.whatsapp.net")).await;
        assert_eq!(cleared, vec!["user2@s.whatsapp.net"]);
        assert_eq!(tracker.snapshot().await.active_typing_count, 0);
    }
}
