use crate::adapters::driving::web::auth::{
    constant_time_str_eq, extract_client_ip, is_admin_session_or_key_authorized, is_api_authorized,
};
use crate::adapters::driving::web::state::WebhookServerState;
use crate::adapters::driving::web::ui::chrono_now_secs;
use axum::{
    extract::{ConnectInfo, Extension, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{error, info, warn};

const ADMIN_UNAUTHORIZED_MSG: &str = "Akses ditolak. Berikan Admin Key yang valid via header (X-Admin-Key / Authorization: Bearer) atau sesi SSO admin.";

fn too_many_requests_response(retry_after_secs: u64) -> Response {
    let mut resp_headers = HeaderMap::new();
    if let Ok(val) = retry_after_secs.to_string().parse() {
        resp_headers.insert(header::RETRY_AFTER, val);
    }
    (
        StatusCode::TOO_MANY_REQUESTS,
        resp_headers,
        Json(json!({
            "success": false,
            "error": format!("Terlalu banyak percobaan autentikasi gagal. Coba lagi dalam {} detik.", retry_after_secs),
            "retry_after_secs": retry_after_secs,
        })),
    )
        .into_response()
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
    pub version: String,
    pub git_commit: String,
    pub build_time: String,
    pub agy_cli: crate::core::domain::AgyCliPublicSummary,
}

fn resolve_agy_bin_for_web() -> PathBuf {
    std::env::var("AGENT_BINARY_PATH")
        .or_else(|_| std::env::var("AGY_BINARY_PATH"))
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/usr/local/bin/agy"))
}

pub async fn api_status_handler(
    State(state): State<Arc<WebhookServerState>>,
) -> impl IntoResponse {
    let auth = state.agent_engine.is_authenticated().await;
    let live_model = state.agent_engine.get_model().await;
    let companion_active = state.companion_jid.is_some();
    let agy_bin = resolve_agy_bin_for_web();
    let agy_report =
        crate::adapters::driven::agy_cli::AgyUpdater::get_health_report_cached(&agy_bin, false)
            .await;
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
        agy_cli: agy_report.to_public_summary(),
    })
}

pub async fn api_agy_status_handler(
    State(state): State<Arc<WebhookServerState>>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    if !is_api_authorized(&headers, &state) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "success": false, "error": ADMIN_UNAUTHORIZED_MSG })),
        )
            .into_response();
    }
    let force_refresh = query
        .get("refresh")
        .map(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "true" | "1" | "yes"))
        .unwrap_or(false);
    let agy_bin = resolve_agy_bin_for_web();
    let report = crate::adapters::driven::agy_cli::AgyUpdater::get_health_report_cached(
        &agy_bin,
        force_refresh,
    )
    .await;
    (StatusCode::OK, Json(json!({ "success": true, "agy_cli": report }))).into_response()
}

pub async fn api_version_handler() -> impl IntoResponse {
    let report = crate::core::domain::version::VersionEngine::check_upstream_status().await;
    Json(report)
}

/// A model entry in the dynamic /api/models response.
#[derive(Debug, Serialize)]
struct ModelOption {
    pub id: String,
    pub name: String,
    pub category: String,
}

#[derive(Debug, Serialize)]
struct ApiModelsResponse {
    pub current: String,
    pub available: Vec<ModelOption>,
    pub source: &'static str,
}

pub async fn api_get_models_handler(
    State(state): State<Arc<WebhookServerState>>,
) -> impl IntoResponse {
    let current = state.agent_engine.get_model().await;

    // Dynamically fetch from AGY CLI — falls back to the static catalog (with a
    // reliable source marker) if the CLI is unavailable.
    let discovery = crate::adapters::driven::fetch_models_from_agy_cli().await;
    let source = match discovery.source {
        crate::adapters::driven::ModelSource::Dynamic => "dynamic",
        crate::adapters::driven::ModelSource::StaticFallback => "static_fallback",
    };

    let available: Vec<ModelOption> = discovery
        .models
        .into_iter()
        .map(|e| ModelOption {
            id: e.id,
            name: e.name,
            category: e.category,
        })
        .collect();

    Json(ApiModelsResponse {
        current,
        available,
        source,
    })
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
    let authorized = is_admin_session_or_key_authorized(&headers, &state)
        || payload
            .admin_key
            .as_deref()
            .map(|k| constant_time_str_eq(k, &state.setup_code))
            .unwrap_or(false);

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
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "success": false,
                "error": format!("{}", e)
            })),
        ),
    }
}

#[derive(Debug, Deserialize)]
pub struct SetupRequest {
    pub token: String,
    pub setup_code: String,
}

