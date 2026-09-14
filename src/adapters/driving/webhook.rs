use axum::{
    extract::{DefaultBodyLimit, Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse},
    routing::{get, post},
    Json, Router,
};
use crate::core::domain::{ChatType, Gatekeeper, GatekeeperDecision, IncomingMessage, PersonaEngine, QuotedMessage, Sender};
use crate::core::ports::{AgentEnginePort, SessionStorePort};
use crate::core::usecases::ProcessIncomingMessageUseCase;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

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
    pub workspace_dir: PathBuf,
}

pub fn create_router(state: Arc<WebhookServerState>) -> Router {
    Router::new()
        .route("/", get(dashboard_or_setup_handler))
        .route("/setup", get(setup_page_handler))
        .route("/health", get(health_handler))
        .route("/api/status", get(api_status_handler))
        .route("/api/models", get(api_get_models_handler))
        .route("/api/model", post(api_set_model_handler))
        .route("/api/setup", post(api_setup_handler))
        .route("/api/auth/verify", post(api_verify_admin_handler))
        .route("/api/simulate", post(simulate_handler))
        .route("/api/simulate/reset", post(simulate_reset_handler))
        .route("/api/simulate/job/{id}", get(simulate_job_status_handler))
        .route("/webhook", post(webhook_handler))
        .layer(DefaultBodyLimit::max(100 * 1024 * 1024))
        .with_state(state)
}

async fn health_handler() -> impl IntoResponse {
    (StatusCode::OK, "Aina is running smoothly")
}

#[derive(Debug, Serialize)]
struct ApiStatusResponse {
    pub authenticated: bool,
    pub bot_name: String,
    pub bot_jid: String,
    pub companion_jid: Option<String>,
    pub companion_name: Option<String>,
    pub companion_active: bool,
    pub model: String,
    pub whatsmeow_url: String,
    pub companion_url: Option<String>,
    pub timezone: String,
    pub locale: String,
}

async fn api_status_handler(
    State(state): State<Arc<WebhookServerState>>,
) -> impl IntoResponse {
    let auth = state.agent_engine.is_authenticated().await;
    let live_model = state.agent_engine.get_model().await;
    let companion_active = state.companion_jid.is_some();
    Json(ApiStatusResponse {
        authenticated: auth,
        bot_name: state.bot_name.clone(),
        bot_jid: state.bot_jid.clone(),
        companion_jid: state.companion_jid.clone(),
        companion_name: state.companion_name.clone(),
        companion_active,
        model: live_model,
        whatsmeow_url: state.whatsmeow_url.clone(),
        companion_url: state.companion_base_url.clone(),
        timezone: state.timezone.clone(),
        locale: state.locale.clone(),
    })
}

#[derive(Debug, Serialize)]
struct ModelOption {
    pub id: &'static str,
    pub name: &'static str,
}

#[derive(Debug, Serialize)]
struct ApiModelsResponse {
    pub current: String,
    pub available: Vec<ModelOption>,
}

async fn api_get_models_handler(
    State(state): State<Arc<WebhookServerState>>,
) -> impl IntoResponse {
    let current = state.agent_engine.get_model().await;
    let available = crate::adapters::driven::get_available_models()
        .into_iter()
        .map(|(id, name)| ModelOption { id, name })
        .collect();

    Json(ApiModelsResponse { current, available })
}

#[derive(Debug, Deserialize)]
struct SetModelRequest {
    pub model: String,
    pub admin_key: Option<String>,
}

async fn api_set_model_handler(
    State(state): State<Arc<WebhookServerState>>,
    headers: HeaderMap,
    Json(payload): Json<SetModelRequest>,
) -> impl IntoResponse {
    let authorized = is_admin_authorized(&headers, &state.setup_code)
        || payload.admin_key.as_deref().map(|k| k.trim() == state.setup_code.trim()).unwrap_or(false);

    if !authorized {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "success": false,
                "error": "Akses ditolak. Masukkan Admin Key / Setup Code yang valid."
            })),
        );
    }

    match state.agent_engine.set_model(&payload.model).await {
        Ok(_) => {
            let active = state.agent_engine.get_model().await;
            info!("Aina default model updated to: {}", active);
            (
                StatusCode::OK,
                Json(json!({
                    "success": true,
                    "model": active,
                    "message": format!("Model aktif berhasil diubah ke {}", active)
                })),
            )
        }
        Err(e) => {
            (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "success": false,
                    "error": format!("{}", e)
                })),
            )
        }
    }
}

#[derive(Debug, Deserialize)]
struct SetupRequest {
    pub token: String,
    pub setup_code: String,
}

async fn api_setup_handler(
    State(state): State<Arc<WebhookServerState>>,
    Json(payload): Json<SetupRequest>,
) -> impl IntoResponse {
    info!("Received token configuration request via /api/setup");

    // Industry-Standard Security Check: Verify Setup Code / Admin Key
    if payload.setup_code.trim() != state.setup_code.trim() {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "success": false,
                "message": "Kode Setup / Admin Key salah! Masukkan kode yang tertera di log deployment server atau ADMIN_KEY Anda."
            })),
        );
    }
    
    match state.agent_engine.save_auth_token(&payload.token).await {
        Ok(_) => {
            (
                StatusCode::OK,
                Json(json!({
                    "success": true,
                    "message": "Token berhasil diverifikasi dan disimpan! Aina sekarang aktif."
                })),
            )
        }
        Err(e) => {
            error!("Setup token verification failed: {}", e);
            (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "success": false,
                    "message": format!("Verifikasi token gagal: {}", e)
                })),
            )
        }
    }
}

#[derive(Debug, Deserialize)]
struct VerifyAdminRequest {
    pub admin_key: String,
}

fn is_admin_authorized(headers: &HeaderMap, expected_code: &str) -> bool {
    let expected = expected_code.trim();
    if expected.is_empty() {
        return true;
    }

    if let Some(key) = headers.get("X-Admin-Key").and_then(|v| v.to_str().ok()) {
        if key.trim() == expected {
            return true;
        }
    }

    if let Some(auth) = headers.get("Authorization").and_then(|v| v.to_str().ok()) {
        if let Some(bearer) = auth.strip_prefix("Bearer ") {
            if bearer.trim() == expected {
                return true;
            }
        }
    }

    false
}

async fn api_verify_admin_handler(
    State(state): State<Arc<WebhookServerState>>,
    Json(payload): Json<VerifyAdminRequest>,
) -> impl IntoResponse {
    if payload.admin_key.trim() == state.setup_code.trim() {
        (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "message": "Autentikasi admin berhasil!"
            })),
        )
    } else {
        (
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "success": false,
                "message": "Admin Key / Setup Code salah! Periksa log server Anda."
            })),
        )
    }
}

#[derive(Debug, Deserialize)]
struct SimulateRequest {
    pub sender_name: Option<String>,
    pub sender_jid: Option<String>,
    pub chat_type: Option<String>,
    pub is_mention: Option<bool>,
    pub is_from_me: Option<bool>,
    pub session_role: Option<String>,
    pub text: String,
    pub model_override: Option<String>,
}

#[derive(Debug, Serialize)]
struct SimulateResponse {
    pub decision: String,
    pub reason: String,
    pub response_text: Option<String>,
    pub duration_seconds: Option<f64>,
    pub conversation_id: Option<String>,
}

async fn simulate_reset_handler(
    State(state): State<Arc<WebhookServerState>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if !is_admin_authorized(&headers, &state.setup_code) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "error": "Akses simulator ditolak. Harap masukkan Admin Key / Setup Code yang valid."
            })),
        );
    }

    let _ = state.session_store.delete_conversation_id("628999888777@s.whatsapp.net").await;
    let _ = state.session_store.delete_conversation_id("120363999999999@g.us").await;
    let _ = std::fs::remove_file(std::env::temp_dir().join("aina_gh_device_session.json"));

    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "message": "Sesi percakapan simulator berhasil direset!"
        })),
    )
}

