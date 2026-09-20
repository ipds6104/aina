use crate::adapters::driving::web::state::WebhookServerState;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;

pub async fn api_persona_diagnostics_handler() -> impl IntoResponse {
    let script_candidates = [
        PathBuf::from("scripts/persona_status.py"),
        PathBuf::from("/app/scripts/persona_status.py"),
        PathBuf::from("/root/projects/aina/scripts/persona_status.py"),
    ];

    let resolved = script_candidates
        .into_iter()
        .find(|p| p.exists())
        .unwrap_or_else(|| PathBuf::from("scripts/persona_status.py"));

    if !resolved.exists() {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "success": false,
                "error": "Skrip persona_status.py tidak ditemukan di server",
            })),
        );
    }

    match tokio::process::Command::new("python3")
        .arg(&resolved)
        .arg("diag")
        .arg("--json")
        .output()
        .await
    {
        Ok(output) if output.status.success() => {
            let text = String::from_utf8_lossy(&output.stdout);
            match serde_json::from_str::<serde_json::Value>(&text) {
                Ok(val) => (
                    StatusCode::OK,
                    Json(json!({
                        "success": true,
                        "diagnostics": val,
                    })),
                ),
                Err(e) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({
                        "success": false,
                        "error": format!("Gagal mem-parsing output diagnostik JSON: {}", e),
                        "raw": text,
                    })),
                ),
            }
        }
        Ok(output) => {
            let err = String::from_utf8_lossy(&output.stderr);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "error": format!("Eksekusi diag gagal: {}", err),
                })),
            )
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "success": false,
                "error": format!("Gagal menjalankan proses python: {}", e),
            })),
        ),
    }
}

#[derive(Debug, Deserialize)]
pub struct PersonaJournalQuery {
    limit: Option<usize>,
}

pub async fn api_persona_journal_handler(
    Query(query): Query<PersonaJournalQuery>,
) -> impl IntoResponse {
    let limit = query.limit.unwrap_or(20);
    let script_candidates = [
        PathBuf::from("scripts/persona_status.py"),
        PathBuf::from("/app/scripts/persona_status.py"),
        PathBuf::from("/root/projects/aina/scripts/persona_status.py"),
    ];

    let resolved = script_candidates
        .into_iter()
        .find(|p| p.exists())
        .unwrap_or_else(|| PathBuf::from("scripts/persona_status.py"));

    if !resolved.exists() {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "success": false,
                "error": "Skrip persona_status.py tidak ditemukan di server",
            })),
        );
    }

    match tokio::process::Command::new("python3")
        .arg(&resolved)
        .arg("history")
        .arg("--limit")
        .arg(limit.to_string())
        .arg("--json")
        .output()
        .await
    {
        Ok(output) if output.status.success() => {
            let text = String::from_utf8_lossy(&output.stdout);
            match serde_json::from_str::<serde_json::Value>(&text) {
                Ok(val) => (
                    StatusCode::OK,
                    Json(json!({
                        "success": true,
                        "journal": val,
                    })),
                ),
                Err(e) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({
                        "success": false,
                        "error": format!("Gagal mem-parsing output journal JSON: {}", e),
                        "raw": text,
                    })),
                ),
            }
        }
        Ok(output) => {
            let err = String::from_utf8_lossy(&output.stderr);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "error": format!("Eksekusi journal gagal: {}", err),
                })),
            )
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "success": false,
                "error": format!("Gagal menjalankan proses python: {}", e),
            })),
        ),
    }
}

#[derive(Debug, Deserialize)]
pub struct MetacogQuery {
    pub domain: Option<String>,
    pub limit: Option<usize>,
}

pub async fn api_metacog_diagnostics_handler(
    State(state): State<Arc<WebhookServerState>>,
    Query(query): Query<MetacogQuery>,
) -> impl IntoResponse {
    let manifest = crate::core::domain::metacognition::AgentCapabilityManifest::default_manifest();
    let stats = state.session_store.get_metacognitive_calibration_stats().await.ok();
    let limit = query.limit.unwrap_or(10);
    let recent = state.session_store.list_metacognitive_predictions(limit, query.domain.as_deref()).await.unwrap_or_default();

    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "manifest": manifest,
            "calibration": stats,
            "recent_predictions": recent,
        })),
    )
}

pub async fn api_metacog_capabilities_handler() -> impl IntoResponse {
    let manifest = crate::core::domain::metacognition::AgentCapabilityManifest::default_manifest();
    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "capabilities": manifest,
        })),
    )
}

pub async fn api_metacog_calibration_handler(
    State(state): State<Arc<WebhookServerState>>,
    Query(query): Query<MetacogQuery>,
) -> impl IntoResponse {
    match state.session_store.get_metacognitive_calibration_stats().await {
        Ok(stats) => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "domain": query.domain,
                "calibration": stats,
            })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "success": false,
                "error": format!("Gagal memuat statistik kalibrasi metakognisi: {}", e),
            })),
        ),
    }
}