pub async fn api_setup_handler(
    State(state): State<Arc<WebhookServerState>>,
    headers: HeaderMap,
    peer: Option<Extension<ConnectInfo<SocketAddr>>>,
    Json(payload): Json<SetupRequest>,
) -> Response {
    info!("Received token configuration request via /api/setup");

    let client_ip = extract_client_ip(
        &headers,
        peer.map(|Extension(ConnectInfo(addr))| addr),
        state.trust_proxy_headers,
    );
    let now = chrono_now_secs();

    if let Some(retry_after) = state.auth_attempt_limiter.check_locked(client_ip, now).await {
        warn!(
            "Rate-limited /api/setup request from {} (retry_after={}s)",
            client_ip, retry_after
        );
        return too_many_requests_response(retry_after);
    }

    // Fail-closed constant-time verification of Setup Code / Admin Key (or valid OIDC admin session)
    let authorized = is_admin_session_or_key_authorized(&headers, &state)
        || constant_time_str_eq(&payload.setup_code, &state.setup_code);

    if !authorized {
        if let Some(retry_after) = state
            .auth_attempt_limiter
            .record_failure(client_ip, now)
            .await
        {
            warn!(
                "IP {} locked out on /api/setup after repeated failures ({}s)",
                client_ip, retry_after
            );
            return too_many_requests_response(retry_after);
        }
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "success": false,
                "message": "Kode Setup / Admin Key salah! Masukkan kode yang tertera di log deployment server atau ADMIN_KEY Anda."
            })),
        )
            .into_response();
    }

    state.auth_attempt_limiter.record_success(client_ip).await;

    match state.agent_engine.save_auth_token(&payload.token).await {
        Ok(_) => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "message": "Token berhasil diverifikasi dan disimpan! Aina sekarang aktif."
            })),
        )
            .into_response(),
        Err(e) => {
            error!("Setup token verification failed: {}", e);
            (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "success": false,
                    "message": format!("Verifikasi token gagal: {}", e)
                })),
            )
                .into_response()
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
    headers: HeaderMap,
) -> impl IntoResponse {
    if !is_api_authorized(&headers, &state) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "success": false,
                "error": ADMIN_UNAUTHORIZED_MSG,
            })),
        );
    }

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
    Json(payload): Json<AddTokenApiRequest>,
) -> impl IntoResponse {
    let authorized = is_api_authorized(&headers, &state)
        || payload
            .setup_code
            .as_deref()
            .map(|k| constant_time_str_eq(k, &state.setup_code))
            .unwrap_or(false);

    if !authorized {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "success": false,
                "error": ADMIN_UNAUTHORIZED_MSG
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
    headers: HeaderMap,
    peer: Option<Extension<ConnectInfo<SocketAddr>>>,
    Json(payload): Json<VerifyAdminRequest>,
) -> Response {
    let client_ip = extract_client_ip(
        &headers,
        peer.map(|Extension(ConnectInfo(addr))| addr),
        state.trust_proxy_headers,
    );
    let now = chrono_now_secs();

    if let Some(retry_after) = state.auth_attempt_limiter.check_locked(client_ip, now).await {
        warn!(
            "Rate-limited /api/auth/verify request from {} (retry_after={}s)",
            client_ip, retry_after
        );
        return too_many_requests_response(retry_after);
    }

    // Fail-closed constant-time verification strictly against setup_code (gateway keys cannot grant admin)
    let is_valid = constant_time_str_eq(&payload.admin_key, &state.setup_code);

    if is_valid {
        state.auth_attempt_limiter.record_success(client_ip).await;
        (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "message": "Autentikasi admin berhasil!"
            })),
        )
            .into_response()
    } else {
        if let Some(retry_after) = state
            .auth_attempt_limiter
            .record_failure(client_ip, now)
            .await
        {
            warn!(
                "IP {} locked out on /api/auth/verify after repeated failures ({}s)",
                client_ip, retry_after
            );
            return too_many_requests_response(retry_after);
        }
        (
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "success": false,
                "message": "Admin Key / Setup Code salah! Periksa log server Anda."
            })),
        )
            .into_response()
    }
}

pub async fn api_remove_account_handler(
    State(state): State<Arc<WebhookServerState>>,
    headers: HeaderMap,
    axum::extract::Path(id): axum::extract::Path<usize>,
) -> impl IntoResponse {
    if !is_api_authorized(&headers, &state) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "success": false,
                "error": ADMIN_UNAUTHORIZED_MSG
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
) -> impl IntoResponse {
    if !is_api_authorized(&headers, &state) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "success": false,
                "error": ADMIN_UNAUTHORIZED_MSG
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
) -> impl IntoResponse {
    if !is_api_authorized(&headers, &state) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "success": false,
                "error": ADMIN_UNAUTHORIZED_MSG
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
    Json(payload): Json<OAuthExchangeRequest>,
) -> impl IntoResponse {
    let authorized = is_api_authorized(&headers, &state)
        || payload
            .setup_code
            .as_deref()
            .map(|k| constant_time_str_eq(k, &state.setup_code))
            .unwrap_or(false);

    if !authorized {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "success": false,
                "error": ADMIN_UNAUTHORIZED_MSG
            })),
        );
    }

    if !crate::adapters::driven::agy_cli::is_safe_session_id(&payload.session_id) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "success": false,
                "error": "Invalid session_id: must be 16-64 ASCII alphanumeric, '_' or '-' characters."
            })),
        );
    }

    match state
        .agent_engine
        .exchange_oauth_code(&payload.session_id, &payload.code)
        .await
    {
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
