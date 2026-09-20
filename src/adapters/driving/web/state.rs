use crate::core::domain::{IncomingMessage, PersonaEngine};
use crate::core::ports::{AgentEnginePort, SessionStorePort};
use crate::core::usecases::ProcessIncomingMessageUseCase;
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum SimulationJobStatus {
    Processing { started_at: i64 },
    Completed {
        decision: String,
        reason: String,
        response_text: Option<String>,
        duration_seconds: Option<f64>,
        conversation_id: Option<String>,
        finished_at: i64,
    },
    Failed {
        error: String,
        finished_at: i64,
    },
}

#[derive(Debug, Clone)]
pub struct SimulationJob {
    #[allow(dead_code)]
    pub id: String,
    pub status: SimulationJobStatus,
}

#[derive(Clone)]
pub struct ActiveTaskInfo {
    pub abort_handle: tokio::task::AbortHandle,
    pub started_at_epoch: i64,
    pub input_text: String,
}

pub struct WebhookServerState {
    pub usecase: Arc<ProcessIncomingMessageUseCase>,
    pub agent_engine: Arc<dyn AgentEnginePort>,
    pub session_store: Arc<dyn SessionStorePort>,
    pub persona_engine: Arc<PersonaEngine>,
    pub bot_name: String,
    pub bot_jid: String,
    pub bot_lid: Option<String>,
    pub companion_jid: Option<String>,
    pub companion_name: Option<String>,
    pub companion_session_id: Option<String>,
    #[allow(dead_code)]
    pub model: String,
    pub whatsmeow_url: String,
    pub whatsmeow_api_key: String,
    pub companion_base_url: Option<String>,
    pub companion_api_key: Option<String>,
    pub setup_code: String,
    pub timezone: String,
    pub locale: String,
    pub sim_jobs: Arc<RwLock<HashMap<String, SimulationJob>>>,
    pub chat_queues: Arc<tokio::sync::Mutex<HashMap<String, tokio::sync::mpsc::UnboundedSender<IncomingMessage>>>>,
    pub active_tasks: Arc<tokio::sync::Mutex<HashMap<String, ActiveTaskInfo>>>,
    pub workspace_dir: PathBuf,
}
