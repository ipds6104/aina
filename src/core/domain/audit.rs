use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use tracing::debug;

static PROCESS_START_INSTANT: OnceLock<std::time::Instant> = OnceLock::new();

pub fn get_process_uptime_secs() -> u64 {
    let start = PROCESS_START_INSTANT.get_or_init(std::time::Instant::now);
    start.elapsed().as_secs()
}

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
    pub created_at_epoch: i64,
    pub completed_at_epoch: Option<i64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ActionAuditFilter {
    pub chat_jid: Option<String>,
    pub sender_jid: Option<String>,
    pub decision: Option<String>,
    pub status: Option<String>,
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
    pub timestamp_epoch: i64,
}

pub struct AuditEngine;

impl AuditEngine {
    /// Resolves the default brain directory path from APP_DATA_DIR or system default.
    pub fn default_brain_path() -> PathBuf {
        if let Ok(dir) = std::env::var("APP_DATA_DIR") {
            let p = PathBuf::from(dir);
            if p.ends_with("brain") {
                p
            } else {
                p.join("brain")
            }
        } else {
            PathBuf::from("/root/.gemini/antigravity-cli/brain")
        }
    }

    /// Finds all Antigravity CLI transcript files.
    pub fn find_transcripts<P: AsRef<Path>>(base_path: P) -> Vec<PathBuf> {
        let mut files = Vec::new();
        let brain_dir = base_path.as_ref();
        if !brain_dir.is_dir() {
            return files;
        }

        if let Ok(conv_entries) = std::fs::read_dir(brain_dir) {
            for c_entry in conv_entries.flatten() {
                let p = c_entry.path();
                if p.is_dir() {
                    let transcript = p
                        .join(".system_generated")
                        .join("logs")
                        .join("transcript.jsonl");
                    if transcript.is_file() {
                        files.push(transcript);
                    }
                }
            }
        }

        files
    }

    /// Searches and queries audit trail steps across all transcripts.
    pub fn query_audit_trail<P: AsRef<Path>>(
        brain_path: P,
        query: Option<&str>,
        errors_only: bool,
        limit: usize,
    ) -> Vec<AuditStep> {
        let transcripts = Self::find_transcripts(brain_path);
        let mut steps = Vec::new();

        for path in transcripts {
            let session_id = path
                .parent()
                .and_then(|p| p.parent())
                .and_then(|p| p.file_name())
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "unknown".to_string());

            if let Ok(content) = std::fs::read_to_string(&path) {
                for line in content.lines() {
                    if line.trim().is_empty() {
                        continue;
                    }
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(line) {
                        let status = val.get("status").and_then(|s| s.as_str()).unwrap_or("");
                        if errors_only && status != "ERROR" {
                            continue;
                        }

                        let content_str = val.get("content").and_then(|c| c.as_str()).unwrap_or("");
                        let tool_calls = val.get("tool_calls").and_then(|t| t.as_array());
                        let tool_calls_count = tool_calls.map(|t| t.len()).unwrap_or(0);

                        // If query is provided, match in content or tool calls
                        if let Some(q) = query {
                            let q_lower = q.to_lowercase();
                            let content_matches = content_str.to_lowercase().contains(&q_lower);
                            let tools_match = tool_calls
                                .map(|arr| {
                                    arr.iter().any(|item| {
                                        item.to_string().to_lowercase().contains(&q_lower)
                                    })
                                })
                                .unwrap_or(false);

                            if !content_matches && !tools_match {
                                continue;
                            }
                        }

                        let summary = if !content_str.is_empty() {
                            let first_line = content_str.lines().next().unwrap_or("");
                            if first_line.chars().count() > 100 {
                                let trunc: String = first_line.chars().take(97).collect();
                                format!("{}...", trunc)
                            } else {
                                first_line.to_string()
                            }
                        } else if let Some(tools) = tool_calls {
                            let tool_names: Vec<String> = tools
                                .iter()
                                .filter_map(|t| t.get("name").and_then(|n| n.as_str()))
                                .map(String::from)
                                .collect();
                            format!("Tools: {}", tool_names.join(", "))
                        } else {
                            format!("Step status: {}", status)
                        };

                        steps.push(AuditStep {
                            session_id: session_id.clone(),
                            step_index: val.get("step_index").and_then(|i| i.as_i64()),
                            step_type: val
                                .get("type")
                                .and_then(|t| t.as_str())
                                .map(String::from),
                            source: val
                                .get("source")
                                .and_then(|s| s.as_str())
                                .map(String::from),
                            status: if status.is_empty() {
                                None
                            } else {
                                Some(status.to_string())
                            },
                            created_at: val
                                .get("created_at")
                                .and_then(|c| c.as_str())
                                .map(String::from),
                            summary,
                            tool_calls_count,
                        });
                    }
                }
            }
        }

        // Sort by created_at descending
        steps.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        if steps.len() > limit {
            steps.truncate(limit);
        }