async fn simulate_handler(
    State(state): State<Arc<WebhookServerState>>,
    headers: HeaderMap,
    Json(payload): Json<SimulateRequest>,
) -> impl IntoResponse {
    // 1. Enforce Admin Authentication
    if !is_admin_authorized(&headers, &state.setup_code) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "error": "Akses simulator ditolak. Harap masukkan Admin Key / Setup Code yang valid."
            })),
        );
    }

    info!("Running real end-to-end WhatsApp simulation");

    let is_auth = state.agent_engine.is_authenticated().await;
    if !is_auth {
        return (
            StatusCode::PRECONDITION_FAILED,
            Json(json!({
                "error": "Aina belum terautentikasi ke Google Antigravity. Silakan lakukan setup terlebih dahulu."
            })),
        );
    }

    // Built-in quick commands in simulator: /reset, /clear, /new
    let trimmed_text = payload.text.trim();
    if trimmed_text.eq_ignore_ascii_case("/reset")
        || trimmed_text.eq_ignore_ascii_case("/clear")
        || trimmed_text.eq_ignore_ascii_case("/new")
        || trimmed_text.eq_ignore_ascii_case("/restart")
    {
        let chat_jid = if payload.chat_type.as_deref().unwrap_or("dm").to_lowercase() == "group" {
            "120363999999999@g.us"
        } else {
            "628999888777@s.whatsapp.net"
        };
        let _ = state.session_store.delete_conversation_id(chat_jid).await;
        let _ = std::fs::remove_file(std::env::temp_dir().join("aina_gh_device_session.json"));
        let reply = "🔄 *Sesi Percakapan Simulator Berhasil Direset*\n\nMemori percakapan untuk sesi ini telah dibersihkan. Pesan berikutnya akan dimulai sebagai percakapan baru yang segar.".to_string();
        return (
            StatusCode::OK,
            Json(json!(SimulateResponse {
                decision: "Respond".to_string(),
                reason: "Quick /reset command".to_string(),
                response_text: Some(reply),
                duration_seconds: Some(0.01),
                conversation_id: None,
            })),
        );
    }
    if trimmed_text.starts_with("/model") {
        let parts: Vec<&str> = trimmed_text.split_whitespace().collect();
        if parts.len() == 1 || (parts.len() >= 2 && (parts[1] == "status" || parts[1] == "list")) {
            let current = state.agent_engine.get_model().await;
            let reply = format!(
                "🤖 *Status Model AI Aina*\n\nModel aktif saat ini: *{}*\n\n*Pilihan Model Tersedia:*\n• `gemini-3.8-flash-medium` (Default Cepat & Seimbang)\n• `gemini-3.8-flash-high` (Penalaran Tinggi / Deep Thinking)\n• `gemini-3.8-flash-low` (Respons Kilat & Kasual)\n• `gemini-3.1-pro-high` (Deep Coding & Arsitektur)\n• `claude-opus-4-6-thinking` (Claude Opus Thinking - Khusus Eksplisit)\n• `claude-sonnet-4-6` (Claude Sonnet 4.6)\n\n_Untuk mengganti model, ketik:_ `/model <nama_model>`",
                current
            );
            return (
                StatusCode::OK,
                Json(json!(SimulateResponse {
                    decision: "Respond".to_string(),
                    reason: "Quick /model status command".to_string(),
                    response_text: Some(reply),
                    duration_seconds: Some(0.01),
                    conversation_id: None,
                })),
            );
        } else if parts.len() >= 2 {
            let target_model = parts[1];
            match state.agent_engine.set_model(target_model).await {
                Ok(_) => {
                    let new_model = state.agent_engine.get_model().await;
                    let reply = format!(
                        "✅ *Model AI Berhasil Diubah*\n\nAina sekarang menggunakan model: *{}*.\nPengujian berikutnya akan diproses menggunakan mesin ini.",
                        new_model
                    );
                    return (
                        StatusCode::OK,
                        Json(json!(SimulateResponse {
                            decision: "Respond".to_string(),
                            reason: "Quick /model switch command".to_string(),
                            response_text: Some(reply),
                            duration_seconds: Some(0.01),
                            conversation_id: None,
                        })),
                    );
                }
                Err(e) => {
                    let reply = format!(
                        "⚠️ *Gagal Mengganti Model*\n\n{}\n\nContoh: `/model gemini-3.8-flash-medium`",
                        e
                    );
                    return (
                        StatusCode::OK,
                        Json(json!(SimulateResponse {
                            decision: "Respond".to_string(),
                            reason: "Quick /model error".to_string(),
                            response_text: Some(reply),
                            duration_seconds: Some(0.01),
                            conversation_id: None,
                        })),
                    );
                }
            }
        }
    }

    let is_from_me = payload.is_from_me.unwrap_or(false);
    let session_role = match payload.session_role.as_deref().unwrap_or("primary_bot").to_lowercase().as_str() {
        "user_companion" | "companion" => crate::core::domain::SessionRole::UserCompanion,
        _ => crate::core::domain::SessionRole::PrimaryBot,
    };

    let default_sender_jid = if session_role == crate::core::domain::SessionRole::UserCompanion && is_from_me {
        state.companion_jid.clone().unwrap_or_else(|| "628111222333@s.whatsapp.net".to_string())
    } else {
        "628999888777@s.whatsapp.net".to_string()
    };

    let sender_name = payload.sender_name.unwrap_or_else(|| {
        if is_from_me {
            state.companion_name.clone().unwrap_or_else(|| "Saya (Owner)".to_string())
        } else {
            "Pengguna Tester".to_string()
        }
    });
    let sender_jid = payload.sender_jid.unwrap_or(default_sender_jid);
    let is_group = payload.chat_type.as_deref().unwrap_or("dm").to_lowercase() == "group";
    let is_mention = payload.is_mention.unwrap_or(false);
    
    let chat_jid = if is_group {
        "120363999999999@g.us".to_string()
    } else {
        sender_jid.clone()
    };

    let chat_type = if is_group {
        ChatType::Group
    } else {
        ChatType::DirectMessage
    };

    let mentioned_jids = if is_group && is_mention {
        vec![state.bot_jid.clone()]
    } else {
        vec![]
    };

    let msg = IncomingMessage {
        id: format!("sim-{}", chrono_now_secs()),
        platform: crate::core::domain::Platform::WebSimulator,
        session_role,
        chat_jid: chat_jid.clone(),
        chat_type,
        sender: Sender {
            jid: sender_jid.clone(),
            name: Some(sender_name),
        },
        text: payload.text,
        timestamp: chrono_now_secs(),
        is_from_me,
        quoted_message: None,
        mentioned_jids,
        is_bot_mentioned: false,
        bot_lid: state.bot_lid.clone(),
        has_media: false,
        media_type: None,
        media_path: None,
    };

    let decision = Gatekeeper::evaluate(&msg, &state.bot_jid, &state.bot_name, state.bot_lid.as_deref());

    match decision {
        GatekeeperDecision::Ignore { reason } => {
            (
                StatusCode::OK,
                Json(json!(SimulateResponse {
                    decision: "Ignore".to_string(),
                    reason,
                    response_text: None,
                    duration_seconds: None,
                    conversation_id: None,
                })),
            )
        }
        GatekeeperDecision::RecordOnly { reason } => {
            let _ = state.session_store.record_message(&msg.chat_jid, &msg.sender.jid, &msg.text, false).await;
            (
                StatusCode::OK,
                Json(json!(SimulateResponse {
                    decision: "RecordOnly".to_string(),
                    reason,
                    response_text: None,
                    duration_seconds: None,
                    conversation_id: None,
                })),
            )
        }
        GatekeeperDecision::Respond { reason } => {
            let _ = state.session_store.record_message(&msg.chat_jid, &msg.sender.jid, &msg.text, false).await;
            
            let conv_id = match state.session_store.get_conversation_id(&msg.chat_jid).await {
                Ok(id) => id,
                Err(_) => None,
            };

            let profile = match state.session_store.get_user_profile(&msg.sender.jid).await {
                Ok(p) => p,
                Err(_) => None,
            };

            let prompt = state.persona_engine.build_prompt(&msg, profile.as_ref());
            let job_id = format!("job-{}", chrono_now_secs() * 1000 + (rand::random::<u32>() % 1000) as i64);

            let now = chrono_now_secs();
            {
                let mut jobs = state.sim_jobs.write().await;
                // Clean up jobs older than 10 minutes
                jobs.retain(|_, v| match &v.status {
                    SimulationJobStatus::Processing { started_at } => (now - started_at) < 600,
                    SimulationJobStatus::Completed { finished_at, .. } => (now - finished_at) < 600,
                    SimulationJobStatus::Failed { finished_at, .. } => (now - finished_at) < 600,
                });

                jobs.insert(
                    job_id.clone(),
                    SimulationJob {
                        id: job_id.clone(),
                        status: SimulationJobStatus::Processing { started_at: now },
                    },
                );
            }

            let state_clone = Arc::clone(&state);
            let job_id_clone = job_id.clone();
            let msg_chat_jid = msg.chat_jid.clone();
            let model_override = payload.model_override.clone();

            tokio::spawn(async move {
                info!("Starting async agent execution for job {} (model override: {:?})", job_id_clone, model_override);
                let exec_res = state_clone
                    .agent_engine
                    .execute_with_model(conv_id.as_deref(), &prompt, model_override.as_deref())
                    .await;
                let fin_time = chrono_now_secs();

                let mut jobs = state_clone.sim_jobs.write().await;
                match exec_res {
                    Ok(agent_res) => {
                        let _ = state_clone.session_store.save_conversation_id(&msg_chat_jid, &agent_res.conversation_id).await;
                        let _ = state_clone.session_store.record_message(&msg_chat_jid, &state_clone.bot_jid, &agent_res.response_text, true).await;

                        jobs.insert(
                            job_id_clone.clone(),
                            SimulationJob {
                                id: job_id_clone,
                                status: SimulationJobStatus::Completed {
                                    decision: "Respond".to_string(),
                                    reason,
                                    response_text: Some(agent_res.response_text),
                                    duration_seconds: Some(agent_res.duration_seconds),
                                    conversation_id: Some(agent_res.conversation_id),
                                    finished_at: fin_time,
                                },
                            },
                        );
                    }
                    Err(e) => {
                        error!("Agent async execution error for job {}: {}", job_id_clone, e);
                        jobs.insert(
                            job_id_clone.clone(),
                            SimulationJob {
                                id: job_id_clone,
                                status: SimulationJobStatus::Failed {
                                    error: format!("Agent error: {}", e),
                                    finished_at: fin_time,
                                },
                            },
                        );
                    }
                }
            });

            // Return immediately with 202 Accepted (<5ms)
            (
                StatusCode::ACCEPTED,
                Json(json!({
                    "job_id": job_id,
                    "status": "processing"
                })),
            )
        }
    }
}

async fn simulate_job_status_handler(
    State(state): State<Arc<WebhookServerState>>,
    headers: HeaderMap,
    Path(job_id): Path<String>,
) -> impl IntoResponse {
    if !is_admin_authorized(&headers, &state.setup_code) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "error": "Akses simulator ditolak. Harap masukkan Admin Key / Setup Code yang valid."
            })),
        );
    }

    let jobs = state.sim_jobs.read().await;
    match jobs.get(&job_id) {
        Some(job) => {
            let now = chrono_now_secs();
            match &job.status {
                SimulationJobStatus::Processing { started_at } => {
                    let elapsed = (now - started_at).max(0);
                    (
                        StatusCode::OK,
                        Json(json!({
                            "status": "processing",
                            "job_id": job_id,
                            "elapsed_seconds": elapsed,
                        })),
                    )
                }
                SimulationJobStatus::Completed {
                    decision,
                    reason,
                    response_text,
                    duration_seconds,
                    conversation_id,
                    ..
                } => (
                    StatusCode::OK,
                    Json(json!({
                        "status": "completed",
                        "job_id": job_id,
                        "decision": decision,
                        "reason": reason,
                        "response_text": response_text,
                        "duration_seconds": duration_seconds,
                        "conversation_id": conversation_id,
                    })),
                ),
                SimulationJobStatus::Failed { error, .. } => (
                    StatusCode::OK,
                    Json(json!({
                        "status": "failed",
                        "job_id": job_id,
                        "error": error,
                    })),
                ),
            }
        }
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "status": "not_found",
                "error": "Pekerjaan simulasi tidak ditemukan atau sudah kedaluwarsa."
            })),
        ),
    }
}

async fn dashboard_or_setup_handler(
    State(state): State<Arc<WebhookServerState>>,
) -> impl IntoResponse {
    let is_auth = state.agent_engine.is_authenticated().await;
    let live_model = state.agent_engine.get_model().await;
    Html(render_html(is_auth, &state, &live_model))
}

async fn setup_page_handler(
    State(state): State<Arc<WebhookServerState>>,
) -> impl IntoResponse {
    let is_auth = state.agent_engine.is_authenticated().await;
    let live_model = state.agent_engine.get_model().await;
    Html(render_html(is_auth, &state, &live_model))
}

