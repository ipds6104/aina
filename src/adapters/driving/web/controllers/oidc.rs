use crate::adapters::driving::web::state::{
    AuthenticatedUserSession, OidcPendingState, WebhookServerState,
};
use axum::{
    extract::{Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{Html, IntoResponse, Response},
    Json,
};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use tracing::{error, info, warn};

pub const OIDC_SESSION_COOKIE: &str = "aina_session";
pub const OIDC_STATE_COOKIE: &str = "aina_oidc_state";
pub const DEFAULT_SESSION_TTL_SECS: i64 = 7 * 24 * 3600; // 7 days

#[derive(Debug, Deserialize)]
pub struct CallbackQuery {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
    pub error_description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    #[serde(default)]
    pub token_type: Option<String>,
    #[serde(default)]
    pub expires_in: Option<i64>,
    #[serde(default)]
    pub id_token: Option<String>,
    #[serde(default)]
    pub refresh_token: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct MeResponse {
    pub authenticated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<AuthenticatedUserSession>,
}

/// Compute HMAC-SHA256 (RFC 2104) without external dependencies
pub fn hmac_sha256(key: &[u8], message: &[u8]) -> [u8; 32] {
    let mut k = [0u8; 64];
    if key.len() > 64 {
        let hash = Sha256::digest(key);
        k[..32].copy_from_slice(&hash);
    } else {
        k[..key.len()].copy_from_slice(key);
    }

    let mut ipad = [0x36u8; 64];
    let mut opad = [0x5cu8; 64];
    for i in 0..64 {
        ipad[i] ^= k[i];
        opad[i] ^= k[i];
    }

    let mut inner = Sha256::new();
    inner.update(&ipad);
    inner.update(message);
    let inner_hash = inner.finalize();

    let mut outer = Sha256::new();
    outer.update(&opad);
    outer.update(&inner_hash);
    outer.finalize().into()
}

/// Constant-time byte slice comparison to prevent timing attacks
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Sign a session struct into a tamper-evident session token
pub fn sign_session(session: &AuthenticatedUserSession, secret: &str) -> String {
    let json_bytes = serde_json::to_vec(session).unwrap_or_default();
    let payload_b64 = URL_SAFE_NO_PAD.encode(&json_bytes);
    let sig = hmac_sha256(secret.as_bytes(), payload_b64.as_bytes());
    let sig_b64 = URL_SAFE_NO_PAD.encode(sig);
    format!("{}.{}", payload_b64, sig_b64)
}

/// Verify and parse a signed session token
pub fn verify_session(token: &str, secret: &str) -> Option<AuthenticatedUserSession> {
    let (payload_b64, sig_b64) = token.split_once('.')?;
    let expected_sig = hmac_sha256(secret.as_bytes(), payload_b64.as_bytes());
    let provided_sig = URL_SAFE_NO_PAD.decode(sig_b64).ok()?;

    if !constant_time_eq(&expected_sig, &provided_sig) {
        return None;
    }

    let json_bytes = URL_SAFE_NO_PAD.decode(payload_b64).ok()?;
    let session: AuthenticatedUserSession = serde_json::from_slice(&json_bytes).ok()?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    if session.exp <= now {
        return None;
    }

    Some(session)
}

/// Extract authenticated user session from HTTP request headers (Cookie)
pub fn extract_session_from_headers(
    headers: &HeaderMap,
    secret: &str,
) -> Option<AuthenticatedUserSession> {
    let cookie_header = headers.get(header::COOKIE)?.to_str().ok()?;
    for part in cookie_header.split(';') {
        let trimmed = part.trim();
        if let Some(val) = trimmed.strip_prefix(&format!("{}=", OIDC_SESSION_COOKIE)) {
            if let Some(session) = verify_session(val, secret) {
                return Some(session);
            }
        }
    }
    None
}

/// Resolve the active session secret for HMAC signing
pub fn resolve_session_secret(state: &WebhookServerState) -> String {
    if let Some(ref sec) = state.oidc_config.session_secret {
        if !sec.trim().is_empty() {
            return sec.trim().to_string();
        }
    }
    // Deterministic fallback derived from setup_code and whatsmeow_api_key
    format!("{}:{}", state.setup_code, state.whatsmeow_api_key)
}

/// Clean up expired pending OIDC states (> 10 minutes)
async fn cleanup_expired_states(state: &WebhookServerState) {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let mut lock = state.oidc_states.write().await;
    lock.retain(|_, v| now - v.created_at < 600);
}

/// Handler: GET /auth/login
pub async fn oidc_login_handler(
    State(state): State<Arc<WebhookServerState>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if !state.oidc_config.enabled {
        return (
            StatusCode::NOT_FOUND,
            "Single Sign-On (OIDC) is not enabled on this instance.",
        )
            .into_response();
    }

    let issuer_url = match &state.oidc_config.issuer_url {
        Some(url) if !url.trim().is_empty() => url.trim().trim_end_matches('/'),
        _ => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "OIDC_ISSUER_URL is not configured.",
            )
                .into_response()
        }
    };

    let client_id = match &state.oidc_config.client_id {
        Some(cid) if !cid.trim().is_empty() => cid.trim(),
        _ => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "OIDC_CLIENT_ID is not configured.",
            )
                .into_response()
        }
    };

    cleanup_expired_states(&state).await;

    // Generate cryptographic random state (16 bytes = 22 base64url chars)
    let state_bytes: [u8; 16] = rand::random();
    let state_param = URL_SAFE_NO_PAD.encode(state_bytes);

    // Generate cryptographic PKCE code_verifier (32 bytes = 43 base64url chars)
    let verifier_bytes: [u8; 32] = rand::random();
    let code_verifier = URL_SAFE_NO_PAD.encode(verifier_bytes);

    // Compute S256 code_challenge
    let code_challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(code_verifier.as_bytes()));

    // Resolve redirect URI (configured or derived from request Host)
    let redirect_uri = if let Some(ref r_uri) = state.oidc_config.redirect_uri {
        r_uri.trim().to_string()
    } else {
        let host = headers
            .get(header::HOST)
            .and_then(|h| h.to_str().ok())
            .unwrap_or("localhost:8090");
        let proto = headers
            .get("x-forwarded-proto")
            .and_then(|p| p.to_str().ok())
            .unwrap_or("http");
        format!("{}://{}/auth/callback", proto, host)
    };

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    // Store state in memory table
    {
        let mut lock = state.oidc_states.write().await;
        lock.insert(
            state_param.clone(),
            OidcPendingState {
                code_verifier: code_verifier.clone(),
                created_at: now,
                return_to: Some("/".to_string()),
            },
        );
    }

    // Build standard OIDC Authorization URL (RFC 6749 + RFC 7636 PKCE)
    let auth_url = format!(
        "{}/authorize?response_type=code&client_id={}&redirect_uri={}&scope={}&state={}&code_challenge={}&code_challenge_method=S256",
        issuer_url,
        urlencoding(&client_id),
        urlencoding(&redirect_uri),
        urlencoding(&state.oidc_config.scopes),
        urlencoding(&state_param),
        urlencoding(&code_challenge),
    );

    let state_cookie = format!(
        "{}={}; Path=/auth; HttpOnly; SameSite=Lax; Max-Age=600",
        OIDC_STATE_COOKIE, state_param
    );

    let mut response_headers = HeaderMap::new();
    if let Ok(cookie_val) = state_cookie.parse() {
        response_headers.insert(header::SET_COOKIE, cookie_val);
    }
    if let Ok(loc) = auth_url.parse() {
        response_headers.insert(header::LOCATION, loc);
    }

    (StatusCode::SEE_OTHER, response_headers).into_response()
}

