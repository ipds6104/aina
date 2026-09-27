//! Periodic deterministic Knowledge Base neatness checking and auto-healing.

use crate::core::ports::KnowledgePort;
use std::path::Path;
use tracing::info;

pub struct KnowledgeLinter;

impl KnowledgeLinter {
    /// Inspects and auto-heals Markdown knowledge documents if the workspace knowledge directory exists.
    pub fn lint_workspace(knowledge_port: &dyn KnowledgePort, workspace_dir: Option<&Path>) {
        if let Some(ws) = workspace_dir {
            if ws.join("knowledge").is_dir() {
                let report = knowledge_port.lint(ws, true);
                if report.auto_healed {
                    info!(
                        "Scheduler auto-healed knowledge base for workspace {:?}",
                        ws
                    );
                } else if !report.is_clean {
                    info!(
                        "Scheduler found {} knowledge base violations in {:?}",
                        report.violations_count, ws
                    );
                }
            }
        }
    }
}