async fn webhook_handler(
    State(state): State<Arc<WebhookServerState>>,
    Query(query_params): Query<HashMap<String, String>>,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    debug!("Received webhook payload: {:?}, query: {:?}", payload, query_params);

    let mut msg_opt = parse_whatsmeow_message(
        &payload,
        Some(&query_params),
        Some(&state.bot_jid),
        state.companion_jid.as_deref(),
        state.companion_session_id.as_deref(),
    );

    if let Some(ref mut msg) = msg_opt {
        let root = if let Some(data_obj) = payload.get("data").and_then(|d| d.as_object()) {
            data_obj
        } else if let Some(root_obj) = payload.as_object() {
            root_obj
        } else {
            &serde_json::Map::new()
        };

        let media_base64 = root
            .get("media_base64")
            .or_else(|| payload.get("media_base64"))
            .and_then(|v| v.as_str());

        let mime_type = root
            .get("mime_type")
            .or_else(|| payload.get("mime_type"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let filename_opt = root
            .get("filename")
            .or_else(|| payload.get("filename"))
            .or_else(|| root.get("media_info").and_then(|m| m.get("filename")))
            .or_else(|| payload.get("media_info").and_then(|m| m.get("filename")))
            .and_then(|v| v.as_str());

        let media_type_opt = root
            .get("media_type")
            .or_else(|| payload.get("media_type"))
            .and_then(|v| v.as_str())
            .or_else(|| msg.media_type.as_deref());

        let download_url_opt = root
            .get("download_url")
            .or_else(|| root.get("media_url"))
            .or_else(|| payload.get("download_url"))
            .or_else(|| payload.get("media_url"))
            .and_then(|v| v.as_str());

        let mut media_bytes: Option<Vec<u8>> = None;
        let mut effective_mime = mime_type.to_string();

        if let Some(b64) = media_base64 {
            let clean = b64.trim();
            if !clean.is_empty() {
                use base64::Engine;
                if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(clean) {
                    media_bytes = Some(bytes);
                }
            }
        }

        if media_bytes.is_none() {
            // CLAIM-CHECK PATTERN:
            // If media_base64 is absent or omitted, retrieve media stream on-demand
            // using the download_url claim check ticket or fallback to media id.
            let (target_url, target_key) = if msg.session_role == crate::core::domain::SessionRole::UserCompanion
                && state.companion_base_url.is_some()
            {
                (
                    state.companion_base_url.as_deref().unwrap(),
                    state.companion_api_key.as_deref().unwrap_or(&state.whatsmeow_api_key),
                )
            } else {
                (state.whatsmeow_url.as_str(), state.whatsmeow_api_key.as_str())
            };

            let full_url = if let Some(url_str) = download_url_opt {
                if url_str.starts_with("http://") || url_str.starts_with("https://") {
                    url_str.to_string()
                } else {
                    format!(
                        "{}{}",
                        target_url.trim_end_matches('/'),
                        if url_str.starts_with('/') {
                            url_str.to_string()
                        } else {
                            format!("/{}", url_str)
                        }
                    )
                }
            } else if msg.has_media
                && !msg.id.is_empty()
                && msg.id != "unknown_id"
                && !target_url.is_empty()
            {
                format!(
                    "{}/api/v1/media/{}/download",
                    target_url.trim_end_matches('/'),
                    msg.id
                )
            } else {
                String::new()
            };

            if !full_url.is_empty() {
                info!("Claim-Check: Fetching media for msg {} from {}", msg.id, full_url);
                let client = reqwest::Client::builder()
                    .timeout(std::time::Duration::from_secs(60))
                    .build()
                    .unwrap_or_else(|_| reqwest::Client::new());

                let mut req = client.get(&full_url);
                if !target_key.is_empty() {
                    req = req
                        .header("Authorization", format!("Bearer {}", target_key))
                        .header("X-API-Key", target_key);
                }

                match req.send().await {
                    Ok(resp) => {
                        if resp.status().is_success() {
                            if effective_mime.is_empty() {
                                if let Some(ct) = resp
                                    .headers()
                                    .get(reqwest::header::CONTENT_TYPE)
                                    .and_then(|v| v.to_str().ok())
                                {
                                    effective_mime = ct.to_string();
                                }
                            }
                            match resp.bytes().await {
                                Ok(b) => {
                                    info!(
                                        "Successfully streamed {} bytes of media for msg {}",
                                        b.len(),
                                        msg.id
                                    );
                                    media_bytes = Some(b.to_vec());
                                }
                                Err(e) => {
                                    error!(
                                        "Failed to read media response body from {}: {:?}",
                                        full_url, e
                                    );
                                }
                            }
                        } else {
                            warn!(
                                "Failed to stream media from {}: HTTP status {}",
                                full_url,
                                resp.status()
                            );
                        }
                    }
                    Err(e) => {
                        error!("Network error fetching media from {}: {:?}", full_url, e);
                    }
                }
            }
        }

        if let Some(bytes) = media_bytes {
            let ext = resolve_media_extension(&effective_mime, filename_opt, media_type_opt);
            let media_dir = state.workspace_dir.join("media");
            if let Err(e) = tokio::fs::create_dir_all(&media_dir).await {
                error!("Failed to create media directory {:?}: {:?}", media_dir, e);
            } else {
                let file_name = format!("{}.{}", msg.id, ext);
                let target_path = media_dir.join(&file_name);
                let sidecar_txt_path = media_dir.join(format!("{}.txt", msg.id));

                if let Err(e) = tokio::fs::write(&target_path, &bytes).await {
                    error!("Failed to write media file to {:?}: {:?}", target_path, e);
                } else {
                    info!("Saved incoming media to {:?} ({} bytes)", target_path, bytes.len());
                    let abs_path_str = target_path.to_string_lossy().to_string();
                    msg.media_path = Some(abs_path_str.clone());
                    msg.has_media = true;

                    if msg.text.trim().is_empty() {
                        if let Some(fname) = filename_opt {
                            msg.text = format!("[Dokumen terlampir: {}]", fname);
                        } else if let Some(m_type) = media_type_opt {
                            msg.text = format!("[{} terlampir]", match m_type {
                                "document" => "Dokumen",
                                "video" => "Video",
                                "audio" => "Audio",
                                _ => "Foto / Media",
                            });
                        }
                    }

                    // Create initial .txt companion file with metadata & caption for searchable indexing
                    let initial_txt = format!(
                        "ID: {}\nPengirim: {} ({})\nWaktu: {}\nCaption/Pesan: {}\nPath File: {}\nNama File Asli: {}\nMIME Type: {}\nUkuran: {} bytes\n\n--- Catatan & Hasil Transkripsi / Analisis Aina ---\n",
                        msg.id,
                        msg.sender.name.as_deref().unwrap_or("Anonim"),
                        msg.sender.jid,
                        msg.timestamp,
                        msg.text,
                        abs_path_str,
                        filename_opt.unwrap_or("-"),
                        effective_mime,
                        bytes.len()
                    );
                    let _ = tokio::fs::write(&sidecar_txt_path, initial_txt).await;
                }
            }
        }

        dispatch_message_to_queue(&state, msg.clone()).await;
    } else {
        debug!("Webhook received event that was not a parseable user message");
    }

    (StatusCode::OK, "OK")
}

async fn dispatch_message_to_queue(state: &Arc<WebhookServerState>, msg: IncomingMessage) {
    let usecase = Arc::clone(&state.usecase);
    dispatch_incoming_message_to_queue(&state.chat_queues, msg, 300, move |m| {
        let uc = Arc::clone(&usecase);
        async move { uc.execute(m).await }
    })
    .await;
}

pub async fn dispatch_incoming_message_to_queue<F, Fut>(
    chat_queues: &Arc<tokio::sync::Mutex<HashMap<String, tokio::sync::mpsc::UnboundedSender<IncomingMessage>>>>,
    msg: IncomingMessage,
    idle_timeout_secs: u64,
    handler: F,
) where
    F: Fn(IncomingMessage) -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = anyhow::Result<()>> + Send + 'static,
{
    let chat_jid = msg.chat_jid.clone();
    let mut queues = chat_queues.lock().await;

    let mut needs_new_worker = false;
    if let Some(tx) = queues.get(&chat_jid) {
        if tx.is_closed() {
            needs_new_worker = true;
        }
    } else {
        needs_new_worker = true;
    }

    if needs_new_worker {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<IncomingMessage>();
        let queues_ref = Arc::clone(chat_queues);
        let worker_chat_jid = chat_jid.clone();
        let handler = Arc::new(handler);

        tokio::spawn(async move {
            info!("Started sequential FIFO message queue worker for chat {}", worker_chat_jid);
            loop {
                match tokio::time::timeout(std::time::Duration::from_secs(idle_timeout_secs), rx.recv()).await {
                    Ok(Some(queued_msg)) => {
                        if let Err(e) = handler(queued_msg).await {
                            error!("Error processing queued message for {}: {:?}", worker_chat_jid, e);
                        }
                    }
                    Ok(None) => break,
                    Err(_) => {
                        // Idle timeout reached without incoming messages
                        let mut lock = queues_ref.lock().await;
                        if rx.is_empty() {
                            lock.remove(&worker_chat_jid);
                            info!("Sequential queue worker idle timeout for chat {}, cleaned up", worker_chat_jid);
                            break;
                        }
                    }
                }
            }
        });

        queues.insert(chat_jid.clone(), tx);
    }

    if let Some(tx) = queues.get(&chat_jid) {
        if let Err(e) = tx.send(msg) {
            error!("Failed to enqueue message into queue for {}: {:?}", chat_jid, e);
        }
    }
}

pub fn resolve_media_extension(
    mime_type: &str,
    filename: Option<&str>,
    media_type: Option<&str>,
) -> String {
    if let Some(fname) = filename {
        if let Some(ext) = std::path::Path::new(fname).extension().and_then(|e| e.to_str()) {
            let clean_ext = ext.trim().to_lowercase();
            if !clean_ext.is_empty() && clean_ext.len() <= 6 && clean_ext.chars().all(|c| c.is_alphanumeric()) {
                return clean_ext;
            }
        }
    }

    let mime_clean = mime_type.split(';').next().unwrap_or(mime_type).trim().to_lowercase();
    match mime_clean.as_str() {
        "image/jpeg" | "image/jpg" => "jpg".to_string(),
        "image/png" => "png".to_string(),
        "image/webp" => "webp".to_string(),
        "image/gif" => "gif".to_string(),
        "video/mp4" => "mp4".to_string(),
        "video/quicktime" => "mov".to_string(),
        "video/x-matroska" => "mkv".to_string(),
        "audio/ogg" => "ogg".to_string(),
        "audio/mp4" | "audio/m4a" => "m4a".to_string(),
        "audio/mpeg" | "audio/mp3" => "mp3".to_string(),
        "audio/wav" | "audio/x-wav" => "wav".to_string(),
        "application/pdf" => "pdf".to_string(),
        "application/zip" => "zip".to_string(),
        "application/x-tar" => "tar".to_string(),
        "application/gzip" => "gz".to_string(),
        "text/plain" => "txt".to_string(),
        "text/csv" => "csv".to_string(),
        "application/json" => "json".to_string(),
        "application/msword" => "doc".to_string(),
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => "docx".to_string(),
        "application/vnd.ms-excel" => "xls".to_string(),
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" => "xlsx".to_string(),
        "application/vnd.ms-powerpoint" => "ppt".to_string(),
        "application/vnd.openxmlformats-officedocument.presentationml.presentation" => "pptx".to_string(),
        _ => match media_type.unwrap_or("") {
            "image" => "jpg".to_string(),
            "video" => "mp4".to_string(),
            "audio" => "ogg".to_string(),
            "document" => "pdf".to_string(),
            _ => "bin".to_string(),
        },
    }
}

fn parse_whatsmeow_message(
    val: &Value,
    query_params: Option<&HashMap<String, String>>,
    bot_jid: Option<&str>,
    companion_jid: Option<&str>,
    companion_session_id: Option<&str>,
) -> Option<IncomingMessage> {
    let root = if let Some(data_obj) = val.get("data").and_then(|d| d.as_object()) {
        data_obj
    } else if let Some(root_obj) = val.as_object() {
        root_obj
    } else {
        return None;
    };

    let chat_jid = root
        .get("from")
        .or_else(|| root.get("chat_jid"))
        .or_else(|| root.get("remote_jid"))
        .or_else(|| root.get("chat"))
        .and_then(|v| v.as_str())?
        .to_string();

    let sender_jid = root
        .get("sender")
        .or_else(|| root.get("participant"))
        .and_then(|v| v.as_str())
        .unwrap_or(&chat_jid)
        .to_string();

    let sender_name = root
        .get("push_name")
        .or_else(|| root.get("sender_name"))
        .or_else(|| root.get("name"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let text = root
        .get("body")
        .or_else(|| root.get("message"))
        .or_else(|| root.get("text"))
        .or_else(|| root.get("conversation"))
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();

    let msg_id = root
        .get("id")
        .or_else(|| root.get("message_id"))
        .and_then(|v| v.as_str())
        .unwrap_or("unknown_id")
        .to_string();

    let is_from_me = root
        .get("is_from_me")
        .or_else(|| root.get("from_me"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let timestamp = root
        .get("timestamp")
        .and_then(|v| v.as_i64())
        .unwrap_or_else(chrono_now_secs);

    let chat_type = if chat_jid.ends_with("@g.us")
        || root
            .get("is_group")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    {
        ChatType::Group
    } else {
        ChatType::DirectMessage
    };

    let quoted_message = root
        .get("quoted_message")
        .or_else(|| root.get("context_info"))
        .and_then(|q| {
            let q_id = q.get("id").and_then(|v| v.as_str()).unwrap_or_default();
            let q_sender = q
                .get("participant")
                .or_else(|| q.get("sender"))
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            let q_text = q
                .get("body")
                .or_else(|| q.get("text"))
                .or_else(|| q.get("conversation"))
                .and_then(|v| v.as_str())
                .unwrap_or_default();

            if !q_text.is_empty() {
                Some(QuotedMessage {
                    id: q_id.to_string(),
                    sender_jid: q_sender.to_string(),
                    text: q_text.to_string(),
                })
            } else {
                None
            }
        });

    let mentioned_jids = root
        .get("mentioned_jids")
        .or_else(|| root.get("mentions"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|item| item.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    let is_bot_mentioned = root
        .get("is_bot_mentioned")
        .or_else(|| val.get("is_bot_mentioned"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let bot_lid = root
        .get("bot_lid")
        .or_else(|| val.get("bot_lid"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let has_media = root
        .get("has_media")
        .or_else(|| val.get("has_media"))
        .and_then(|v| v.as_bool())
        .or_else(|| {
            if root.get("download_url").is_some()
                || root.get("media_url").is_some()
                || val.get("download_url").is_some()
                || val.get("media_url").is_some()
                || root.get("media_base64").is_some()
                || val.get("media_base64").is_some()
            {
                Some(true)
            } else {
                None
            }
        })
        .unwrap_or(false);

    let media_type = root
        .get("media_type")
        .or_else(|| val.get("media_type"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let is_bot_unassigned = bot_jid
        .map(|b| {
            let clean = b.trim().to_lowercase();
            clean.is_empty()
                || clean.starts_with("unassigned")
                || clean.starts_with("placeholder")
                || clean.starts_with("dummy")
                || clean.starts_with("6280000000000")
                || clean.starts_with("628123456789")
        })
        .unwrap_or(true);

    let explicit_role = query_params
        .and_then(|q| q.get("role").or_else(|| q.get("session_role")))
        .map(|s| s.as_str())
        .or_else(|| {
            root.get("session_role")
                .or_else(|| val.get("session_role"))
                .and_then(|v| v.as_str())
        });

    let session_id = query_params
        .and_then(|q| q.get("session_id").or_else(|| q.get("session")))
        .map(|s| s.as_str())
        .or_else(|| {
            root.get("session_id")
                .or_else(|| root.get("session"))
                .or_else(|| val.get("session_id"))
                .or_else(|| val.get("session"))
                .and_then(|v| v.as_str())
        });

    let to_jid = root
        .get("to")
        .or_else(|| root.get("receiver"))
        .or_else(|| root.get("recipient"))
        .and_then(|v| v.as_str());

    let session_role = if let Some(role_str) = explicit_role {
        if role_str.eq_ignore_ascii_case("user_companion") || role_str.eq_ignore_ascii_case("companion") {
            crate::core::domain::SessionRole::UserCompanion
        } else {
            crate::core::domain::SessionRole::PrimaryBot
        }
    } else if let (Some(sid), Some(comp_sid)) = (session_id, companion_session_id) {
        if sid.trim() == comp_sid.trim() {
            crate::core::domain::SessionRole::UserCompanion
        } else {
            crate::core::domain::SessionRole::PrimaryBot
        }
    } else if let (Some(to), Some(comp_jid)) = (to_jid, companion_jid) {
        let to_clean = to.split('@').next().unwrap_or(to);
        let comp_clean = comp_jid.split('@').next().unwrap_or(comp_jid);
        if to_clean == comp_clean {
            crate::core::domain::SessionRole::UserCompanion
        } else {
            crate::core::domain::SessionRole::PrimaryBot
        }
    } else if let Some(comp_jid) = companion_jid {
        let sender_clean = sender_jid.split('@').next().unwrap_or(&sender_jid);
        let comp_clean = comp_jid.split('@').next().unwrap_or(comp_jid);
        if is_from_me && sender_clean == comp_clean {
            crate::core::domain::SessionRole::UserCompanion
        } else if is_bot_unassigned {
            // If primary dedicated bot is not configured/unassigned, all traffic from this gateway belongs to the companion sensor
            crate::core::domain::SessionRole::UserCompanion
        } else {
            crate::core::domain::SessionRole::PrimaryBot
        }
    } else {
        crate::core::domain::SessionRole::PrimaryBot
    };

    Some(IncomingMessage {
        id: msg_id,
        platform: crate::core::domain::Platform::WhatsApp,
        session_role,
        chat_jid,
        chat_type,
        sender: Sender {
            jid: sender_jid,
            name: sender_name,
        },
        text,
        timestamp,
        is_from_me,
        quoted_message,
        mentioned_jids,
        is_bot_mentioned,
        bot_lid,
        has_media,
        media_type,
        media_path: None,
    })
}

fn chrono_now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn render_html(is_authenticated: bool, state: &WebhookServerState, current_model: &str) -> String {
    let (badge_class, badge_text, badge_bg) = if is_authenticated {
        ("badge-success", "ONLINE & TERAUTENTIKASI", "#10b981")
    } else {
        ("badge-warning", "PERLU SETUP AUTENTIKASI", "#f59e0b")
    };

    let companion_info = if let Some(c_jid) = &state.companion_jid {
        let c_name = state.companion_name.as_deref().unwrap_or("Personal Account");
        format!("{} ({}) <span style=\"color: #10b981; font-weight: 600;\">[Aktif]</span>", c_name, c_jid)
    } else {
        "<span style=\"color: #94a3b8;\">Belum Dikonfigurasi (Opsional)</span>".to_string()
    };

    let status_card = if is_authenticated {
        format!(
            r#"
            <div class="card status-card">
                <div class="card-header">
                    <span class="pulse" style="background: {badge_bg}"></span>
                    <h2>Aina Siap Digunakan!</h2>
                </div>
                <p>Mesin agentik Google Antigravity telah terhubung dan aktif melayani pesan WhatsApp.</p>
                <div class="info-grid">
                    <div class="info-item"><span class="label">Nama Bot (Primary)</span><span class="val">{name}</span></div>
                    <div class="info-item"><span class="label">WhatsApp JID Bot</span><span class="val">{jid}</span></div>
                    <div class="info-item"><span class="label">Companion (Shadow Sensor)</span><span class="val">{companion_info}</span></div>
                    <div class="info-item">
                        <span class="label">Model AI Aktif</span>
                        <div style="display: flex; justify-content: space-between; align-items: baseline;">
                            <span class="val" id="active-model-display">{model}</span>
                            <a href="javascript:void(0)" onclick="openModelModal()" style="font-size: 0.75rem; color: var(--primary); text-decoration: none; font-weight: 600;">⚡ Ganti</a>
                        </div>
                    </div>
                    <div class="info-item"><span class="label">Whatsmeow</span><span class="val">{url}</span></div>
                    <div class="info-item"><span class="label">Zona Waktu / Locale</span><span class="val">{timezone} ({locale})</span></div>
                </div>
                <div class="helper-box">
                    <strong>Webhook Endpoint:</strong>
                    <code>POST /webhook</code>
                    <p style="margin-top: 6px; font-size: 0.85rem; color: #94a3b8;">Arahkan webhook dari instance Whatsmeow ke URL ini.</p>
                </div>
            </div>

            <!-- ADMIN LOCK CARD -->
            <div id="sim-lock-card" class="card" style="border-color: #f59e0b; display: none;">
                <div class="card-header">
                    <span style="font-size: 1.2rem;">🔒</span>
                    <h2>Akses Simulator Terproteksi</h2>
                </div>
                <p style="color: var(--text-muted); font-size: 0.9rem; margin-bottom: 14px;">
                    Untuk mencegah orang luar mengeksekusi agen AI di server Anda, fitur simulator dilindungi. Masukkan <strong>Setup Code / Admin Key</strong> server Anda untuk membuka sesi.
                </p>
                <div style="display: flex; gap: 8px;">
                    <input id="admin-passcode-input" class="form-input" type="password" placeholder="Masukkan Admin Key / Setup Code..." style="margin-bottom: 0;" />
                    <button id="unlock-btn" class="btn" style="white-space: nowrap;" onclick="unlockAdminSession()">Buka Kunci</button>
                </div>
                <div id="unlock-error" style="display: none; color: #ef4444; font-size: 0.85rem; margin-top: 8px;"></div>
            </div>

            <!-- SIMULATOR CHAT REAL END-TO-END -->
            <div id="simulator-card" class="card simulator-card">
                <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 14px; padding: 6px 12px; background: rgba(16,185,129,0.1); border: 1px solid rgba(16,185,129,0.2); border-radius: 6px; font-size: 0.8rem; color: #10b981;">
                    <span>🛡️ Sesi Admin Terverifikasi</span>
                    <a href="javascript:void(0)" onclick="lockAdminSession()" style="color: #f87171; text-decoration: none; font-weight: 600;">Kunci Dashboard</a>
                </div>
                <div class="card-header">
                    <span style="font-size: 1.2rem;">🧪</span>
                    <h2>Simulator Percakapan WhatsApp (Real Test)</h2>
                </div>
                <p style="color: var(--text-muted); font-size: 0.9rem; margin-bottom: 16px;">
                    Uji langsung logika respons, dual-session routing (Bot Utama vs Companion Sensor), etika grup (Gatekeeper), dan eksekusi agentik Antigravity secara nyata tanpa harus mengirim chat dari HP Anda.
                </p>

                <div style="display: grid; grid-template-columns: 1fr 1fr; gap: 12px; margin-bottom: 12px;">
                    <div>
                        <label class="form-label">Sesi WhatsApp (Pipeline Routing)</label>
                        <select id="sim-session-role" class="form-input" onchange="onSessionRoleChange(this.value)">
                            <option value="primary_bot">🤖 Sesi Bot Utama (Primary Dedicated)</option>
                            <option value="user_companion">👥 Sesi Companion (Akun Pribadi / Shadow Sensor)</option>
                        </select>
                    </div>
                    <div>
                        <label class="form-label">Tipe Obrolan</label>
                        <select id="sim-chat-type" class="form-input" onchange="onChatTypeChange(this.value)">
                            <option value="dm">Pesan Pribadi (DM)</option>
                            <option value="group">Grup WhatsApp Kantor</option>
                        </select>
                    </div>
                </div>

                <div style="display: grid; grid-template-columns: 1fr 1fr; gap: 12px; margin-bottom: 12px;">
                    <div>
                        <label class="form-label">Nama Pengirim</label>
                        <input id="sim-sender-name" class="form-input" value="Owner" />
                    </div>
                    <div style="display: flex; align-items: center; padding-top: 20px;">
                        <label style="font-size: 0.85rem; color: #94a3b8; display: flex; align-items: center; gap: 8px; cursor: pointer;">
                            <input type="checkbox" id="sim-is-from-me" onchange="onIsFromMeChange(this.checked)" />
                            <span>Kirim Sebagai Diri Sendiri (is_from_me / Owner)</span>
                        </label>
                    </div>
                </div>

                <div style="margin-bottom: 12px;">
                    <label class="form-label">Pilihan Model AI (Sesi Simulasi Ini)</label>
                    <select id="sim-model-select" class="form-input">
                        <option value="default">Mengikuti Model Aktif Default ({model})</option>
                        <option value="gemini-3.8-flash-medium">Gemini 3.8 Flash (Medium) - Default Cepat &amp; Seimbang (5-15s)</option>
                        <option value="gemini-3.8-flash-high">Gemini 3.8 Flash (High) - Penalaran Tinggi / Deep Thinking</option>
                        <option value="gemini-3.8-flash-low">Gemini 3.8 Flash (Low) - Respons Kilat &amp; Kasual (&lt;3s)</option>
                        <option value="gemini-3.1-pro-high">Gemini 3.1 Pro (High) - Deep Coding &amp; Arsitektur Sistem</option>
                        <option value="claude-opus-4-6-thinking">Claude Opus 4.6 (Thinking) - Khusus Tugas Sangat Kompleks (Eksplisit)</option>
                        <option value="claude-sonnet-4-6">Claude Sonnet 4.6 (Thinking)</option>
                    </select>
                </div>

                <div id="mention-toggle-wrapper" style="display: none; margin-bottom: 12px;">
                    <label style="font-size: 0.85rem; color: #94a3b8; display: flex; align-items: center; gap: 8px; cursor: pointer;">
                        <input type="checkbox" id="sim-is-mention" />
                        <span>Simulasikan Tag / Mention (@Aina) dalam grup</span>
                    </label>
                </div>

                <div style="display: flex; justify-content: space-between; align-items: center; margin-top: 14px; margin-bottom: 8px;">
                    <label class="form-label" style="margin-bottom: 0; font-weight: 600;">💬 Alur Percakapan Interaktif (Multi-Turn)</label>
                    <button type="button" class="btn-outline" style="font-size: 0.78rem; padding: 4px 10px; border-radius: 6px; cursor: pointer;" onclick="resetSimulationChat()">
                        🔄 Reset Percakapan Baru
                    </button>
                </div>

                <!-- SCROLLABLE CHAT THREAD -->
                <div id="sim-chat-thread" class="chat-thread-container">
                    <div id="sim-thread-empty" class="chat-bubble-system">
                        Belum ada pesan. Mulai obrolan dengan Aina di bawah. Anda bisa membalas chat secara berkelanjutan layaknya di WhatsApp!
                    </div>
                </div>

                <label class="form-label">Ketik Pesan Chat (Tekan Enter untuk kirim, Shift+Enter untuk baris baru):</label>
                <div style="display: flex; gap: 8px; align-items: flex-start;">
                    <textarea id="sim-text" class="form-input" style="height: 60px; margin-bottom: 0; resize: none;" placeholder="Ketik pesan atau balasan Anda ke Aina (contoh: '!aina rangkum diskusi tadi' atau 'Sudah ku-authorize ya')..."></textarea>
                    <button id="sim-btn" class="btn" style="min-width: 130px; height: 60px; display: flex; align-items: center; justify-content: center; gap: 6px; font-weight: 600;" onclick="runSimulation()">
                        <span>Kirim</span> 🚀
                    </button>
                </div>
            </div>
            "#,
            badge_bg = badge_bg,
            name = state.bot_name,
            jid = state.bot_jid,
            companion_info = companion_info,
            model = current_model,
            url = state.whatsmeow_url,
            timezone = state.timezone,
            locale = state.locale,
        )
    } else {
        r#"
        <div class="card warning-card">
            <div class="card-header">
                <span class="pulse" style="background: #f59e0b"></span>
                <h2>Setup Autentikasi Google Antigravity</h2>
            </div>
            <p>Aina memerlukan OAuth Token untuk mengakses Google Antigravity CLI di server ini.</p>
        </div>
        "#.to_string()
    };

    let auth_form_display = if is_authenticated { "display: none;" } else { "display: block;" };

    format!(
        r#"<!DOCTYPE html>
<html lang="id">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Aina (あいな) - Assistant Dashboard</title>
    
    <!-- Google Fonts: Google Sans & Google Sans Text (Gemini standard), JetBrains Mono -->
    <link rel="preconnect" href="https://fonts.googleapis.com">
    <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
    <link href="https://fonts.googleapis.com/css2?family=Google+Sans:wght@400;500;700&family=Google+Sans+Text:ital,wght@0,400;0,500;0,700;1,400&family=JetBrains+Mono:ital,wght@0,400;0,500;0,600;1,400&display=swap" rel="stylesheet">
    
    <!-- Markdown Parser & Highlight.js CDN -->
    <link rel="stylesheet" href="https://cdn.jsdelivr.net/gh/highlightjs/cdn-release@11.9.0/build/styles/github-dark-dimmed.min.css">
    <script src="https://cdn.jsdelivr.net/npm/marked/marked.min.js"></script>
    <script src="https://cdn.jsdelivr.net/gh/highlightjs/cdn-release@11.9.0/build/highlight.min.js"></script>
    <script src="https://cdn.jsdelivr.net/npm/dompurify@3.0.9/dist/purify.min.js"></script>

    <style>
        :root {{
            --bg: #0b0f19;
            --surface: #151d2e;
            --border: #2c3a52;
            --border-subtle: #1e293b;
            --text: #f1f5f9;
            --text-muted: #94a3b8;
            --primary: #38bdf8;
            --primary-hover: #0284c7;
            --success: #10b981;
            --warning: #f59e0b;
            --danger: #ef4444;
            --font-sans: 'Google Sans', 'Google Sans Text', -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
            --font-mono: 'JetBrains Mono', 'Fira Code', monospace;
        }}
        * {{ box-sizing: border-box; margin: 0; padding: 0; }}
        body {{
            background: var(--bg);
            color: var(--text);
            font-family: var(--font-sans);
            -webkit-font-smoothing: antialiased;
            -moz-osx-font-smoothing: grayscale;
            line-height: 1.65;
            padding: 40px 20px;
            display: flex;
            justify-content: center;
        }}
        .container {{
            width: 100%;
            max-width: 760px;
        }}
        .header {{
            text-align: center;
            margin-bottom: 30px;
        }}
        .header h1 {{
            font-size: 2.1rem;
            font-weight: 700;
            display: flex;
            align-items: center;
            justify-content: center;
            gap: 10px;
            letter-spacing: -0.02em;
        }}
        .badge {{
            display: inline-block;
            padding: 4px 14px;
            border-radius: 9999px;
            font-size: 0.75rem;
            font-weight: 700;
            letter-spacing: 0.5px;
            margin-top: 10px;
            background: rgba(255,255,255,0.08);
            border: 1px solid var(--border);
        }}
        .badge-success {{ color: var(--success); border-color: rgba(16,185,129,0.3); background: rgba(16,185,129,0.08); }}
        .badge-warning {{ color: var(--warning); border-color: rgba(245,158,11,0.3); background: rgba(245,158,11,0.08); }}
        .card {{
            background: var(--surface);
            border: 1px solid var(--border);
            border-radius: 14px;
            padding: 24px;
            margin-bottom: 24px;
            box-shadow: 0 10px 25px -5px rgba(0,0,0,0.4);
        }}
        .card-header {{
            display: flex;
            align-items: center;
            gap: 12px;
            margin-bottom: 12px;
        }}
        .card-header h2 {{
            font-size: 1.25rem;
            font-weight: 600;
            letter-spacing: -0.01em;
        }}
        .pulse {{
            width: 12px;
            height: 12px;
            border-radius: 50%;
            display: inline-block;
            animation: pulse-animation 2s infinite;
        }}
        @keyframes pulse-animation {{
            0% {{ transform: scale(0.95); box-shadow: 0 0 0 0 rgba(56, 189, 248, 0.7); }}
            70% {{ transform: scale(1); box-shadow: 0 0 0 8px rgba(56, 189, 248, 0); }}
            100% {{ transform: scale(0.95); box-shadow: 0 0 0 0 rgba(56, 189, 248, 0); }}
        }}
        .info-grid {{
            display: grid;
            grid-template-columns: 1fr 1fr;
            gap: 12px;
            margin: 18px 0;
        }}
        .info-item {{
            background: rgba(0,0,0,0.25);
            padding: 10px 14px;
            border-radius: 8px;
            border: 1px solid rgba(255,255,255,0.05);
        }}
        .info-item .label {{
            font-size: 0.75rem;
            color: var(--text-muted);
            display: block;
            margin-bottom: 2px;
        }}
        .info-item .val {{
            font-size: 0.9rem;
            font-weight: 600;
            word-break: break-all;
        }}
        .helper-box {{
            background: #090d16;
            border: 1px solid #1e293b;
            padding: 14px;
            border-radius: 8px;
            margin-top: 14px;
        }}
        code {{
            background: rgba(255,255,255,0.08);
            color: var(--primary);
            padding: 2px 6px;
            border-radius: 4px;
            font-family: var(--font-mono);
            font-size: 0.85rem;
        }}
        .form-label {{
            font-size: 0.85rem;
            color: var(--text-muted);
            display: block;
            margin-bottom: 6px;
            font-weight: 500;
        }}
        .form-input {{
            width: 100%;
            background: #090d16;
            border: 1px solid var(--border);
            border-radius: 8px;
            color: #fff;
            padding: 10px 14px;
            font-family: inherit;
            font-size: 0.9rem;
            margin-bottom: 12px;
            transition: border-color 0.2s;
        }}
        .form-input:focus {{
            outline: none;
            border-color: var(--primary);
        }}
        textarea.form-input {{
            font-family: var(--font-mono);
            font-size: 0.82rem;
            resize: vertical;
        }}
        .btn {{
            background: var(--primary);
            color: #0b0f19;
            border: none;
            padding: 10px 20px;
            border-radius: 8px;
            font-weight: 600;
            font-family: inherit;
            cursor: pointer;
            transition: all 0.2s;
        }}
        .btn:hover {{ background: var(--primary-hover); color: #fff; }}
        .btn-outline {{
            background: transparent;
            border: 1px solid var(--border);
            color: var(--text-muted);
        }}
        .btn-outline:hover {{ background: rgba(255,255,255,0.05); color: var(--text); }}
        .alert {{
            padding: 12px;
            border-radius: 8px;
            margin-top: 14px;
            display: none;
            font-size: 0.85rem;
        }}
        .alert-success {{ background: rgba(16,185,129,0.15); border: 1px solid var(--success); color: var(--success); }}
        .alert-error {{ background: rgba(239,68,68,0.15); border: 1px solid var(--danger); color: var(--danger); }}
        
        /* Modern Chat Bubble & Multi-Turn Thread Styling */
        .chat-thread-container {{
            min-height: 140px;
            max-height: 480px;
            overflow-y: auto;
            background: #080c14;
            border: 1px solid var(--border);
            border-radius: 10px;
            padding: 16px;
            margin-bottom: 14px;
            display: flex;
            flex-direction: column;
            gap: 14px;
            scroll-behavior: smooth;
        }}
        .chat-row-user {{
            display: flex;
            justify-content: flex-end;
            width: 100%;
        }}
        .chat-bubble-user {{
            background: #1e3a8a;
            border: 1px solid #2563eb;
            color: #ffffff;
            padding: 10px 16px;
            border-radius: 14px 14px 2px 14px;
            max-width: 82%;
            font-size: 0.92rem;
            line-height: 1.5;
            word-break: break-word;
            box-shadow: 0 2px 8px rgba(0,0,0,0.25);
        }}
        .chat-row-bot {{
            display: flex;
            justify-content: flex-start;
            width: 100%;
        }}
        .chat-bubble-bot {{
            background: #0f2427;
            border: 1px solid #14532d;
            padding: 14px 18px;
            border-radius: 14px 14px 14px 2px;
            max-width: 92%;
            width: 100%;
            box-shadow: 0 4px 16px rgba(0,0,0,0.3);
        }}
        .chat-bubble-typing {{
            background: rgba(255,255,255,0.04);
            border: 1px dashed var(--primary);
            padding: 10px 16px;
            border-radius: 12px;
            color: var(--primary);
            font-size: 0.85rem;
            display: inline-flex;
            align-items: center;
            gap: 8px;
        }}
        .chat-bubble-system {{
            align-self: center;
            background: rgba(255,255,255,0.05);
            border: 1px solid rgba(255,255,255,0.1);
            color: var(--text-muted);
            padding: 6px 14px;
            border-radius: 999px;
            font-size: 0.78rem;
        }}
        .chat-bubble {{
            background: #0f2427;
            border: 1px solid #14532d;
            padding: 18px 20px;
            border-radius: 12px;
            border-bottom-left-radius: 2px;
            position: relative;
            box-shadow: 0 4px 16px rgba(0,0,0,0.3);
        }}
        .markdown-body {{
            font-size: 0.94rem;
            line-height: 1.68;
            color: #f1f5f9;
        }}
        .markdown-body p {{ margin-bottom: 12px; }}
        .markdown-body p:last-child {{ margin-bottom: 0; }}
        .markdown-body h1, .markdown-body h2, .markdown-body h3, .markdown-body h4 {{
            font-weight: 700;
            color: #ffffff;
            margin-top: 18px;
            margin-bottom: 8px;
            letter-spacing: -0.01em;
        }}
        .markdown-body h1 {{ font-size: 1.35rem; border-bottom: 1px solid var(--border); padding-bottom: 6px; }}
        .markdown-body h2 {{ font-size: 1.18rem; border-bottom: 1px solid rgba(255,255,255,0.08); padding-bottom: 4px; }}
        .markdown-body h3 {{ font-size: 1.05rem; }}
        .markdown-body ul, .markdown-body ol {{
            padding-left: 22px;
            margin-bottom: 12px;
        }}
        .markdown-body li {{ margin-bottom: 4px; }}
        .markdown-body hr {{
            border: 0;
            border-top: 1px solid var(--border);
            margin: 16px 0;
        }}
        
        /* Inline Code */
        .markdown-body :not(pre) > code {{
            background: rgba(255,255,255,0.08);
            color: #7dd3fc;
            padding: 2px 6px;
            border-radius: 4px;
            font-family: var(--font-mono);
            font-size: 0.85em;
            border: 1px solid rgba(255,255,255,0.06);
        }}

        /* Code Block Action Bar & Highlighting */
        .code-block-wrapper {{
            margin: 14px 0;
            border-radius: 8px;
            overflow: hidden;
            border: 1px solid #334155;
            background: #0d1117;
        }}
        .code-block-header {{
            display: flex;
            justify-content: space-between;
            align-items: center;
            background: #161b22;
            padding: 6px 14px;
            border-bottom: 1px solid #30363d;
            font-size: 0.78rem;
            color: #8b949e;
            font-family: var(--font-mono);
            font-weight: 500;
        }}
        .copy-btn {{
            background: transparent;
            border: 1px solid #30363d;
            color: #c9d1d9;
            padding: 3px 10px;
            border-radius: 4px;
            cursor: pointer;
            font-size: 0.72rem;
            font-family: var(--font-sans);
            transition: all 0.2s;
        }}
        .copy-btn:hover {{
            background: #21262d;
            color: #58a6ff;
            border-color: #58a6ff;
        }}
        .markdown-body pre {{
            margin: 0;
            padding: 14px 16px;
            overflow-x: auto;
            background: transparent !important;
        }}
        .markdown-body pre code {{
            font-family: var(--font-mono);
            font-size: 0.86rem;
            line-height: 1.55;
            background: transparent !important;
            padding: 0 !important;
        }}

        /* Tables */
        .markdown-body table {{
            width: 100%;
            border-collapse: collapse;
            margin: 14px 0;
            font-size: 0.88rem;
            border-radius: 6px;
            overflow: hidden;
        }}
        .markdown-body th, .markdown-body td {{
            border: 1px solid #334155;
            padding: 8px 12px;
            text-align: left;
        }}
        .markdown-body th {{
            background: rgba(255,255,255,0.06);
            font-weight: 600;
            color: #ffffff;
        }}
        .markdown-body tr:nth-child(even) {{
            background: rgba(255,255,255,0.02);
        }}

        /* GitHub-style Alerts / Callouts */
        .markdown-alert {{
            padding: 12px 16px;
            margin: 14px 0;
            border-left: 4px solid;
            border-radius: 0 8px 8px 0;
            background: rgba(255, 255, 255, 0.03);
            font-size: 0.9rem;
        }}
        .markdown-alert-title {{
            font-weight: 700;
            margin-bottom: 4px;
            display: flex;
            align-items: center;
            gap: 6px;
            font-size: 0.8rem;
            text-transform: uppercase;
            letter-spacing: 0.5px;
        }}
        .markdown-alert-note {{ border-color: #38bdf8; }}
        .markdown-alert-note .markdown-alert-title {{ color: #38bdf8; }}
        .markdown-alert-tip {{ border-color: #10b981; }}
        .markdown-alert-tip .markdown-alert-title {{ color: #10b981; }}
        .markdown-alert-important {{ border-color: #a855f7; }}
        .markdown-alert-important .markdown-alert-title {{ color: #a855f7; }}
        .markdown-alert-warning {{ border-color: #f59e0b; }}
        .markdown-alert-warning .markdown-alert-title {{ color: #f59e0b; }}
        .markdown-alert-caution {{ border-color: #ef4444; }}
        .markdown-alert-caution .markdown-alert-title {{ color: #ef4444; }}

        blockquote {{
            border-left: 3px solid #38bdf8;
            padding-left: 12px;
            margin: 12px 0;
            color: #94a3b8;
            font-style: italic;
        }}
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <h1>🌸 Aina (あいな)</h1>
            <span class="badge {badge_class}">{badge_text}</span>
        </div>

        {status_card}

        <div id="auth-form-container" class="card" style="{auth_form_display}">
            <h3 style="margin-bottom: 10px;">🔐 Setup Autentikasi Pertama Kali</h3>
            <p style="color: var(--text-muted); font-size: 0.88rem; margin-bottom: 12px;">
                Untuk mencegah akses tidak sah pada server publik, Anda memerlukan <strong>Kode Setup</strong> yang tercetak pada log deployment server.
            </p>

            <div class="helper-box" style="margin-bottom: 14px;">
                <p style="font-size: 0.82rem; color: #94a3b8; margin-bottom: 4px;">1. Ambil token dari terminal laptop lokal Anda:</p>
                <code>cat ~/.gemini/antigravity-cli/antigravity-oauth-token</code>
            </div>

            <label class="form-label">Kode Setup / Admin Key (Lihat di log terminal/Coolify):</label>
            <input id="setup-code-input" class="form-input" placeholder="AINA-XXXXXX atau ADMIN_KEY Anda" />

            <label class="form-label">Tempelkan seluruh JSON token di bawah:</label>
            <textarea id="token-input" class="form-input" style="height: 120px;" placeholder='{{"auth_method":"oauth","id_token":"...","token":{{...}}}}'></textarea>
            
            <button id="submit-btn" class="btn" onclick="submitToken()">Verifikasi & Simpan Token</button>
            <div id="alert-box" class="alert"></div>
        </div>
    </div>

    <script>
        let lastSimulationResponseText = '';

        function checkAdminAuth() {{
            const key = localStorage.getItem('aina_admin_key');
            const simCard = document.getElementById('simulator-card');
            const lockCard = document.getElementById('sim-lock-card');
            if (!simCard || !lockCard) return;

            if (key) {{
                simCard.style.display = 'block';
                lockCard.style.display = 'none';
            }} else {{
                simCard.style.display = 'none';
                lockCard.style.display = 'block';
            }}
        }}

        async function unlockAdminSession() {{
            const input = document.getElementById('admin-passcode-input');
            const errEl = document.getElementById('unlock-error');
            const btn = document.getElementById('unlock-btn');
            const val = input.value.trim();
            if (!val) {{
                errEl.innerText = 'Harap masukkan Setup Code / Admin Key.';
                errEl.style.display = 'block';
                return;
            }}
            errEl.style.display = 'none';
            btn.disabled = true;
            btn.innerText = 'Memverifikasi...';

            try {{
                const res = await fetch('/api/auth/verify', {{
                    method: 'POST',
                    headers: {{ 'Content-Type': 'application/json' }},
                    body: JSON.stringify({{ admin_key: val }})
                }});
                const data = await res.json();
                if (res.ok && data.success) {{
                    localStorage.setItem('aina_admin_key', val);
                    checkAdminAuth();
                }} else {{
                    errEl.innerText = data.message || 'Admin Key tidak valid!';
                    errEl.style.display = 'block';
                }}
            }} catch(e) {{
                errEl.innerText = 'Koneksi error: ' + e.message;
                errEl.style.display = 'block';
            }} finally {{
                btn.disabled = false;
                btn.innerText = 'Buka Kunci';
            }}
        }}

        function lockAdminSession() {{
            localStorage.removeItem('aina_admin_key');
            checkAdminAuth();
        }}

        async function openModelModal() {{
            const key = localStorage.getItem('aina_admin_key');
            if (!key) {{
                alert('Silakan masukkan Admin Key / Setup Code terlebih dahulu pada kartu simulator di bawah.');
                return;
            }}
            const currentEl = document.getElementById('active-model-display');
            const current = currentEl ? currentEl.innerText.trim() : '';
            const choice = prompt(
                `Pilih Model AI Default untuk Aina:\n\nModel aktif saat ini: ${{current}}\n\nPilihan Model Tersedia:\n• gemini-3.8-flash-medium (Default Cepat & Seimbang)\n• gemini-3.8-flash-high (Penalaran Tinggi / Deep Thinking)\n• gemini-3.8-flash-low (Respons Kilat & Kasual)\n• gemini-3.1-pro-high (Deep Coding & Arsitektur)\n• claude-opus-4-6-thinking (Claude Opus Thinking - Khusus Eksplisit)\n• claude-sonnet-4-6 (Claude Sonnet 4.6)\n\nMasukkan nama model baru:`,
                current
            );
            if (!choice || choice.trim() === current || choice.trim() === '') return;

            try {{
                const res = await fetch('/api/model', {{
                    method: 'POST',
                    headers: {{
                        'Content-Type': 'application/json',
                        'X-Admin-Key': key
                    }},
                    body: JSON.stringify({{ model: choice.trim() }})
                }});
                const data = await res.json();
                if (res.ok && data.success) {{
                    alert(`✅ Sukses! Model default Aina sekarang: ${{data.model}}`);
                    if (currentEl) currentEl.innerText = data.model;
                }} else {{
                    alert(`⚠️ Gagal mengganti model: ${{data.error || data.message || 'Error'}}`);
                }}
            }} catch(e) {{
                alert(`Koneksi error: ${{e.message}}`);
            }}
        }}

        function onChatTypeChange(val) {{
            const el = document.getElementById('mention-toggle-wrapper');
            if (el) el.style.display = (val === 'group') ? 'block' : 'none';
        }}

        function onSessionRoleChange(role) {{
            const fromMeEl = document.getElementById('sim-is-from-me');
            if (role === 'user_companion' && fromMeEl && !fromMeEl.checked) {{
                // Keep checkbox flexible for testing companion personal DMs or owner group commands
            }}
        }}

        function onIsFromMeChange(checked) {{
            const nameInput = document.getElementById('sim-sender-name');
            if (checked) {{
                if (!nameInput.dataset.original) nameInput.dataset.original = nameInput.value;
                nameInput.value = 'Saya (Owner)';
            }} else if (nameInput.dataset.original) {{
                nameInput.value = nameInput.dataset.original;
            }}
        }}

        function escapeHtml(str) {{
            if (!str) return '';
            return str
                .replace(/&/g, "&amp;")
                .replace(/</g, "&lt;")
                .replace(/>/g, "&gt;")
                .replace(/"/g, "&quot;")
                .replace(/'/g, "&#039;");
        }}

        async function resetSimulationChat() {{
            const adminKey = localStorage.getItem('aina_admin_key') || '';
            const threadEl = document.getElementById('sim-chat-thread');
            try {{
                await fetch('/api/simulate/reset', {{
                    method: 'POST',
                    headers: {{ 'X-Admin-Key': adminKey }}
                }});
                if (threadEl) {{
                    threadEl.innerHTML = `
                        <div class="chat-bubble-system">
                            🔄 Sesi percakapan direset. Anda dapat memulai obrolan atau pengujian topik baru dari awal.
                        </div>
                    `;
                }}
                const textEl = document.getElementById('sim-text');
                if (textEl) {{
                    textEl.value = '';
                    textEl.focus();
                }}
            }} catch(e) {{
                alert('Gagal mereset sesi percakapan: ' + e.message);
            }}
        }}

        function copySnippetText(encoded) {{
            try {{
                const decoded = decodeURIComponent(encoded);
                navigator.clipboard.writeText(decoded);
                alert('Teks jawaban Aina berhasil disalin ke clipboard!');
            }} catch(e) {{
                console.error(e);
            }}
        }}

        async function runSimulation() {{
            const textInput = document.getElementById('sim-text');
            const text = textInput.value.trim();
            const sessionRole = document.getElementById('sim-session-role') ? document.getElementById('sim-session-role').value : 'primary_bot';
            const chatType = document.getElementById('sim-chat-type').value;
            const senderName = document.getElementById('sim-sender-name').value.trim() || 'Owner';
            const isMention = document.getElementById('sim-is-mention') ? document.getElementById('sim-is-mention').checked : false;
            const isFromMe = document.getElementById('sim-is-from-me') ? document.getElementById('sim-is-from-me').checked : false;
            const modelSelectEl = document.getElementById('sim-model-select');
            const selectedModel = modelSelectEl ? modelSelectEl.value : 'default';
            const modelOverride = (selectedModel !== 'default') ? selectedModel : null;

            const btn = document.getElementById('sim-btn');
            const threadEl = document.getElementById('sim-chat-thread');
            const adminKey = localStorage.getItem('aina_admin_key') || '';

            if (!text) {{
                alert('Tolong ketik pesan chat terlebih dahulu.');
                return;
            }}

            // 1. Remove initial empty placeholder if present
            const emptyEl = document.getElementById('sim-thread-empty');
            if (emptyEl) emptyEl.remove();

            // 2. Append User Message Bubble
            const userRow = document.createElement('div');
            userRow.className = 'chat-row-user';
            const roleTag = (sessionRole === 'user_companion') ? ' <span style="font-size: 0.65rem; background: rgba(56,189,248,0.2); padding: 1px 4px; border-radius: 4px; color: #38bdf8;">Companion</span>' : '';
            userRow.innerHTML = `
                <div class="chat-bubble-user">
                    <div style="font-size: 0.72rem; opacity: 0.8; margin-bottom: 3px; font-weight: 600;">${{escapeHtml(senderName)}}${{roleTag}}</div>
                    <div style="white-space: pre-wrap;">${{escapeHtml(text)}}</div>
                </div>
            `;
            threadEl.appendChild(userRow);

            // 3. Clear text input immediately for fast reply DX
            textInput.value = '';

            // 4. Append Typing Indicator Bubble
            const botRow = document.createElement('div');
            botRow.className = 'chat-row-bot';
            botRow.innerHTML = `
                <div class="chat-bubble-typing">
                    <span class="pulse" style="width: 8px; height: 8px;"></span>
                    <span class="typing-text">Aina sedang berpikir dan mengeksekusi... (0s)</span>
                </div>
            `;
            threadEl.appendChild(botRow);
            threadEl.scrollTop = threadEl.scrollHeight;

            btn.disabled = true;
            let secondsElapsed = 0;
            btn.innerText = 'Memproses (0s)...';
            const timerInterval = setInterval(() => {{
                secondsElapsed++;
                btn.innerText = `Memproses (${{secondsElapsed}}s)...`;
                const typingSpan = botRow.querySelector('.typing-text');
                if (typingSpan) typingSpan.innerText = `Aina sedang berpikir dan mengeksekusi... (${{secondsElapsed}}s)`;
            }}, 1000);

            try {{
                // Submit simulation job
                const res = await fetch('/api/simulate', {{
                    method: 'POST',
                    headers: {{
                        'Content-Type': 'application/json',
                        'X-Admin-Key': adminKey
                    }},
                    body: JSON.stringify({{
                        text: text,
                        session_role: sessionRole,
                        chat_type: chatType,
                        sender_name: senderName,
                        is_mention: isMention,
                        is_from_me: isFromMe,
                        model_override: modelOverride
                    }})
                }});

                if (res.status === 401) {{
                    alert('Sesi kedaluwarsa atau Admin Key tidak valid. Harap buka kunci kembali.');
                    lockAdminSession();
                    botRow.remove();
                    return;
                }}

                const contentType = res.headers.get('content-type') || '';
                if (!res.ok && !contentType.includes('application/json')) {{
                    const rawBody = await res.text();
                    let errMsg = `Server HTTP error ${{res.status}}`;
                    if (res.status === 524 || res.status === 504 || res.status === 529) {{
                        errMsg = `Gateway Timeout / Overloaded (${{res.status}}): Server proxy memutuskan koneksi.`;
                    }} else {{
                        errMsg = rawBody.slice(0, 200) || errMsg;
                    }}
                    throw new Error(errMsg);
                }}

                const initialData = await res.json();

                // If Gatekeeper answered immediately (e.g. Ignore or RecordOnly)
                if (initialData.decision && initialData.decision !== 'Respond') {{
                    botRow.innerHTML = `
                        <div class="chat-bubble-system">
                            🛡️ Gatekeeper: ${{escapeHtml(initialData.decision)}} (${{escapeHtml(initialData.reason)}}) - <em>(Aina menyimak tanpa membalas chat sesuai etika grup)</em>
                        </div>
                    `;
                    threadEl.scrollTop = threadEl.scrollHeight;
                    return;
                }}

                // If sync response was returned directly
                if (initialData.decision === 'Respond') {{
                    const replyText = initialData.response_text || '';
                    lastSimulationResponseText = replyText;
                    botRow.innerHTML = `
                        <div class="chat-bubble-bot">
                            <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 10px; border-bottom: 1px solid rgba(255,255,255,0.06); padding-bottom: 6px;">
                                <div style="font-size: 0.75rem; color: var(--primary); font-weight: 700; display: flex; align-items: center; gap: 6px;">
                                    <span>🌸 Aina</span>
                                    <span style="color: #94a3b8; font-weight: 400;">• ${{initialData.duration_seconds ? initialData.duration_seconds.toFixed(1) : secondsElapsed}}s</span>
                                    <span style="color: #64748b; font-weight: 400;">(${{escapeHtml(initialData.reason)}})</span>
                                </div>
                                <button type="button" class="copy-btn" onclick="copySnippetText('${{encodeURIComponent(replyText)}}')" style="padding: 2px 8px; font-size: 0.72rem;">Salin</button>
                            </div>
                            <div class="markdown-body">${{renderMarkdownToHtml(replyText)}}</div>
                        </div>
                    `;
                    threadEl.scrollTop = threadEl.scrollHeight;
                    textInput.focus();
                    return;
                }}

                const jobId = initialData.job_id;
                if (!jobId) {{
                    throw new Error(initialData.error || 'Gagal memulai pekerjaan simulasi.');
                }}

                // Poll job status
                let isFinished = false;
                while (!isFinished) {{
                    await new Promise(r => setTimeout(r, 1500));

                    const pollRes = await fetch(`/api/simulate/job/${{jobId}}`, {{
                        headers: {{ 'X-Admin-Key': adminKey }}
                    }});

                    if (!pollRes.ok) {{
                        if (pollRes.status === 404) {{
                            throw new Error('Sesi pekerjaan simulasi kedaluwarsa.');
                        }}
                        continue;
                    }}

                    const jobData = await pollRes.json();

                    if (jobData.status === 'processing') {{
                        continue;
                    }} else if (jobData.status === 'completed') {{
                        isFinished = true;
                        const replyText = jobData.response_text || '';
                        lastSimulationResponseText = replyText;
                        botRow.innerHTML = `
                            <div class="chat-bubble-bot">
                                <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 10px; border-bottom: 1px solid rgba(255,255,255,0.06); padding-bottom: 6px;">
                                    <div style="font-size: 0.75rem; color: var(--primary); font-weight: 700; display: flex; align-items: center; gap: 6px;">
                                        <span>🌸 Aina</span>
                                        <span style="color: #94a3b8; font-weight: 400;">• ${{jobData.duration_seconds ? jobData.duration_seconds.toFixed(1) : secondsElapsed}}s</span>
                                        <span style="color: #64748b; font-weight: 400;">(${{escapeHtml(jobData.reason)}})</span>
                                    </div>
                                    <button type="button" class="copy-btn" onclick="copySnippetText('${{encodeURIComponent(replyText)}}')" style="padding: 2px 8px; font-size: 0.72rem;">Salin</button>
                                </div>
                                <div class="markdown-body">${{renderMarkdownToHtml(replyText)}}</div>
                            </div>
                        `;
                        threadEl.scrollTop = threadEl.scrollHeight;
                        textInput.focus();
                    }} else if (jobData.status === 'failed') {{
                        isFinished = true;
                        throw new Error(jobData.error || 'Eksekusi agen AI gagal.');
                    }}
                }}
            }} catch(err) {{
                botRow.innerHTML = `
                    <div class="chat-bubble-system" style="border-color: rgba(239,68,68,0.4); color: #f87171;">
                        ⚠️ Gagal memproses simulasi: ${{escapeHtml(err.message)}}
                    </div>
                `;
                threadEl.scrollTop = threadEl.scrollHeight;
            }} finally {{
                clearInterval(timerInterval);
                btn.disabled = false;
                btn.innerHTML = '<span>Kirim</span> 🚀';
            }}
        }}

        function renderMarkdownToHtml(rawText) {{
            if (!rawText) return '';

            // 1. Configure marked
            marked.setOptions({{
                breaks: true,
                gfm: true,
                highlight: function(code, lang) {{
                    const language = (lang && hljs.getLanguage(lang)) ? lang : 'plaintext';
                    try {{
                        return hljs.highlight(code, {{ language }}).value;
                    }} catch(e) {{
                        return code;
                    }}
                }}
            }});

            // 2. Parse Markdown
            let rawHtml = marked.parse(rawText);

            // 3. Sanitize HTML
            let cleanHtml = DOMPurify.sanitize(rawHtml);

            // 4. Transform DOM for modern code headers and callout alerts
            const tempDiv = document.createElement('div');
            tempDiv.innerHTML = cleanHtml;

            // Code block decoration
            tempDiv.querySelectorAll('pre').forEach((pre) => {{
                const codeEl = pre.querySelector('code');
                let lang = 'code';
                if (codeEl) {{
                    const classes = Array.from(codeEl.classList);
                    const langClass = classes.find(c => c.startsWith('language-'));
                    if (langClass) lang = langClass.replace('language-', '');
                }}

                const wrapper = document.createElement('div');
                wrapper.className = 'code-block-wrapper';

                const header = document.createElement('div');
                header.className = 'code-block-header';
                header.innerHTML = `<span>${{lang}}</span><button type="button" class="copy-btn" onclick="copyCodeBlock(this)">Salin Kode</button>`;

                wrapper.appendChild(header);
                pre.parentNode.insertBefore(wrapper, pre);
                wrapper.appendChild(pre);
            }});

            // GitHub-style alerts: > [!NOTE], > [!TIP], > [!IMPORTANT], > [!WARNING], > [!CAUTION]
            tempDiv.querySelectorAll('blockquote').forEach((bq) => {{
                const text = bq.innerHTML.trim();
                const alertTypes = ['NOTE', 'TIP', 'IMPORTANT', 'WARNING', 'CAUTION'];
                const icons = {{
                    NOTE: 'ℹ️',
                    TIP: '💡',
                    IMPORTANT: '📌',
                    WARNING: '⚠️',
                    CAUTION: '🚨'
                }};
                for (const type of alertTypes) {{
                    const marker = `[!${{type}}]`;
                    if (text.includes(marker)) {{
                        const alertDiv = document.createElement('div');
                        alertDiv.className = `markdown-alert markdown-alert-${{type.toLowerCase()}}`;
                        const content = text.replace(marker, '').trim();
                        alertDiv.innerHTML = `<div class="markdown-alert-title">${{icons[type]}} ${{type}}</div><div>${{content}}</div>`;
                        bq.parentNode.replaceChild(alertDiv, bq);
                        break;
                    }}
                }}
            }});

            return tempDiv.innerHTML;
        }}

        function copyCodeBlock(btn) {{
            const wrapper = btn.closest('.code-block-wrapper');
            const codeEl = wrapper ? wrapper.querySelector('pre code') : null;
            if (!codeEl) return;
            navigator.clipboard.writeText(codeEl.innerText).then(() => {{
                const orig = btn.innerText;
                btn.innerText = '✓ Tersalin!';
                btn.style.color = '#10b981';
                setTimeout(() => {{
                    btn.innerText = orig;
                    btn.style.color = '';
                }}, 2000);
            }}).catch(e => {{
                console.error('Clipboard copy failed:', e);
            }});
        }}

        function copyFullResponse(btn) {{
            const textToCopy = lastSimulationResponseText || (document.getElementById('sim-response-text') ? document.getElementById('sim-response-text').innerText : '');
            if (!textToCopy) return;

            navigator.clipboard.writeText(textToCopy).then(() => {{
                const orig = btn.innerHTML;
                btn.innerHTML = '<span>✓ Jawaban Tersalin!</span>';
                btn.style.color = '#10b981';
                btn.style.borderColor = '#10b981';
                setTimeout(() => {{
                    btn.innerHTML = orig;
                    btn.style.color = '';
                    btn.style.borderColor = '';
                }}, 2000);
            }}).catch(e => {{
                console.error('Copy full response failed:', e);
            }});
        }}

        async function submitToken() {{
            const token = document.getElementById('token-input').value.trim();
            const setupCode = document.getElementById('setup-code-input').value.trim();
            const btn = document.getElementById('submit-btn');
            const alertBox = document.getElementById('alert-box');

            if (!setupCode) {{
                showAlert('Harap masukkan Kode Setup / Admin Key yang tertera di log.', false);
                return;
            }}

            if (!token) {{
                showAlert('Harap tempelkan token JSON terlebih dahulu.', false);
                return;
            }}

            try {{
                JSON.parse(token);
            }} catch(e) {{
                showAlert('Format token tidak valid: Harus berupa JSON yang valid.', false);
                return;
            }}

            btn.disabled = true;
            btn.innerText = 'Memverifikasi token...';
            alertBox.style.display = 'none';

            try {{
                const res = await fetch('/api/setup', {{
                    method: 'POST',
                    headers: {{ 'Content-Type': 'application/json' }},
                    body: JSON.stringify({{ token, setup_code: setupCode }})
                }});

                const data = await res.json();
                if (res.ok && data.success) {{
                    localStorage.setItem('aina_admin_key', setupCode);
                    showAlert(data.message + ' Halaman akan dimuat ulang...', true);
                    setTimeout(() => window.location.reload(), 2000);
                }} else {{
                    showAlert(data.message || 'Gagal memverifikasi token.', false);
                }}
            }} catch(err) {{
                showAlert('Terjadi kesalahan koneksi: ' + err.message, false);
            }} finally {{
                btn.disabled = false;
                btn.innerText = 'Verifikasi & Simpan Token';
            }}
        }}

        function showAlert(msg, isSuccess) {{
            const alertBox = document.getElementById('alert-box');
            alertBox.style.display = 'block';
            alertBox.className = 'alert ' + (isSuccess ? 'alert-success' : 'alert-error');
            alertBox.innerText = msg;
        }}

        // Run check on page load and attach Enter key handler
        function initPage() {{
            checkAdminAuth();
            const textEl = document.getElementById('sim-text');
            if (textEl && !textEl.dataset.bound) {{
                textEl.dataset.bound = 'true';
                textEl.addEventListener('keydown', function(e) {{
                    if (e.key === 'Enter' && !e.shiftKey) {{
                        e.preventDefault();
                        runSimulation();
                    }}
                }});
            }}
        }}
        document.addEventListener('DOMContentLoaded', initPage);
        initPage();
    </script>
</body>
</html>
        "#,
        badge_class = badge_class,
        badge_text = badge_text,
        status_card = status_card,
        auth_form_display = auth_form_display,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::domain::{ChatType, Platform, Sender, SessionRole};
    use serde_json::json;

    #[test]
    fn test_parse_whatsmeow_unassigned_bot_defaults_to_companion() {
        let payload = json!({
            "from": "628111222333@s.whatsapp.net",
            "body": "Halo, ini pesan dari rekan kerja",
            "id": "MSG_TEST_001",
            "is_from_me": false
        });

        // Bot is unassigned, but companion is active
        let msg = parse_whatsmeow_message(
            &payload,
            None,
            Some("unassigned@s.whatsapp.net"),
            Some("628999888777@s.whatsapp.net"),
            Some("default"),
        )
        .expect("Message should be parsed");

        assert_eq!(msg.session_role, SessionRole::UserCompanion);
        assert_eq!(msg.text, "Halo, ini pesan dari rekan kerja");
    }

    #[test]
    fn test_parse_whatsmeow_query_params_role() {
        let payload = json!({
            "from": "1203630123456789@g.us",
            "body": "!aina tolong rangkum",
            "id": "MSG_TEST_002",
            "is_from_me": false
        });

        let mut query_params = HashMap::new();
        query_params.insert("role".to_string(), "companion".to_string());

        let msg = parse_whatsmeow_message(
            &payload,
            Some(&query_params),
            Some("628123456789@s.whatsapp.net"),
            Some("628999888777@s.whatsapp.net"),
            Some("default"),
        )
        .expect("Message should be parsed");

        assert_eq!(msg.session_role, SessionRole::UserCompanion);
    }

    #[test]
    fn test_parse_whatsmeow_session_id_matching() {
        let payload = json!({
            "session_id": "companion_office",
            "from": "628999888777@s.whatsapp.net",
            "body": "Catatan tugas",
            "id": "MSG_TEST_003",
            "is_from_me": true
        });

        let msg = parse_whatsmeow_message(
            &payload,
            None,
            Some("628123456789@s.whatsapp.net"),
            Some("628999888777@s.whatsapp.net"),
            Some("companion_office"),
        )
        .expect("Message should be parsed");

        assert_eq!(msg.session_role, SessionRole::UserCompanion);
    }

    #[tokio::test]
    async fn test_dispatch_incoming_message_to_queue_fifo_ordering() {
        let chat_queues = Arc::new(tokio::sync::Mutex::new(HashMap::new()));
        let processed = Arc::new(tokio::sync::Mutex::new(Vec::new()));

        let make_msg = |id: &str, text: &str| IncomingMessage {
            id: id.to_string(),
            platform: Platform::WhatsApp,
            session_role: SessionRole::PrimaryBot,
            chat_jid: "group123@g.us".to_string(),
            chat_type: ChatType::Group,
            sender: Sender {
                jid: "user456@s.whatsapp.net".to_string(),
                name: Some("Owner".to_string()),
            },
            text: text.to_string(),
            timestamp: 123456,
            is_from_me: false,
            quoted_message: None,
            mentioned_jids: vec![],
            is_bot_mentioned: true,
            bot_lid: None,
            has_media: false,
            media_type: None,
            media_path: None,
        };

        let p1 = Arc::clone(&processed);
        dispatch_incoming_message_to_queue(&chat_queues, make_msg("M1", "First"), 1, move |msg| {
            let p_inner = Arc::clone(&p1);
            async move {
                tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                p_inner.lock().await.push(msg.id);
                Ok(())
            }
        })
        .await;

        let p2 = Arc::clone(&processed);
        dispatch_incoming_message_to_queue(&chat_queues, make_msg("M2", "Second"), 1, move |msg| {
            let p_inner = Arc::clone(&p2);
            async move {
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                p_inner.lock().await.push(msg.id);
                Ok(())
            }
        })
        .await;

        let p3 = Arc::clone(&processed);
        dispatch_incoming_message_to_queue(&chat_queues, make_msg("M3", "Third"), 1, move |msg| {
            let p_inner = Arc::clone(&p3);
            async move {
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                p_inner.lock().await.push(msg.id);
                Ok(())
            }
        })
        .await;

        let start = std::time::Instant::now();
        loop {
            let count = processed.lock().await.len();
            if count == 3 || start.elapsed() > std::time::Duration::from_secs(3) {
                break;
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
        }

        let results = processed.lock().await.clone();
        assert_eq!(results, vec!["M1", "M2", "M3"]);

        // Verify worker cleaned up from chat_queues after 1s idle timeout
        tokio::time::sleep(tokio::time::Duration::from_millis(1200)).await;
        let queues = chat_queues.lock().await;
        assert!(!queues.contains_key("group123@g.us"));
    }

    #[tokio::test]
    async fn test_webhook_large_payload_limit() {
        let app = Router::new()
            .route(
                "/test_limit",
                post(|body: String| async move {
                    (StatusCode::OK, format!("len: {}", body.len()))
                }),
            )
            .layer(DefaultBodyLimit::max(100 * 1024 * 1024));

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });

        let client = reqwest::Client::new();
        // Generate a 5MB payload (exceeds standard 2MB Axum default limit)
        let payload_size = 5 * 1024 * 1024;
        let large_data = "x".repeat(payload_size);
        let resp = client
            .post(format!("http://{}/test_limit", addr))
            .body(large_data)
            .send()
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let text = resp.text().await.unwrap();
        assert_eq!(text, format!("len: {}", payload_size));
    }

    #[test]
    fn test_resolve_media_extension_variants() {
        assert_eq!(resolve_media_extension("application/pdf", Some("doc.pdf"), Some("document")), "pdf");
        assert_eq!(resolve_media_extension("application/pdf", None, Some("document")), "pdf");
        assert_eq!(resolve_media_extension("image/png", Some("snapshot.png"), Some("image")), "png");
        assert_eq!(resolve_media_extension("image/jpeg; charset=utf-8", None, Some("image")), "jpg");
        assert_eq!(resolve_media_extension("video/mp4", None, Some("video")), "mp4");
        assert_eq!(resolve_media_extension("audio/ogg", None, Some("audio")), "ogg");
        assert_eq!(resolve_media_extension("audio/mp4", None, Some("audio")), "m4a");
        assert_eq!(resolve_media_extension("application/vnd.openxmlformats-officedocument.wordprocessingml.document", Some("report.docx"), Some("document")), "docx");
    }

    #[test]
    fn test_parse_whatsmeow_claim_check_payload() {
        let payload = json!({
            "id": "MSG_CLAIM_CHECK_999",
            "from": "6282234120921@s.whatsapp.net",
            "body": "ini pdfnya",
            "has_media": true,
            "media_type": "document",
            "download_url": "/api/v1/media/MSG_CLAIM_CHECK_999/download",
            "filename": "laporan_keuangan_q3.pdf",
            "mime_type": "application/pdf",
            "file_length": 15518976
        });

        let msg = parse_whatsmeow_message(
            &payload,
            None,
            Some("628123456789@s.whatsapp.net"),
            None,
            None,
        )
        .expect("Message should parse");

        assert_eq!(msg.id, "MSG_CLAIM_CHECK_999");
        assert_eq!(msg.has_media, true);
        assert_eq!(msg.media_type, Some("document".to_string()));
        assert_eq!(msg.text, "ini pdfnya");
    }

    struct DummySessionStore;
    #[async_trait::async_trait]
    impl crate::core::ports::SessionStorePort for DummySessionStore {
        async fn get_conversation_id(&self, _chat_jid: &str) -> anyhow::Result<Option<String>> { Ok(None) }
        async fn save_conversation_id(&self, _chat_jid: &str, _conv_uuid: &str) -> anyhow::Result<()> { Ok(()) }
        async fn delete_conversation_id(&self, _chat_jid: &str) -> anyhow::Result<()> { Ok(()) }
        async fn record_message(&self, _chat_jid: &str, _sender_jid: &str, _text: &str, _is_from_me: bool) -> anyhow::Result<()> { Ok(()) }
        async fn get_user_profile(&self, _sender_jid: &str) -> anyhow::Result<Option<crate::core::ports::UserProfile>> { Ok(None) }
        async fn save_user_profile(&self, _profile: &crate::core::ports::UserProfile) -> anyhow::Result<()> { Ok(()) }
    }

    struct DummyAgentEngine;
    #[async_trait::async_trait]
    impl crate::core::ports::AgentEnginePort for DummyAgentEngine {
        async fn execute_with_model(&self, _conv_id: Option<&str>, _prompt: &str, _model: Option<&str>) -> anyhow::Result<crate::core::ports::AgentResponse> {
            Ok(crate::core::ports::AgentResponse {
                conversation_id: "dummy".into(),
                response_text: "OK".into(),
                duration_seconds: 0.1,
            })
        }
        async fn get_model(&self) -> String { "dummy".into() }
        async fn set_model(&self, _model: &str) -> anyhow::Result<()> { Ok(()) }
        async fn is_authenticated(&self) -> bool { true }
        async fn save_auth_token(&self, _token_content: &str) -> anyhow::Result<()> { Ok(()) }
    }

    struct DummyWhatsApp;
    #[async_trait::async_trait]
    impl crate::core::ports::WhatsAppPort for DummyWhatsApp {
        async fn send_text(&self, _to: &str, _text: &str, _qid: Option<&str>) -> anyhow::Result<()> { Ok(()) }
        async fn send_presence(&self, _to: &str, _state: crate::core::domain::PresenceState) -> anyhow::Result<()> { Ok(()) }
    }

    #[tokio::test]
    async fn test_webhook_claim_check_stream_download() {
        const DUMMY_PDF_CONTENT: &[u8] = b"%PDF-1.4 Mock Claim Check Large Document Content";
        let mock_app = Router::new().route(
            "/api/v1/media/{id}/download",
            get(|Path(id): Path<String>, headers: HeaderMap| async move {
                let api_key = headers.get("X-API-Key").and_then(|v| v.to_str().ok()).unwrap_or("");
                if api_key != "secret-whatsmeow-key" {
                    return (StatusCode::UNAUTHORIZED, axum::http::HeaderMap::new(), vec![]);
                }
                let mut resp_headers = HeaderMap::new();
                resp_headers.insert("Content-Type", "application/pdf".parse().unwrap());
                resp_headers.insert("Content-Disposition", format!("attachment; filename=\"{}.pdf\"", id).parse().unwrap());
                (StatusCode::OK, resp_headers, DUMMY_PDF_CONTENT.to_vec())
            }),
        );

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let _ = axum::serve(listener, mock_app).await;
        });

        let test_id = format!("aina_cc_test_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos());
        let temp_dir = std::env::temp_dir().join(test_id);
        let _ = tokio::fs::create_dir_all(&temp_dir).await;

        let session_store = Arc::new(DummySessionStore);
        let agent_engine = Arc::new(DummyAgentEngine);
        let whatsapp = Arc::new(DummyWhatsApp);
        let persona_engine = Arc::new(PersonaEngine::new(
            "Aina".to_string(),
            "Org".to_string(),
            "admin@s.whatsapp.net".to_string(),
            "Asia/Jakarta".to_string(),
            7,
            "id".to_string(),
            format!("http://{}", addr),
            "bot@s.whatsapp.net".to_string(),
            Some(temp_dir.to_string_lossy().to_string()),
        ));

        let usecase = Arc::new(ProcessIncomingMessageUseCase::new(
            session_store.clone(),
            agent_engine.clone(),
            whatsapp.clone(),
            persona_engine.clone(),
            "bot@s.whatsapp.net".to_string(),
            "Aina".to_string(),
            None,
        ));

        let state = Arc::new(WebhookServerState {
            usecase,
            agent_engine,
            session_store,
            persona_engine,
            bot_name: "Aina".to_string(),
            bot_jid: "bot@s.whatsapp.net".to_string(),
            bot_lid: None,
            companion_jid: None,
            companion_name: None,
            companion_session_id: None,
            model: "dummy".to_string(),
            whatsmeow_url: format!("http://{}", addr),
            whatsmeow_api_key: "secret-whatsmeow-key".to_string(),
            companion_base_url: None,
            companion_api_key: None,
            setup_code: "SECRET123".to_string(),
            timezone: "Asia/Jakarta".to_string(),
            locale: "id".to_string(),
            sim_jobs: Arc::new(RwLock::new(HashMap::new())),
            chat_queues: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            workspace_dir: temp_dir.clone(),
        });

        let app = create_router(state);
        let aina_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let aina_addr = aina_listener.local_addr().unwrap();
        tokio::spawn(async move {
            let _ = axum::serve(aina_listener, app).await;
        });

        let client = reqwest::Client::new();
        let payload = json!({
            "id": "MSG_CC_TEST_001",
            "chat_jid": "user123@s.whatsapp.net",
            "sender_jid": "user123@s.whatsapp.net",
            "download_url": "/api/v1/media/MSG_CC_TEST_001/download",
            "media_url": "/api/v1/media/MSG_CC_TEST_001/download",
            "has_media": true,
            "media_type": "document",
            "filename": "laporan_keuangan.pdf",
            "mime_type": "application/pdf",
            "file_length": DUMMY_PDF_CONTENT.len(),
            "text": "tolong cek pdf ini"
        });

        let resp = client
            .post(format!("http://{}/webhook", aina_addr))
            .json(&payload)
            .send()
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);

        let target_pdf = temp_dir.join("media").join("MSG_CC_TEST_001.pdf");
        let target_txt = temp_dir.join("media").join("MSG_CC_TEST_001.txt");

        let mut downloaded = false;
        for _ in 0..50 {
            if target_pdf.exists() && target_txt.exists() {
                downloaded = true;
                break;
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        }

        assert!(downloaded, "Media file or sidecar .txt was not created by claim-check handler");
        let read_bytes = tokio::fs::read(&target_pdf).await.unwrap();
        assert_eq!(read_bytes, DUMMY_PDF_CONTENT);

        let sidecar_content = tokio::fs::read_to_string(&target_txt).await.unwrap();
        assert!(sidecar_content.contains("ID: MSG_CC_TEST_001"));
        assert!(sidecar_content.contains("laporan_keuangan.pdf"));
        assert!(sidecar_content.contains("application/pdf"));

        let _ = tokio::fs::remove_dir_all(&temp_dir).await;
    }
}
