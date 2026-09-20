use crate::adapters::driving::web::state::WebhookServerState;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
pub struct ScheduleTasksQuery {
    pub active_only: Option<bool>,
}

pub async fn api_schedule_tasks_handler(
    State(state): State<Arc<WebhookServerState>>,
    Query(query): Query<ScheduleTasksQuery>,
) -> impl IntoResponse {
    let active_only = query.active_only.unwrap_or(false);
    match state.session_store.list_scheduled_tasks(active_only).await {
        Ok(tasks) => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "count": tasks.len(),
                "tasks": tasks,
            })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "success": false,
                "error": format!("Gagal memuat daftar jadwal: {}", e),
            })),
        ),
    }
}

#[derive(Debug, Deserialize)]
pub struct ScheduleRunsQuery {
    pub limit: Option<usize>,
}

pub async fn api_schedule_runs_handler(
    State(state): State<Arc<WebhookServerState>>,
    Query(query): Query<ScheduleRunsQuery>,
) -> impl IntoResponse {
    let limit = query.limit.unwrap_or(50).min(500);
    match state.session_store.list_scheduled_task_runs(limit).await {
        Ok(runs) => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "count": runs.len(),
                "runs": runs,
            })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "success": false,
                "error": format!("Gagal memuat riwayat eksekusi jadwal: {}", e),
            })),
        ),
    }
}

pub async fn api_schedule_diagnostics_handler(
    State(state): State<Arc<WebhookServerState>>,
) -> impl IntoResponse {
    match state.session_store.get_scheduler_diagnostics().await {
        Ok(diag) => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "diagnostics": diag,
            })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "success": false,
                "error": format!("Gagal mengambil metrik diagnostik scheduler: {}", e),
            })),
        ),
    }
}

