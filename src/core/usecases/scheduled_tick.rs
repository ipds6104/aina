use crate::core::ports::{SessionStorePort, WhatsAppPort};
use std::sync::Arc;
use tracing::debug;

pub struct ScheduledTickUseCase {
    _session_store: Arc<dyn SessionStorePort>,
    _whatsapp: Arc<dyn WhatsAppPort>,
}

impl ScheduledTickUseCase {
    pub fn new(
        session_store: Arc<dyn SessionStorePort>,
        whatsapp: Arc<dyn WhatsAppPort>,
    ) -> Self {
        Self {
            _session_store: session_store,
            _whatsapp: whatsapp,
        }
    }

    pub async fn execute(&self) -> anyhow::Result<()> {
        debug!("Running periodic scheduler tick...");
        // Placeholder for scheduled jobs: e.g. morning greetings, pending task followups, etc.
        Ok(())
    }
}
