use crate::core::domain::KnowledgeEngine;
use crate::core::ports::{SessionStorePort, WhatsAppPort};
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{debug, info};

pub struct ScheduledTickUseCase {
    _session_store: Arc<dyn SessionStorePort>,
    _whatsapp: Arc<dyn WhatsAppPort>,
    workspace_dir: Option<PathBuf>,
}

impl ScheduledTickUseCase {
    pub fn new(
        session_store: Arc<dyn SessionStorePort>,
        whatsapp: Arc<dyn WhatsAppPort>,
        workspace_dir: Option<PathBuf>,
    ) -> Self {
        Self {
            _session_store: session_store,
            _whatsapp: whatsapp,
            workspace_dir,
        }
    }

    pub async fn execute(&self) -> anyhow::Result<()> {
        debug!("Running periodic scheduler tick...");

        // 1. In-process deterministic Knowledge Base neatness check & auto-heal
        if let Some(ref ws) = self.workspace_dir {
            if ws.join("knowledge").is_dir() {
                let report = KnowledgeEngine::lint(ws, true);
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

        Ok(())
    }
}