        debug!("Audit query returned {} steps", steps.len());
        steps
    }

    /// Finds the transcript file for a specific conversation ID.
    pub fn find_transcript_path<P: AsRef<Path>>(brain_path: P, conversation_id: &str) -> Option<PathBuf> {
        let path = brain_path
            .as_ref()
            .join(conversation_id)
            .join(".system_generated")
            .join("logs")
            .join("transcript.jsonl");
        if path.is_file() {
            Some(path)
        } else {
            None
        }
    }

    /// Parses a transcript file into detailed step objects and extracts unique tool names.
    pub fn parse_transcript_file<P: AsRef<Path>>(path: P) -> (Vec<TranscriptStepDetail>, Vec<String>) {
        let mut steps = Vec::new();
        let mut tool_names_set = Vec::new();

        if let Ok(content) = std::fs::read_to_string(path.as_ref()) {
            for line in content.lines() {
                if line.trim().is_empty() {
                    continue;
                }
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(line) {
                    let step_index = val.get("step_index").and_then(|i| i.as_i64());
                    let step_type = val.get("type").and_then(|t| t.as_str()).map(String::from);
                    let source = val.get("source").and_then(|s| s.as_str()).map(String::from);
                    let status = val.get("status").and_then(|s| s.as_str()).map(String::from);
                    let created_at = val.get("created_at").and_then(|c| c.as_str()).map(String::from);

                    let content_str = val.get("content").and_then(|c| c.as_str()).unwrap_or("");
                    let raw_tools = val.get("tool_calls").and_then(|t| t.as_array());

                    let mut tool_calls = Vec::new();
                    if let Some(arr) = raw_tools {
                        for t in arr {
                            let name = t.get("name").and_then(|n| n.as_str()).unwrap_or("unknown").to_string();
                            if !tool_names_set.contains(&name) {
                                tool_names_set.push(name.clone());
                            }

                            let args = t.get("args").or_else(|| t.get("arguments")).or_else(|| t.get("parameters"));
                            let clean_str = |val_opt: Option<&serde_json::Value>| -> Option<String> {
                                val_opt.and_then(|v| {
                                    v.as_str().map(|s| {
                                        let trimmed = s.trim();
                                        trimmed.strip_prefix('"').and_then(|x| x.strip_suffix('"')).unwrap_or(trimmed).to_string()
                                    })
                                })
                            };

                            let tool_action = args.and_then(|a| clean_str(a.get("toolAction")));
                            let tool_summary = args.and_then(|a| clean_str(a.get("toolSummary")));

                            tool_calls.push(ToolCallDetail {
                                name,
                                tool_action,
                                tool_summary,
                                arguments: args.cloned(),
                            });
                        }
                    }

                    let summary = if !content_str.is_empty() {
                        let first_line = content_str.lines().next().unwrap_or("");
                        if first_line.chars().count() > 100 {
                            let trunc: String = first_line.chars().take(97).collect();
                            format!("{}...", trunc)
                        } else {
                            first_line.to_string()
                        }
                    } else if !tool_calls.is_empty() {
                        let names: Vec<&str> = tool_calls.iter().map(|t| t.name.as_str()).collect();
                        format!("Tools: {}", names.join(", "))
                    } else {
                        format!("Step: {}", step_type.as_deref().unwrap_or("unknown"))
                    };

                    steps.push(TranscriptStepDetail {
                        step_index,
                        step_type,
                        source,
                        status,
                        created_at,
                        summary,
                        tool_calls,
                    });
                }
            }
        }

        (steps, tool_names_set)
    }

    /// Loads transcript details for a specific conversation ID.
    pub fn load_transcript_for_conversation<P: AsRef<Path>>(
        brain_path: P,
        conversation_id: &str,
    ) -> (Vec<TranscriptStepDetail>, Vec<String>) {
        if let Some(p) = Self::find_transcript_path(brain_path, conversation_id) {
            Self::parse_transcript_file(&p)
        } else {
            (Vec::new(), Vec::new())
        }
    }

    /// Extracts unique tool names invoked during a conversation.
    pub fn extract_tools_for_conversation<P: AsRef<Path>>(
        brain_path: P,
        conversation_id: &str,
    ) -> Vec<String> {
        let (_, tools) = Self::load_transcript_for_conversation(brain_path, conversation_id);
        tools
    }

    /// Inspects workspaces and extracts git repository provenance and script count.
    pub fn inspect_workspaces<P: AsRef<Path>>(workspace_dir: P) -> Vec<WorkspaceProvenance> {
        let mut provenances = Vec::new();
        let ws_path = workspace_dir.as_ref();

        // 1. Inspect the main workspace directory
        if ws_path.is_dir() {
            let name = ws_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "default".to_string());
            let (is_git, commit, branch, is_dirty) = Self::get_git_info_for_dir(ws_path);
            let scripts_count = ws_path
                .join("scripts")
                .read_dir()
                .map(|e| e.flatten().filter(|f| f.path().is_file()).count())
                .unwrap_or(0);

            provenances.push(WorkspaceProvenance {
                name,
                path: ws_path.to_string_lossy().to_string(),
                is_git_repo: is_git,
                git_commit: commit,
                git_branch: branch,
                is_dirty,
                scripts_count,
            });

            // 2. If workspace_dir is inside a parent `workspaces` directory, check sibling workspaces
            if let Some(parent) = ws_path.parent() {
                if parent.is_dir() && parent.file_name().map(|n| n == "workspaces").unwrap_or(false) {
                    if let Ok(siblings) = std::fs::read_dir(parent) {
                        for entry in siblings.flatten() {
                            let p = entry.path();
                            if p.is_dir() && p != ws_path {
                                let sib_name = p
                                    .file_name()
                                    .map(|n| n.to_string_lossy().to_string())
                                    .unwrap_or_else(|| "sibling".to_string());
                                let (s_git, s_commit, s_branch, s_dirty) = Self::get_git_info_for_dir(&p);
                                let s_scripts = p
                                    .join("scripts")
                                    .read_dir()
                                    .map(|e| e.flatten().filter(|f| f.path().is_file()).count())
                                    .unwrap_or(0);

                                provenances.push(WorkspaceProvenance {
                                    name: sib_name,
                                    path: p.to_string_lossy().to_string(),
                                    is_git_repo: s_git,
                                    git_commit: s_commit,
                                    git_branch: s_branch,
                                    is_dirty: s_dirty,
                                    scripts_count: s_scripts,
                                });
                            }
                        }
                    }
                }
            }
        }

        provenances
    }

    /// Gets git commit, branch, and dirty status for a directory.
    pub fn get_git_info_for_dir<P: AsRef<Path>>(dir: P) -> (bool, Option<String>, Option<String>, bool) {
        let p = dir.as_ref();
        let has_git_folder = p.join(".git").exists();

        let commit_out = std::process::Command::new("git")
            .arg("-C")
            .arg(p)
            .arg("rev-parse")
            .arg("--short")
            .arg("HEAD")
            .output();

        let commit = commit_out.ok().and_then(|o| {
            if o.status.success() {
                let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
                if s.is_empty() {
                    None
                } else {
                    Some(s)
                }
            } else {
                None
            }
        });

        let branch_out = std::process::Command::new("git")
            .arg("-C")
            .arg(p)
            .arg("rev-parse")
            .arg("--abbrev-ref")
            .arg("HEAD")
            .output();

        let branch = branch_out.ok().and_then(|o| {
            if o.status.success() {
                let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
                if s.is_empty() {
                    None
                } else {
                    Some(s)
                }
            } else {
                None
            }
        });

        let status_out = std::process::Command::new("git")
            .arg("-C")
            .arg(p)
            .arg("status")
            .arg("--porcelain")
            .output();

        let is_dirty = status_out
            .ok()
            .map(|o| o.status.success() && !o.stdout.is_empty())
            .unwrap_or(false);

        let is_git = has_git_folder || commit.is_some();
        (is_git, commit, branch, is_dirty)
    }

    /// Reads resident set size (RSS) and total virtual memory (VIRT) in bytes from /proc/self/statm.
    pub fn read_process_memory() -> (u64, u64) {
        if let Ok(content) = std::fs::read_to_string("/proc/self/statm") {
            let parts: Vec<&str> = content.split_whitespace().collect();
            if parts.len() >= 2 {
                let total_pages = parts[0].parse::<u64>().unwrap_or(0);
                let resident_pages = parts[1].parse::<u64>().unwrap_or(0);
                let page_size = 4096u64;
                return (resident_pages * page_size, total_pages * page_size);
            }
        }
        (0, 0)
    }

    /// Formats byte counts into human-readable strings (e.g. "12.45 MB").
    pub fn format_bytes(bytes: u64) -> String {
        const KB: u64 = 1024;
        const MB: u64 = 1024 * KB;
        const GB: u64 = 1024 * MB;

        if bytes >= GB {
            format!("{:.2} GB", bytes as f64 / GB as f64)
        } else if bytes >= MB {
            format!("{:.2} MB", bytes as f64 / MB as f64)
        } else if bytes >= KB {
            format!("{:.2} KB", bytes as f64 / KB as f64)
        } else {
            format!("{} B", bytes)
        }
    }

    /// Computes total size of a directory in bytes recursively.
    pub fn compute_dir_size<P: AsRef<Path>>(path: P) -> u64 {
        let mut total = 0u64;
        let mut stack = vec![path.as_ref().to_path_buf()];

        while let Some(current) = stack.pop() {
            if let Ok(entries) = std::fs::read_dir(&current) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.is_symlink() {
                        continue;
                    }
                    if p.is_dir() {
                        stack.push(p);
                    } else if let Ok(meta) = entry.metadata() {
                        total += meta.len();
                    }
                }
            }
        }
        total
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
        // In linux /proc/self/statm, memory should be non-zero
        assert!(rss > 0);
        assert!(virt >= rss);
    }
}