/// Handler: GET /auth/callback
pub async fn oidc_callback_handler(
    State(state): State<Arc<WebhookServerState>>,
    headers: HeaderMap,
    Query(query): Query<CallbackQuery>,
) -> Response {
    if !state.oidc_config.enabled {
        return (
            StatusCode::NOT_FOUND,
            "Single Sign-On (OIDC) is not enabled on this instance.",
        )
            .into_response();
    }

    if let Some(err) = query.error {
        let desc = query.error_description.unwrap_or_else(|| err.clone());
        warn!("OIDC IdP returned authorization error: {} ({})", err, desc);
        return Html(render_auth_error_card(
            &state.oidc_config.provider_name,
            &format!("Autentikasi ditolak oleh server IdP: {}", desc),
        ))
        .into_response();
    }

    let code = match query.code {
        Some(c) if !c.trim().is_empty() => c,
        _ => {
            return Html(render_auth_error_card(
                &state.oidc_config.provider_name,
                "Kode otorisasi (code) tidak ditemukan dalam URL callback.",
            ))
            .into_response();
        }
    };

    let state_param = match query.state {
        Some(s) if !s.trim().is_empty() => s,
        _ => {
            return Html(render_auth_error_card(
                &state.oidc_config.provider_name,
                "Parameter state tidak ditemukan dalam URL callback.",
            ))
            .into_response();
        }
    };

    // Retrieve and remove pending state
    let pending_state = {
        let mut lock = state.oidc_states.write().await;
        lock.remove(&state_param)
    };

    let pending = match pending_state {
        Some(p) => p,
        None => {
            return Html(render_auth_error_card(
                &state.oidc_config.provider_name,
                "Sesi login telah kedaluwarsa atau parameter state tidak valid. Silakan ulangi proses masuk.",
            ))
            .into_response();
        }
    };

    let issuer_url = match &state.oidc_config.issuer_url {
        Some(url) => url.trim().trim_end_matches('/'),
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "OIDC_ISSUER_URL is missing.",
            )
                .into_response()
        }
    };

    let client_id = match &state.oidc_config.client_id {
        Some(cid) => cid.trim(),
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "OIDC_CLIENT_ID is missing.",
            )
                .into_response()
        }
    };

    let redirect_uri = if let Some(ref r_uri) = state.oidc_config.redirect_uri {
        r_uri.trim().to_string()
    } else {
        let host = headers
            .get(header::HOST)
            .and_then(|h| h.to_str().ok())
            .unwrap_or("localhost:8090");
        let proto = headers
            .get("x-forwarded-proto")
            .and_then(|p| p.to_str().ok())
            .unwrap_or("http");
        format!("{}://{}/auth/callback", proto, host)
    };

    // 1. Exchange authorization code for tokens at IdP token endpoint
    let token_endpoint = format!("{}/token", issuer_url);
    let http_client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .unwrap_or_default();

    let mut token_params = vec![
        ("grant_type", "authorization_code".to_string()),
        ("code", code),
        ("redirect_uri", redirect_uri),
        ("client_id", client_id.to_string()),
        ("code_verifier", pending.code_verifier),
    ];

    if let Some(ref sec) = state.oidc_config.client_secret {
        if !sec.trim().is_empty() {
            token_params.push(("client_secret", sec.trim().to_string()));
        }
    }

    let form_body = token_params
        .iter()
        .map(|(k, v)| format!("{}={}", urlencoding(k), urlencoding(v)))
        .collect::<Vec<_>>()
        .join("&");

    let token_resp = match http_client
        .post(&token_endpoint)
        .header(
            header::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .body(form_body)
        .send()
        .await
    {
        Ok(resp) => resp,
        Err(e) => {
            error!("Failed to reach OIDC token endpoint {}: {}", token_endpoint, e);
            return Html(render_auth_error_card(
                &state.oidc_config.provider_name,
                &format!("Gagal menghubungi token endpoint IdP: {}", e),
            ))
            .into_response();
        }
    };

    if !token_resp.status().is_success() {
        let status = token_resp.status();
        let body = token_resp.text().await.unwrap_or_default();
        error!(
            "OIDC token endpoint returned status {}: {}",
            status, body
        );
        return Html(render_auth_error_card(
            &state.oidc_config.provider_name,
            &format!("Pertukaran kode otorisasi gagal (HTTP {}): {}", status, body),
        ))
        .into_response();
    }

    let token_data: TokenResponse = match token_resp.json().await {
        Ok(t) => t,
        Err(e) => {
            error!("Failed to deserialize OIDC token response: {}", e);
            return Html(render_auth_error_card(
                &state.oidc_config.provider_name,
                "Format data token dari IdP tidak valid.",
            ))
            .into_response();
        }
    };

    // 2. Fetch user profile from IdP UserInfo endpoint
    let userinfo_endpoint = format!("{}/userinfo", issuer_url);
    let userinfo_resp = match http_client
        .get(&userinfo_endpoint)
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", token_data.access_token),
        )
        .send()
        .await
    {
        Ok(resp) => resp,
        Err(e) => {
            error!("Failed to call OIDC userinfo endpoint {}: {}", userinfo_endpoint, e);
            return Html(render_auth_error_card(
                &state.oidc_config.provider_name,
                &format!("Gagal mengambil profil userinfo dari IdP: {}", e),
            ))
            .into_response();
        }
    };

    if !userinfo_resp.status().is_success() {
        let status = userinfo_resp.status();
        let body = userinfo_resp.text().await.unwrap_or_default();
        error!("OIDC userinfo returned status {}: {}", status, body);
        return Html(render_auth_error_card(
            &state.oidc_config.provider_name,
            &format!("Gagal mengambil data user (HTTP {}): {}", status, body),
        ))
        .into_response();
    }

    let claims: serde_json::Value = match userinfo_resp.json().await {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to deserialize userinfo claims JSON: {}", e);
            return Html(render_auth_error_card(
                &state.oidc_config.provider_name,
                "Format data userinfo tidak dapat dibaca.",
            ))
            .into_response();
        }
    };

    // Extract user identity claims (agnostic extraction with standard + custom attribute fallbacks)
    let sub = claims
        .get("sub")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    let name = claims
        .get("name")
        .and_then(|v| v.as_str())
        .or_else(|| claims.get("preferred_username").and_then(|v| v.as_str()))
        .unwrap_or(&sub)
        .to_string();

    let email = claims
        .get("email")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let preferred_username = claims
        .get("preferred_username")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let nip = claims
        .get("nip")
        .and_then(|v| v.as_str())
        .or_else(|| {
            claims
                .get("attributes")
                .and_then(|a| a.get("nip"))
                .and_then(|v| v.as_str())
        })
        .map(|s| s.to_string());

    let unit_kerja = claims
        .get("unit_kerja")
        .and_then(|v| v.as_str())
        .or_else(|| {
            claims
                .get("attributes")
                .and_then(|a| a.get("unit_kerja"))
                .and_then(|v| v.as_str())
        })
        .map(|s| s.to_string());

    let role = claims
        .get("role")
        .and_then(|v| v.as_str())
        .or_else(|| {
            claims
                .get("attributes")
                .and_then(|a| a.get("role"))
                .and_then(|v| v.as_str())
        })
        .map(|s| s.to_string());

    // Optional allowed domains check
    if let Some(ref allowed_domains) = state.oidc_config.allowed_domains {
        if !allowed_domains.is_empty() {
            let user_email = email.as_deref().unwrap_or("");
            let domain_matched = allowed_domains
                .iter()
                .any(|d| user_email.ends_with(&format!("@{}", d.trim())));
            if !domain_matched {
                warn!(
                    "User {} ({}) rejected: domain not in allowed_domains {:?}",
                    name, user_email, allowed_domains
                );
                return Html(render_auth_error_card(
                    &state.oidc_config.provider_name,
                    &format!(
                        "Akses dibatasi. Domain email akun Anda ({}) tidak diizinkan masuk ke sistem Aina.",
                        user_email
                    ),
                ))
                .into_response();
            }
        }
    }

    // Determine admin authorization (agnostic matching against configured admin_users list or admin role)
    let is_admin = {
        let mut matched = false;
        if let Some(ref admins) = state.oidc_config.admin_users {
            for admin_ident in admins {
                let ai = admin_ident.trim();
                if ai.is_empty() {
                    continue;
                }
                if let Some(ref n) = nip {
                    if n == ai {
                        matched = true;
                        break;
                    }
                }
                if let Some(ref e) = email {
                    if e.eq_ignore_ascii_case(ai) {
                        matched = true;
                        break;
                    }
                }
                if let Some(ref u) = preferred_username {
                    if u.eq_ignore_ascii_case(ai) {
                        matched = true;
                        break;
                    }
                }
                if sub == ai {
                    matched = true;
                    break;
                }
            }
        }
        if !matched {
            if let Some(ref r) = role {
                if r.eq_ignore_ascii_case("admin") || r.eq_ignore_ascii_case("superadmin") {
                    matched = true;
                }
            }
        }
        matched
    };

    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let session = AuthenticatedUserSession {
        sub,
        name: name.clone(),
        email,
        preferred_username,
        nip,
        unit_kerja,
        role,
        is_admin,
        exp: now_secs + DEFAULT_SESSION_TTL_SECS,
    };

    let secret = resolve_session_secret(&state);
    let session_token = sign_session(&session, &secret);

    info!(
        "SSO login successful for user '{}' (admin={}) via {}",
        name, is_admin, state.oidc_config.provider_name
    );

    let session_cookie = format!(
        "{}={}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}",
        OIDC_SESSION_COOKIE, session_token, DEFAULT_SESSION_TTL_SECS
    );

    let clear_state_cookie = format!(
        "{}=; Path=/auth; HttpOnly; SameSite=Lax; Max-Age=0",
        OIDC_STATE_COOKIE
    );

    let mut response_headers = HeaderMap::new();
    if let Ok(c1) = session_cookie.parse() {
        response_headers.append(header::SET_COOKIE, c1);
    }
    if let Ok(c2) = clear_state_cookie.parse() {
        response_headers.append(header::SET_COOKIE, c2);
    }
    if let Ok(loc) = "/".parse() {
        response_headers.insert(header::LOCATION, loc);
    }

    (StatusCode::SEE_OTHER, response_headers).into_response()
}

