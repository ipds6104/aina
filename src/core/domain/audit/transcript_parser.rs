//! JSONL transcript parsing and audit trail query engine.

use super::models::{AuditStep, CountMetric, ToolCallDetail, TranscriptStepDetail};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::debug;

pub struct TranscriptParser;

impl TranscriptParser {
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

    /// Extracts tool names and total invocation counts for a conversation.
    pub fn extract_tool_frequencies_for_conversation<P: AsRef<Path>>(
        brain_path: P,
        conversation_id: &str,
    ) -> Vec<CountMetric> {
        let (steps, _) = Self::load_transcript_for_conversation(brain_path, conversation_id);
        let mut counts = HashMap::new();
        for step in steps {
            for tool in step.tool_calls {
                *counts.entry(tool.name).or_insert(0i64) += 1;
            }
        }
        let mut list: Vec<CountMetric> = counts
            .into_iter()
            .map(|(key, count)| CountMetric { key, count })
            .collect();
        list.sort_by(|a, b| b.count.cmp(&a.count));
        list
    }
}
