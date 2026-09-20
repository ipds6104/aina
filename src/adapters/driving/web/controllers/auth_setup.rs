use crate::adapters::driving::web::auth::{is_admin_authorized, is_api_authorized};
use crate::adapters::driving::web::state::WebhookServerState;
use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use tracing::{error, info};

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
    pub version: String,
    pub git_commit: String,
    pub build_time: String,
}

pub async fn api_status_handler(
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
        version: crate::core::domain::version::CURRENT_VERSION.to_string(),
        git_commit: crate::core::domain::version::CURRENT_COMMIT.to_string(),
        build_time: crate::core::domain::version::BUILD_TIMESTAMP.to_string(),
    })
}

pub async fn api_version_handler() -> impl IntoResponse {
    let report = crate::core::domain::version::VersionEngine::check_upstream_status().await;
    Json(report)
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

pub async fn api_get_models_handler(
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
pub struct SetModelRequest {
    pub model: String,
    pub admin_key: Option<String>,
}

pub async fn api_set_model_handler(
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
pub struct SetupRequest {
    pub token: String,
    pub setup_code: String,
}

pub async fn api_setup_handler(
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
pub struct AddTokenApiRequest {
    pub token: String,
    pub setup_code: Option<String>,
}

pub async fn api_get_accounts_handler(
    State(state): State<Arc<WebhookServerState>>,
) -> impl IntoResponse {
    let accounts = state.agent_engine.get_account_pool_status().await;
    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "total_accounts": accounts.len(),
            "accounts": accounts,
        })),
    )
}

pub async fn api_add_token_handler(
    State(state): State<Arc<WebhookServerState>>,
    headers: HeaderMap,
    Query(query): Query<std::collections::HashMap<String, String>>,
    Json(payload): Json<AddTokenApiRequest>,
) -> impl IntoResponse {
    let key_candidate = query.get("key")
        .or_else(|| query.get("api_key"))
        .map(|s| s.as_str())
        .or(payload.setup_code.as_deref());

    if !is_api_authorized(&headers, key_candidate, &state) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "success": false,
                "error": "Akses ditolak. Berikan API Key atau Admin Key yang valid."
            })),
        );
    }

    match state.agent_engine.save_auth_token(&payload.token).await {
        Ok(_) => {
            let accounts = state.agent_engine.get_account_pool_status().await;
            (
                StatusCode::OK,
                Json(json!({
                    "success": true,
                    "message": "Token akun baru berhasil diverifikasi dan ditambahkan ke pool!",
                    "total_accounts": accounts.len(),
                    "accounts": accounts,
                })),
            )
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "success": false,
                "error": format!("Gagal memverifikasi token: {}", e),
            })),
        ),
    }
}

#[derive(Debug, Deserialize)]
pub struct VerifyAdminRequest {
    pub admin_key: String,
}

pub async fn api_verify_admin_handler(
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

pub async fn api_remove_account_handler(
    State(state): State<Arc<WebhookServerState>>,
    headers: HeaderMap,
    Query(query): Query<std::collections::HashMap<String, String>>,
    axum::extract::Path(id): axum::extract::Path<usize>,
) -> impl IntoResponse {
    let key_candidate = query.get("key")
        .or_else(|| query.get("api_key"))
        .map(|s| s.as_str());

    if !is_api_authorized(&headers, key_candidate, &state) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "success": false,
                "error": "Akses ditolak. Berikan API Key atau Admin Key yang valid."
            })),
        );
    }

    match state.agent_engine.remove_account(id).await {
        Ok(true) => {
            let accounts = state.agent_engine.get_account_pool_status().await;
            (
                StatusCode::OK,
                Json(json!({
                    "success": true,
                    "message": format!("Akun #{} berhasil dihapus dari pool.", id),
                    "total_accounts": accounts.len(),
                    "accounts": accounts,
                })),
            )
        }
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "success": false,
                "error": format!("Akun dengan ID #{} tidak ditemukan di pool.", id),
            })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "success": false,
                "error": format!("Gagal menghapus akun: {}", e),
            })),
        ),
    }
}

pub async fn api_clear_accounts_handler(
    State(state): State<Arc<WebhookServerState>>,
    headers: HeaderMap,
    Query(query): Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let key_candidate = query.get("key")
        .or_else(|| query.get("api_key"))
        .map(|s| s.as_str());

    if !is_api_authorized(&headers, key_candidate, &state) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "success": false,
                "error": "Akses ditolak. Berikan API Key atau Admin Key yang valid."
            })),
        );
    }

    match state.agent_engine.clear_account_pool().await {
        Ok(count) => {
            let accounts = state.agent_engine.get_account_pool_status().await;
            (
                StatusCode::OK,
                Json(json!({
                    "success": true,
                    "message": format!("Berhasil mengosongkan {} akun sekunder dari pool.", count),
                    "total_accounts": accounts.len(),
                    "accounts": accounts,
                })),
            )
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "success": false,
                "error": format!("Gagal mengosongkan pool akun: {}", e),
            })),
        ),
    }
}

#[derive(Debug, Deserialize)]
pub struct OAuthExchangeRequest {
    pub session_id: String,
    pub code: String,
    pub setup_code: Option<String>,
}

pub async fn api_oauth_init_handler(
    State(state): State<Arc<WebhookServerState>>,
    headers: HeaderMap,
    Query(query): Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let key_candidate = query.get("key")
        .or_else(|| query.get("api_key"))
        .map(|s| s.as_str());

    if !is_api_authorized(&headers, key_candidate, &state) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "success": false,
                "error": "Akses ditolak. Berikan API Key atau Admin Key yang valid."
            })),
        );
    }

    match state.agent_engine.init_oauth_session().await {
        Ok((session_id, auth_url)) => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "session_id": session_id,
                "auth_url": auth_url,
            })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "success": false,
                "error": format!("Gagal memulai sesi login Google: {}", e),
            })),
        ),
    }
}

pub async fn api_oauth_exchange_handler(
    State(state): State<Arc<WebhookServerState>>,
    headers: HeaderMap,
    Query(query): Query<std::collections::HashMap<String, String>>,
    Json(payload): Json<OAuthExchangeRequest>,
) -> impl IntoResponse {
    let key_candidate = query.get("key")
        .or_else(|| query.get("api_key"))
        .map(|s| s.as_str())
        .or(payload.setup_code.as_deref());

    if !is_api_authorized(&headers, key_candidate, &state) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "success": false,
                "error": "Akses ditolak. Berikan API Key atau Admin Key yang valid."
            })),
        );
    }

    match state.agent_engine.exchange_oauth_code(&payload.session_id, &payload.code).await {
        Ok(email) => {
            let accounts = state.agent_engine.get_account_pool_status().await;
            (
                StatusCode::OK,
                Json(json!({
                    "success": true,
                    "message": format!("Akun {} berhasil dihubungkan dan ditambahkan ke pool!", email),
                    "email": email,
                    "total_accounts": accounts.len(),
                    "accounts": accounts,
                })),
            )
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "success": false,
                "error": format!("{}", e),
            })),
        ),
    }
}