/// Handler: GET /auth/me
pub async fn oidc_me_handler(
    State(state): State<Arc<WebhookServerState>>,
    headers: HeaderMap,
) -> Json<MeResponse> {
    let secret = resolve_session_secret(&state);
    if let Some(session) = extract_session_from_headers(&headers, &secret) {
        Json(MeResponse {
            authenticated: true,
            user: Some(session),
        })
    } else {
        Json(MeResponse {
            authenticated: false,
            user: None,
        })
    }
}

/// Handler: GET /auth/logout
pub async fn oidc_logout_handler(
    State(state): State<Arc<WebhookServerState>>,
) -> impl IntoResponse {
    let clear_session_cookie = format!(
        "{}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0",
        OIDC_SESSION_COOKIE
    );

    let logout_target = if let Some(ref issuer) = state.oidc_config.issuer_url {
        let base = issuer.trim().trim_end_matches('/');
        let app_url = state
            .oidc_config
            .redirect_uri
            .as_deref()
            .and_then(|u| u.strip_suffix("/auth/callback"))
            .unwrap_or("/");
        format!(
            "{}/end_session?post_logout_redirect_uri={}",
            base,
            urlencoding(app_url)
        )
    } else {
        "/".to_string()
    };

    let mut headers = HeaderMap::new();
    if let Ok(c) = clear_session_cookie.parse() {
        headers.append(header::SET_COOKIE, c);
    }
    if let Ok(loc) = logout_target.parse() {
        headers.insert(header::LOCATION, loc);
    }

    (StatusCode::SEE_OTHER, headers)
}

