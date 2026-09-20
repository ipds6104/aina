use crate::adapters::driving::web::state::WebhookServerState;
use crate::adapters::driving::web::ui::render_html;
use axum::{
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse},
};
use std::sync::Arc;

pub async fn health_handler() -> impl IntoResponse {
    (StatusCode::OK, "Aina is running smoothly")
}

pub async fn dashboard_or_setup_handler(
    State(state): State<Arc<WebhookServerState>>,
) -> impl IntoResponse {
    let is_auth = state.agent_engine.is_authenticated().await;
    let live_model = state.agent_engine.get_model().await;
    Html(render_html(is_auth, &state, &live_model))
}

pub async fn setup_page_handler(
    State(state): State<Arc<WebhookServerState>>,
) -> impl IntoResponse {
    let is_auth = state.agent_engine.is_authenticated().await;
    let live_model = state.agent_engine.get_model().await;
    Html(render_html(is_auth, &state, &live_model))
}
