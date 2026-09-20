use super::state::WebhookServerState;
use axum::http::HeaderMap;

pub fn is_api_authorized(
    headers: &HeaderMap,
    query_key: Option<&str>,
    state: &WebhookServerState,
) -> bool {
    let expected_code = state.setup_code.trim();
    let expected_whatsmeow = state.whatsmeow_api_key.trim();

    if expected_code.is_empty() && expected_whatsmeow.is_empty() {
        return true;
    }

    let is_match = |cand: &str| -> bool {
        let c = cand.trim();
        if c.is_empty() {
            return false;
        }
        (!expected_code.is_empty() && c == expected_code)
            || (!expected_whatsmeow.is_empty() && c == expected_whatsmeow)
            || state.companion_api_key.as_deref().map(|k| !k.trim().is_empty() && c == k.trim()).unwrap_or(false)
    };

    if let Some(key) = query_key {
        if is_match(key) {
            return true;
        }
    }

    if let Some(key) = headers.get("X-Admin-Key").and_then(|v| v.to_str().ok()) {
        if is_match(key) {
            return true;
        }
    }

    if let Some(key) = headers.get("X-API-Key").and_then(|v| v.to_str().ok()) {
        if is_match(key) {
            return true;
        }
    }

    if let Some(auth) = headers.get("Authorization").and_then(|v| v.to_str().ok()) {
        if let Some(bearer) = auth.strip_prefix("Bearer ") {
            if is_match(bearer) {
                return true;
            }
        }
    }

    false
}

pub fn is_admin_authorized(headers: &HeaderMap, expected_code: &str) -> bool {
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
