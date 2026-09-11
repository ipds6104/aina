use axum::{
    extract::{Path, State},
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
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info};

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
    pub model: String,
    pub whatsmeow_url: String,
    pub setup_code: String,
    pub timezone: String,
    pub locale: String,
    pub sim_jobs: Arc<RwLock<HashMap<String, SimulationJob>>>,
}

pub fn create_router(state: Arc<WebhookServerState>) -> Router {
    Router::new()
        .route("/", get(dashboard_or_setup_handler))
        .route("/setup", get(setup_page_handler))
        .route("/health", get(health_handler))
        .route("/api/status", get(api_status_handler))
        .route("/api/setup", post(api_setup_handler))
        .route("/api/auth/verify", post(api_verify_admin_handler))
        .route("/api/simulate", post(simulate_handler))
        .route("/api/simulate/job/{id}", get(simulate_job_status_handler))
        .route("/webhook", post(webhook_handler))
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
    pub model: String,
    pub whatsmeow_url: String,
    pub timezone: String,
    pub locale: String,
}

async fn api_status_handler(
    State(state): State<Arc<WebhookServerState>>,
) -> impl IntoResponse {
    let auth = state.agent_engine.is_authenticated().await;
    Json(ApiStatusResponse {
        authenticated: auth,
        bot_name: state.bot_name.clone(),
        bot_jid: state.bot_jid.clone(),
        model: state.model.clone(),
        whatsmeow_url: state.whatsmeow_url.clone(),
        timezone: state.timezone.clone(),
        locale: state.locale.clone(),
    })
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
    pub text: String,
}

#[derive(Debug, Serialize)]
struct SimulateResponse {
    pub decision: String,
    pub reason: String,
    pub response_text: Option<String>,
    pub duration_seconds: Option<f64>,
    pub conversation_id: Option<String>,
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

