use crate::adapters::driving::web::auth::is_api_authorized;
use crate::adapters::driving::web::state::WebhookServerState;
use crate::adapters::driving::web::ui::chrono_now_secs;
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
pub struct AuditActionsQuery {
    pub chat_jid: Option<String>,
    pub sender_jid: Option<String>,
    pub decision: Option<String>,
    pub status: Option<String>,
    pub q: Option<String>,
    pub since: Option<i64>,
    pub until: Option<i64>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
    pub key: Option<String>,
    pub api_key: Option<String>,
}

pub async fn api_audit_actions_handler(
    State(state): State<Arc<WebhookServerState>>,
    headers: HeaderMap,
    Query(query): Query<AuditActionsQuery>,
) -> impl IntoResponse {
    let key_candidate = query.key.as_deref().or(query.api_key.as_deref());
    if !is_api_authorized(&headers, key_candidate, &state) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "success": false,
                "error": "Akses ditolak. Berikan API Key yang valid via header (Authorization: Bearer <key>, X-API-Key: <key>, X-Admin-Key: <key>) atau query parameter (?key=<key>)."
            })),
        );
    }

    let filter = crate::core::domain::ActionAuditFilter {
        chat_jid: query.chat_jid,
        sender_jid: query.sender_jid,
        decision: query.decision,
        status: query.status,
        query: query.q,
        since_epoch: query.since,
        until_epoch: query.until,
        limit: query.limit,
        offset: query.offset,
    };

    match state.session_store.query_action_audits(&filter).await {
        Ok(actions) => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "count": actions.len(),
                "actions": actions,
            })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "success": false,
                "error": format!("Gagal mengambil riwayat audit aksi: {}", e),
            })),
        ),
    }
}

#[derive(Debug, Deserialize)]
pub struct AuditActionDetailQuery {
    pub key: Option<String>,
    pub api_key: Option<String>,
    pub limit: Option<usize>,
    pub all: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct AuditAuthOnlyQuery {
    pub key: Option<String>,
    pub api_key: Option<String>,
}

pub async fn api_audit_action_detail_handler(
    State(state): State<Arc<WebhookServerState>>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Query(query): Query<AuditActionDetailQuery>,
) -> impl IntoResponse {
    let key_candidate = query.key.as_deref().or(query.api_key.as_deref());
    if !is_api_authorized(&headers, key_candidate, &state) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "success": false,
                "error": "Akses ditolak. Berikan API Key yang valid via header (Authorization: Bearer <key>, X-API-Key: <key>, X-Admin-Key: <key>) atau query parameter (?key=<key>)."
            })),
        );
    }

    let audit_res = if let Ok(numeric_id) = id.parse::<i64>() {
        state.session_store.get_action_audit_by_id(numeric_id).await
    } else {
        state.session_store.get_action_audit_by_message_id(&id).await
    };

    match audit_res {
        Ok(Some(audit)) => {
            let brain_path = crate::core::domain::AuditEngine::default_brain_path();
            let (raw_steps, _) = if let Some(ref conv_id) = audit.conversation_id {
                crate::core::domain::AuditEngine::load_transcript_for_conversation(&brain_path, conv_id)
            } else {
                (Vec::new(), Vec::new())
            };

            let total_transcript_steps = raw_steps.len();
            let running_tasks_count = raw_steps.iter().filter(|s| s.status.as_deref() == Some("RUNNING")).count();
            let has_running_tasks = running_tasks_count > 0;

            let transcript_steps = if query.all == Some(true) {
                raw_steps
            } else {
                let limit = query.limit.unwrap_or(50).max(1);
                if raw_steps.len() > limit {
                    let start = raw_steps.len().saturating_sub(limit);
                    raw_steps[start..].to_vec()
                } else {
                    raw_steps
                }
            };

            let transcript_path = audit.conversation_id.as_deref().and_then(|conv_id| {
                crate::core::domain::AuditEngine::find_transcript_path(&brain_path, conv_id)
                    .map(|p| p.to_string_lossy().to_string())
            });

            let workspace_provenance = crate::core::domain::AuditEngine::inspect_workspaces(&state.workspace_dir);

            let detailed = crate::core::domain::DetailedActionAudit {
                audit,
                transcript_path,
                transcript_steps,
                workspace_provenance,
                has_running_tasks,
                running_tasks_count,
                total_transcript_steps,
            };

            (
                StatusCode::OK,
                Json(json!({
                    "success": true,
                    "detail": detailed,
                })),
            )
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "success": false,
                "error": format!("Audit aksi dengan identitas '{}' tidak ditemukan", id),
            })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "success": false,
                "error": format!("Gagal memuat rincian audit: {}", e),
            })),
        ),
    }
}

pub async fn api_audit_summary_handler(
    State(state): State<Arc<WebhookServerState>>,
    headers: HeaderMap,
    Query(query): Query<AuditAuthOnlyQuery>,
) -> impl IntoResponse {
    let key_candidate = query.key.as_deref().or(query.api_key.as_deref());
    if !is_api_authorized(&headers, key_candidate, &state) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "success": false,
                "error": "Akses ditolak. Berikan API Key yang valid via header (Authorization: Bearer <key>, X-API-Key: <key>, X-Admin-Key: <key>) atau query parameter (?key=<key>)."
            })),
        );
    }

    match state.session_store.get_action_audit_summary().await {
        Ok(summary) => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "summary": summary,
            })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "success": false,
                "error": format!("Gagal menyusun ringkasan audit: {}", e),
            })),
        ),
    }
}

