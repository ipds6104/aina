use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tracing::debug;

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

pub struct AuditEngine;

impl AuditEngine {
    /// Find all Antigravity CLI transcript files
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

    /// Search and query audit trail steps
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
}