    let sender_name = payload.sender_name.unwrap_or_else(|| "Pengguna Tester".to_string());
    let sender_jid = payload.sender_jid.unwrap_or_else(|| "628999888777@s.whatsapp.net".to_string());
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
        chat_jid: chat_jid.clone(),
        chat_type,
        sender: Sender {
            jid: sender_jid.clone(),
            name: Some(sender_name),
        },
        text: payload.text,
        timestamp: chrono_now_secs(),
        is_from_me: false,
        quoted_message: None,
        mentioned_jids,
    };

    let decision = Gatekeeper::evaluate(&msg, &state.bot_jid, &state.bot_name);

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

            tokio::spawn(async move {
                info!("Starting async agent execution for job {}", job_id_clone);
                let exec_res = state_clone.agent_engine.execute(conv_id.as_deref(), &prompt).await;
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
    Html(render_html(is_auth, &state))
}

async fn setup_page_handler(
    State(state): State<Arc<WebhookServerState>>,
) -> impl IntoResponse {
    let is_auth = state.agent_engine.is_authenticated().await;
    Html(render_html(is_auth, &state))
}

async fn webhook_handler(
    State(state): State<Arc<WebhookServerState>>,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    debug!("Received webhook payload: {:?}", payload);

    let msg_opt = parse_whatsmeow_message(&payload);

    if let Some(msg) = msg_opt {
        let usecase = Arc::clone(&state.usecase);
        tokio::spawn(async move {
            if let Err(e) = usecase.execute(msg).await {
                error!("Error processing message: {:?}", e);
            }
        });
    } else {
        debug!("Webhook received event that was not a parseable user message");
    }

    (StatusCode::OK, "OK")
}

fn parse_whatsmeow_message(val: &Value) -> Option<IncomingMessage> {
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

    Some(IncomingMessage {
        id: msg_id,
        platform: crate::core::domain::Platform::WhatsApp,
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
    })
}

fn chrono_now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn render_html(is_authenticated: bool, state: &WebhookServerState) -> String {
    let (badge_class, badge_text, badge_bg) = if is_authenticated {
        ("badge-success", "ONLINE & TERAUTENTIKASI", "#10b981")
    } else {
        ("badge-warning", "PERLU SETUP AUTENTIKASI", "#f59e0b")
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
                    <div class="info-item"><span class="label">Nama Bot</span><span class="val">{name}</span></div>
                    <div class="info-item"><span class="label">WhatsApp JID</span><span class="val">{jid}</span></div>
                    <div class="info-item"><span class="label">Model AI</span><span class="val">{model}</span></div>
                    <div class="info-item"><span class="label">Whatsmeow</span><span class="val">{url}</span></div>
                    <div class="info-item"><span class="label">Zona Waktu</span><span class="val">{timezone}</span></div>
                    <div class="info-item"><span class="label">Locale</span><span class="val">{locale}</span></div>
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
                    Uji langsung logika respons, etika grup (Gatekeeper), dan eksekusi agentik Antigravity secara nyata tanpa harus mengirim chat dari HP Anda.
                </p>

                <div style="display: grid; grid-template-columns: 1fr 1fr; gap: 12px; margin-bottom: 12px;">
                    <div>
                        <label class="form-label">Tipe Obrolan</label>
                        <select id="sim-chat-type" class="form-input" onchange="onChatTypeChange(this.value)">
                            <option value="dm">Pesan Pribadi (DM)</option>
                            <option value="group">Grup WhatsApp Kantor</option>
                        </select>
                    </div>
                    <div>
                        <label class="form-label">Nama Pengirim</label>
                        <input id="sim-sender-name" class="form-input" value="Ihza" />
                    </div>
                </div>

                <div id="mention-toggle-wrapper" style="display: none; margin-bottom: 12px;">
                    <label style="font-size: 0.85rem; color: #94a3b8; display: flex; align-items: center; gap: 8px; cursor: pointer;">
                        <input type="checkbox" id="sim-is-mention" />
                        <span>Simulasikan Tag / Mention (@Aina) dalam grup</span>
                    </label>
                </div>

                <label class="form-label">Isi Pesan Chat</label>
                <textarea id="sim-text" class="form-input" style="height: 80px;" placeholder="Contoh: Aina, tolong buatkan script python untuk cek koneksi..."></textarea>

                <button id="sim-btn" class="btn" style="width: 100%; margin-top: 8px;" onclick="runSimulation()">
                    Kirim & Uji Respon Aina
                </button>

                <div id="sim-result-box" style="display: none; margin-top: 18px;">
                    <div class="chat-bubble">
                        <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 10px; border-bottom: 1px solid rgba(255,255,255,0.06); padding-bottom: 8px;">
                            <div style="font-size: 0.75rem; color: var(--primary); font-weight: 700; display: flex; align-items: center; gap: 6px;" id="sim-meta"></div>
                            <button id="copy-full-btn" type="button" class="copy-btn" onclick="copyFullResponse(this)" style="display: inline-flex; align-items: center; gap: 5px; font-weight: 600; padding: 4px 10px;" title="Salin seluruh jawaban Aina">
                                <span>📋 Salin Jawaban</span>
                            </button>
                        </div>
                        <div id="sim-response-text" class="markdown-body"></div>
                    </div>
                </div>
            </div>
            "#,
            badge_bg = badge_bg,
            name = state.bot_name,
            jid = state.bot_jid,
            model = state.model,
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
        
        /* Modern Chat Bubble & Trending Markdown Styling */
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

        function onChatTypeChange(val) {{
            const el = document.getElementById('mention-toggle-wrapper');
            if (el) el.style.display = (val === 'group') ? 'block' : 'none';
        }}

        async function runSimulation() {{
            const text = document.getElementById('sim-text').value.trim();
            const chatType = document.getElementById('sim-chat-type').value;
            const senderName = document.getElementById('sim-sender-name').value.trim();
            const isMention = document.getElementById('sim-is-mention') ? document.getElementById('sim-is-mention').checked : false;
            const btn = document.getElementById('sim-btn');
            const resBox = document.getElementById('sim-result-box');
            const metaEl = document.getElementById('sim-meta');
            const textEl = document.getElementById('sim-response-text');
            const adminKey = localStorage.getItem('aina_admin_key') || '';

            if (!text) {{
                alert('Tolong ketik pesan chat terlebih dahulu.');
                return;
            }}

            btn.disabled = true;
            let secondsElapsed = 0;
            btn.innerText = 'Memulai simulasi... (0s)';
            const timerInterval = setInterval(() => {{
                secondsElapsed++;
                btn.innerText = `Aina sedang berpikir dan mengeksekusi... (${{secondsElapsed}}s)`;
            }}, 1000);
            resBox.style.display = 'none';

            try {{
                // 1. Submit simulation job (<5ms response, completely immune to proxy timeout)
                const res = await fetch('/api/simulate', {{
                    method: 'POST',
                    headers: {{
                        'Content-Type': 'application/json',
                        'X-Admin-Key': adminKey
                    }},
                    body: JSON.stringify({{
                        text: text,
                        chat_type: chatType,
                        sender_name: senderName,
                        is_mention: isMention
                    }})
                }});

                if (res.status === 401) {{
                    alert('Sesi kedaluwarsa atau Admin Key tidak valid. Harap buka kunci kembali.');
                    lockAdminSession();
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
                    resBox.style.display = 'block';
                    lastSimulationResponseText = '';
                    metaEl.innerText = `Gatekeeper: ${{initialData.decision}} (${{initialData.reason}})`;
                    textEl.innerHTML = `<em>(Aina menyimak/mengabaikan pesan ini sesuai etika grup kantor tanpa membalas chat)</em>`;
                    const copyBtn = document.getElementById('copy-full-btn');
                    if (copyBtn) copyBtn.style.display = 'none';
                    return;
                }}

                // If sync response was returned directly
                if (initialData.decision === 'Respond') {{
                    resBox.style.display = 'block';
                    lastSimulationResponseText = initialData.response_text || '';
                    metaEl.innerText = `Aina membalas (${{initialData.duration_seconds ? initialData.duration_seconds.toFixed(2) : secondsElapsed}}s) - Alasan: ${{initialData.reason}}`;
                    textEl.innerHTML = renderMarkdownToHtml(initialData.response_text || '');
                    const copyBtn = document.getElementById('copy-full-btn');
                    if (copyBtn) copyBtn.style.display = 'inline-flex';
                    return;
                }}

                const jobId = initialData.job_id;
                if (!jobId) {{
                    throw new Error(initialData.error || 'Gagal memulai pekerjaan simulasi.');
                }}

                // 2. Poll job status (each poll takes ~2ms, completely immune to Cloudflare 100s timeout!)
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
                        resBox.style.display = 'block';
                        lastSimulationResponseText = jobData.response_text || '';
                        metaEl.innerText = `Aina membalas (${{jobData.duration_seconds ? jobData.duration_seconds.toFixed(2) : secondsElapsed}}s) - Alasan: ${{jobData.reason}}`;
                        textEl.innerHTML = renderMarkdownToHtml(jobData.response_text || '');
                        const copyBtn = document.getElementById('copy-full-btn');
                        if (copyBtn) copyBtn.style.display = 'inline-flex';
                    }} else if (jobData.status === 'failed') {{
                        isFinished = true;
                        throw new Error(jobData.error || 'Eksekusi agen AI gagal.');
                    }}
                }}
            }} catch(err) {{
                alert('Gagal menjalankan simulasi: ' + err.message);
            }} finally {{
                clearInterval(timerInterval);
                btn.disabled = false;
                btn.innerText = 'Kirim & Uji Respon Aina';
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

        // Run check on page load
        document.addEventListener('DOMContentLoaded', checkAdminAuth);
        checkAdminAuth();
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
