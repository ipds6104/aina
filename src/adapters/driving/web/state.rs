use crate::config::settings::OidcConfig;
use crate::core::domain::{IncomingMessage, PersonaEngine};
use crate::core::ports::{AgentEnginePort, SessionStorePort};
use crate::core::usecases::ProcessIncomingMessageUseCase;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
pub struct OidcPendingState {
    pub code_verifier: String,
    pub created_at: i64,
    pub return_to: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthenticatedUserSession {
    pub sub: String,
    pub name: String,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub preferred_username: Option<String>,
    #[serde(default)]
    pub nip: Option<String>,
    #[serde(default)]
    pub unit_kerja: Option<String>,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub is_admin: bool,
    pub exp: i64,
}

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
}

/// Default bounded capacity for each per-chat FIFO message queue (Phase 19).
pub const CHAT_QUEUE_CAPACITY: usize = 64;

#[derive(Debug, Clone)]
struct EdgeWindow {
    window_start: i64,
    count: u32,
    last_seen: i64,
}

/// Layer 1 pre-media & pre-queue webhook flood guard per sender JID.
/// Uses `std` + `tokio::sync::Mutex` (zero external crates) with bounded memory.
pub struct EdgeIngressGuard {
    window_secs: i64,
    max_events: u32,
    stale_ttl_secs: i64,
    max_entries: usize,
    windows: tokio::sync::Mutex<HashMap<String, EdgeWindow>>,
}

impl Default for EdgeIngressGuard {
    fn default() -> Self {
        Self::from_env()
    }
}

impl EdgeIngressGuard {
    pub fn new(window_secs: i64, max_events: u32, max_entries: usize) -> Self {
        Self {
            window_secs: window_secs.max(1),
            max_events: max_events.max(1),
            stale_ttl_secs: 300,
            max_entries: max_entries.max(1),
            windows: tokio::sync::Mutex::new(HashMap::new()),
        }
    }

    pub fn from_env() -> Self {
        let max_events = std::env::var("AINA_EDGE_BURST_LIMIT")
            .ok()
            .and_then(|v| v.trim().parse::<u32>().ok())
            .filter(|&n| n >= 1)
            .unwrap_or(20);
        Self::new(10, max_events, 10_000)
    }

    pub async fn allow(&self, sender_jid: &str) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        self.allow_at(sender_jid, now).await
    }

    pub async fn allow_at(&self, sender_jid: &str, now_epoch: i64) -> bool {
        let key = sender_jid.trim().to_ascii_lowercase();
        let mut map = self.windows.lock().await;

        if !map.contains_key(&key) && map.len() >= self.max_entries {
            let stale_ttl = self.stale_ttl_secs;
            map.retain(|_, w| now_epoch.saturating_sub(w.last_seen) <= stale_ttl);
            if map.len() >= self.max_entries {
                if let Some(oldest_key) = map
                    .iter()
                    .min_by_key(|(_, w)| w.last_seen)
                    .map(|(k, _)| k.clone())
                {
                    map.remove(&oldest_key);
                }
            }
        }

        let entry = map.entry(key).or_insert(EdgeWindow {
            window_start: now_epoch,
            count: 0,
            last_seen: now_epoch,
        });

        entry.last_seen = now_epoch;
        if now_epoch.saturating_sub(entry.window_start) >= self.window_secs {
            entry.window_start = now_epoch;
            entry.count = 1;
            return true;
        }

        entry.count = entry.count.saturating_add(1);
        entry.count <= self.max_events
    }
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
    pub events_api_key: Option<String>,
    pub events_allow_query_key: bool,
    pub webhook_secret: Option<String>,
    pub trust_proxy_headers: bool,
    pub ephemeral_session_secret: String,
    pub auth_attempt_limiter: Arc<super::auth::AuthAttemptLimiter>,
    pub edge_ingress_guard: Arc<EdgeIngressGuard>,
    pub timezone: String,
    pub locale: String,
    pub sim_jobs: Arc<RwLock<HashMap<String, SimulationJob>>>,
    pub chat_queues: Arc<tokio::sync::Mutex<HashMap<String, tokio::sync::mpsc::Sender<IncomingMessage>>>>,
    pub active_tasks: Arc<tokio::sync::Mutex<HashMap<String, ActiveTaskInfo>>>,
    pub presence_tracker: Arc<crate::core::domain::PresenceTracker>,
    pub workspace_dir: PathBuf,
    /// Root directory of the agent transcript "brain".
    ///
    /// Injected instead of resolved on demand so tests can point it at a
    /// temporary directory. Walking the real per-user brain made the audit
    /// diagnostics endpoint (and its test) depend on ambient machine state.
    pub brain_dir: PathBuf,
    pub oidc_config: OidcConfig,
    pub oidc_states: Arc<RwLock<HashMap<String, OidcPendingState>>>,
}
