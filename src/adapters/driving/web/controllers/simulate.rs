use crate::adapters::driving::web::auth::is_admin_authorized;
use crate::adapters::driving::web::state::{SimulationJob, SimulationJobStatus, WebhookServerState};
use crate::adapters::driving::web::ui::chrono_now_secs;
use crate::core::domain::{ChatType, Gatekeeper, GatekeeperDecision, IncomingMessage, Sender};
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use tracing::{error, info};

#[derive(Debug, Deserialize)]
pub struct SimulateRequest {
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
pub struct SimulateResponse {
    pub decision: String,
    pub reason: String,
    pub response_text: Option<String>,
    pub duration_seconds: Option<f64>,
    pub conversation_id: Option<String>,
}

pub async fn simulate_reset_handler(
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

pub async fn simulate_handler(
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
            let initial_usecase = crate::core::domain::UseCaseClassifier::classify(&msg.text, false, None, &[]);
            let audit = crate::core::domain::NewWhatsAppActionAudit {
                message_id: msg.id.clone(),
                chat_jid: msg.chat_jid.clone(),
                chat_type: "direct".to_string(),
                sender_jid: msg.sender.jid.clone(),
                sender_name: msg.sender.name.clone(),
                decision: "ignore".to_string(),
                decision_reason: reason.clone(),
                conversation_id: None,
                status: "ignored".to_string(),
                input_text: msg.text.clone(),
                has_media: false,
                media_path: None,
                response_text: None,
                error_message: None,
                duration_seconds: Some(0.0),
                tools_invoked: vec![],
                usecase: initial_usecase.as_str().to_string(),
                created_at_epoch: chrono_now_secs(),
                completed_at_epoch: Some(chrono_now_secs()),
            };
            let _ = state.session_store.record_action_audit(&audit).await;

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
            let initial_usecase = crate::core::domain::UseCaseClassifier::classify(&msg.text, false, None, &[]);
            let audit = crate::core::domain::NewWhatsAppActionAudit {
                message_id: msg.id.clone(),
                chat_jid: msg.chat_jid.clone(),
                chat_type: "group".to_string(),
                sender_jid: msg.sender.jid.clone(),
                sender_name: msg.sender.name.clone(),
                decision: "record_only".to_string(),
                decision_reason: reason.clone(),
                conversation_id: None,
                status: "recorded".to_string(),
                input_text: msg.text.clone(),
                has_media: false,
                media_path: None,
                response_text: None,
                error_message: None,
                duration_seconds: Some(0.0),
                tools_invoked: vec![],
                usecase: initial_usecase.as_str().to_string(),
                created_at_epoch: chrono_now_secs(),
                completed_at_epoch: Some(chrono_now_secs()),
            };
            let _ = state.session_store.record_action_audit(&audit).await;

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
            let initial_usecase = crate::core::domain::UseCaseClassifier::classify(&msg.text, false, None, &[]);
            let audit = crate::core::domain::NewWhatsAppActionAudit {
                message_id: msg.id.clone(),
                chat_jid: msg.chat_jid.clone(),
                chat_type: "direct".to_string(),
                sender_jid: msg.sender.jid.clone(),
                sender_name: msg.sender.name.clone(),
                decision: "respond".to_string(),
                decision_reason: reason.clone(),
                conversation_id: conv_id.clone(),
                status: "in_progress".to_string(),
                input_text: msg.text.clone(),
                has_media: false,
                media_path: None,
                response_text: None,
                error_message: None,
                duration_seconds: None,
                tools_invoked: vec![],
                usecase: initial_usecase.as_str().to_string(),
                created_at_epoch: now,
                completed_at_epoch: None,
            };
            let sim_audit_id = state.session_store.record_action_audit(&audit).await.ok();

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
            let msg_text_clone = msg.text.clone();
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

                        if let Some(aid) = sim_audit_id {
                            let brain_path = crate::core::domain::AuditEngine::default_brain_path();
                            let tools = crate::core::domain::AuditEngine::extract_tools_for_conversation(
                                &brain_path,
                                &agent_res.conversation_id,
                            );
                            let refined_usecase = crate::core::domain::UseCaseClassifier::classify(
                                &msg_text_clone,
                                false,
                                None,
                                &tools,
                            );
                            let _ = state_clone.session_store.update_action_audit_result(
                                aid,
                                Some(&agent_res.conversation_id),
                                Some(&agent_res.response_text),
                                None,
                                "success",
                                Some(agent_res.duration_seconds),
                                &tools,
                                Some(refined_usecase.as_str()),
                            ).await;
                        }

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
                        if let Some(aid) = sim_audit_id {
                            let dur = (fin_time - now).max(0) as f64;
                            let _ = state_clone.session_store.update_action_audit_result(
                                aid,
                                None,
                                None,
                                Some(&e.to_string()),
                                "failed",
                                Some(dur),
                                &[],
                                None,
                            ).await;
                        }

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

pub async fn simulate_job_status_handler(
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