pub async fn api_audit_diagnostics_handler(
    State(state): State<Arc<WebhookServerState>>,
    headers: HeaderMap,
    Query(query): Query<AuditAuthOnlyQuery>,
) -> impl IntoResponse {
    let key_candidate = query.key.as_deref().or(query.api_key.as_deref());
    if !is_api_authorized(&headers, key_candidate, &state) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "success": false,
                "error": "Akses ditolak. Berikan API Key yang valid via header (Authorization: Bearer <key>, X-API-Key: <key>, X-Admin-Key: <key>) atau query parameter (?key=<key>)."
            })),
        );
    }

    let brain_path = crate::core::domain::AuditEngine::default_brain_path();
    let (rss, virt) = crate::core::domain::AuditEngine::read_process_memory();
    let db_size = std::fs::metadata("data/aina.db")
        .or_else(|_| std::fs::metadata("aina.db"))
        .map(|m| m.len())
        .unwrap_or(0);
    let brain_dir_size = crate::core::domain::AuditEngine::compute_dir_size(&brain_path);
    let total_conversations = crate::core::domain::AuditEngine::find_transcripts(&brain_path).len();
    let active_model = state.agent_engine.get_model().await;
    let workspaces = crate::core::domain::AuditEngine::inspect_workspaces(&state.workspace_dir);
    let scheduler = state.session_store.get_scheduler_diagnostics().await.ok();

    let diagnostics = crate::core::domain::SystemDiagnostics {
        uptime_seconds: crate::core::domain::get_process_uptime_secs(),
        memory_rss_bytes: rss,
        memory_virt_bytes: virt,
        memory_rss_human: crate::core::domain::AuditEngine::format_bytes(rss),
        memory_virt_human: crate::core::domain::AuditEngine::format_bytes(virt),
        db_size_bytes: db_size,
        brain_dir_size_bytes: brain_dir_size,
        total_conversations_in_brain: total_conversations,
        active_model,
        bot_jid: state.bot_jid.clone(),
        bot_name: state.bot_name.clone(),
        workspaces,
        scheduler,
        timestamp_epoch: chrono_now_secs(),
    };

    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "diagnostics": diagnostics,
        })),
    )
}

#[derive(Debug, Deserialize)]
pub struct AuditTranscriptsQuery {
    pub q: Option<String>,
    pub errors_only: Option<bool>,
    pub limit: Option<usize>,
    pub key: Option<String>,
    pub api_key: Option<String>,
}

pub async fn api_audit_transcripts_handler(
    State(state): State<Arc<WebhookServerState>>,
    headers: HeaderMap,
    Query(query): Query<AuditTranscriptsQuery>,
) -> impl IntoResponse {
    let key_candidate = query.key.as_deref().or(query.api_key.as_deref());
    if !is_api_authorized(&headers, key_candidate, &state) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "success": false,
                "error": "Akses ditolak. Berikan API Key yang valid via header (Authorization: Bearer <key>, X-API-Key: <key>, X-Admin-Key: <key>) atau query parameter (?key=<key>)."
            })),
        );
    }

    let brain_path = crate::core::domain::AuditEngine::default_brain_path();
    let limit = query.limit.unwrap_or(50).min(500);
    let steps = crate::core::domain::AuditEngine::query_audit_trail(
        &brain_path,
        query.q.as_deref(),
        query.errors_only.unwrap_or(false),
        limit,
    );

    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "count": steps.len(),
            "steps": steps,
        })),
    )
}

pub async fn api_audit_presence_handler(
    State(state): State<Arc<WebhookServerState>>,
    headers: HeaderMap,
    Query(query): Query<AuditAuthOnlyQuery>,
) -> impl IntoResponse {
    let key_candidate = query.key.as_deref().or(query.api_key.as_deref());
    if !is_api_authorized(&headers, key_candidate, &state) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "success": false,
                "error": "Akses ditolak. Berikan API Key yang valid."
            })),
        );
    }

    let snapshot = state.presence_tracker.snapshot().await;
    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "presence": snapshot,
        })),
    )
}

#[derive(Deserialize)]
pub struct StopPresencePayload {
    pub chat_jid: Option<String>,
}

pub async fn api_audit_presence_stop_handler(
    State(state): State<Arc<WebhookServerState>>,
    headers: HeaderMap,
    Query(query): Query<AuditAuthOnlyQuery>,
    payload: Option<Json<StopPresencePayload>>,
) -> impl IntoResponse {
    let key_candidate = query.key.as_deref().or(query.api_key.as_deref());
    if !is_api_authorized(&headers, key_candidate, &state) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "success": false,
                "error": "Akses ditolak. Berikan API Key yang valid."
            })),
        );
    }

    let target_jid = payload.as_ref().and_then(|p| p.chat_jid.as_deref());
    let cleared_jids = state.presence_tracker.clear_active(target_jid).await;
    for jid in &cleared_jids {
        let _ = state
            .usecase
            .whatsapp()
            .send_presence_with_session(jid, crate::core::domain::PresenceState::Paused, crate::core::domain::SessionRole::PrimaryBot)
            .await;
    }

    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "message": format!("Presence stopped for {} chats", cleared_jids.len()),
            "cleared_chats": cleared_jids,
        })),
    )
}
