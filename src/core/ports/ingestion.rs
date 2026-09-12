#![allow(dead_code)]

use crate::core::domain::sensor::{IngestionOutcome, IngestionSourceSpec, RawContextItem};
use async_trait::async_trait;

#[async_trait]
pub trait KnowledgeIngestionPort: Send + Sync {
    /// Ingests an incoming raw item through privacy filters and records into knowledge base
    async fn ingest_context(&self, item: RawContextItem) -> anyhow::Result<IngestionOutcome>;
}

#[async_trait]
pub trait SourceRegistryPort: Send + Sync {
    /// Retrieves a source configuration by its unique identifier
    async fn get_source(&self, source_id: &str) -> Option<IngestionSourceSpec>;

    /// Finds a registered sensor matching an incoming transport identifier (e.g. JID, token, channel)
    async fn find_by_transport(&self, transport_id: &str) -> Option<IngestionSourceSpec>;

    /// Lists all active registered ingestion sensors
    async fn list_active_sensors(&self) -> Vec<IngestionSourceSpec>;
}