fn urlencoding(input: &str) -> String {
    let mut encoded = String::with_capacity(input.len());
    for b in input.bytes() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(b as char)
            }
            _ => encoded.push_str(&format!("%{:02X}", b)),
        }
    }
    encoded
}

fn render_auth_error_card(provider_name: &str, error_message: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="id">
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Autentikasi Gagal - Aina</title>
    <style>
        body {{
            background: #0f172a;
            color: #f8fafc;
            font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
            display: flex;
            align-items: center;
            justify-content: center;
            min-height: 100vh;
            margin: 0;
            padding: 20px;
        }}
        .card {{
            background: #1e293b;
            border: 1px solid #ef4444;
            border-radius: 12px;
            padding: 28px;
            max-width: 480px;
            width: 100%;
            box-shadow: 0 10px 25px rgba(0,0,0,0.5);
            text-align: center;
        }}
        .icon {{
            font-size: 2.5rem;
            margin-bottom: 12px;
        }}
        h2 {{
            color: #f87171;
            margin: 0 0 12px 0;
            font-size: 1.25rem;
        }}
        p {{
            color: #cbd5e1;
            font-size: 0.95rem;
            line-height: 1.5;
            margin: 0 0 20px 0;
            word-break: break-word;
        }}
        .btn {{
            display: inline-block;
            background: #3b82f6;
            color: #fff;
            text-decoration: none;
            padding: 10px 20px;
            border-radius: 6px;
            font-size: 0.9rem;
            font-weight: 600;
            transition: background 0.2s;
        }}
        .btn:hover {{
            background: #2563eb;
        }}
    </style>
</head>
<body>
    <div class="card">
        <div class="icon" style="color: #ef4444; display: flex; justify-content: center; margin-bottom: 16px;">
            <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/></svg>
        </div>
        <h2>Gagal Masuk via {provider_name}</h2>
        <p>{error_message}</p>
        <a href="/" class="btn">Kembali ke Beranda Aina</a>
    </div>
</body>
</html>"#,
        provider_name = html_escape(provider_name),
        error_message = html_escape(error_message),
    )
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hmac_sha256_standard_vector() {
        let key = b"my-secret-key-12345";
        let message = b"hello world from aina oidc";
        let sig = hmac_sha256(key, message);
        let sig_hex = sig
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect::<Vec<_>>()
            .join("");
        assert_eq!(
            sig_hex,
            "d7e66d4827b8c64d44e4d77a983cb493501028a31555c6ace594b400f154144c"
        );
    }

    #[test]
    fn test_sign_and_verify_session_roundtrip() {
        let secret = "super-secret-production-signing-key-42";
        let session = AuthenticatedUserSession {
            sub: "usr-0199283".to_string(),
            name: "Ihza Karunia".to_string(),
            email: Some("ihzakarunia@bps.go.id".to_string()),
            preferred_username: Some("ihzakarunia".to_string()),
            nip: Some("199910282023021005".to_string()),
            unit_kerja: Some("BPS Kabupaten Mempawah".to_string()),
            role: Some("pegawai".to_string()),
            is_admin: true,
            exp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64
                + 3600,
        };

        let token = sign_session(&session, secret);
        assert!(!token.is_empty());
        assert!(token.contains('.'));

        let verified = verify_session(&token, secret).expect("session should be valid");
        assert_eq!(verified.sub, "usr-0199283");
        assert_eq!(verified.name, "Ihza Karunia");
        assert_eq!(verified.nip.as_deref(), Some("199910282023021005"));
        assert_eq!(verified.unit_kerja.as_deref(), Some("BPS Kabupaten Mempawah"));
        assert!(verified.is_admin);

        // Tampered token must fail
        let tampered = format!("{}x", token);
        assert!(verify_session(&tampered, secret).is_none());

        // Wrong secret must fail
        assert!(verify_session(&token, "wrong-secret").is_none());
    }

    #[test]
    fn test_expired_session_fails() {
        let secret = "test-secret";
        let session = AuthenticatedUserSession {
            sub: "usr-expired".to_string(),
            name: "Expired User".to_string(),
            email: None,
            preferred_username: None,
            nip: None,
            unit_kerja: None,
            role: None,
            is_admin: false,
            exp: 1000, // Long expired epoch
        };

        let token = sign_session(&session, secret);
        assert!(verify_session(&token, secret).is_none());
    }
}
