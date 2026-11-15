use crate::adapters::driving::web::state::WebhookServerState;
use crate::adapters::driving::web::ui::render_html;
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse},
};
use std::sync::Arc;

pub async fn health_handler() -> impl IntoResponse {
    (StatusCode::OK, "Aina is running smoothly")
}

pub async fn dashboard_or_setup_handler(
    State(state): State<Arc<WebhookServerState>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let secret = crate::adapters::driving::web::controllers::oidc::resolve_session_secret(&state);
    let session = crate::adapters::driving::web::controllers::oidc::extract_session_from_headers(&headers, &secret);

    // Strict Access Control: If SSO (OIDC) is enabled, this page can ONLY be accessed if already logged in!
    if state.oidc_config.enabled && session.is_none() {
        if !crate::adapters::driving::web::auth::is_api_authorized(&headers, None, &state) {
            return axum::response::Redirect::to("/auth/login").into_response();
        }
    }

    let is_auth = state.agent_engine.is_authenticated().await;
    let live_model = state.agent_engine.get_model().await;
    Html(render_html(is_auth, &state, &live_model, session.as_ref())).into_response()
}

pub async fn setup_page_handler(
    State(state): State<Arc<WebhookServerState>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let is_auth = state.agent_engine.is_authenticated().await;
    let live_model = state.agent_engine.get_model().await;
    let secret = crate::adapters::driving::web::controllers::oidc::resolve_session_secret(&state);
    let session = crate::adapters::driving::web::controllers::oidc::extract_session_from_headers(&headers, &secret);
    Html(render_html(is_auth, &state, &live_model, session.as_ref()))
}
