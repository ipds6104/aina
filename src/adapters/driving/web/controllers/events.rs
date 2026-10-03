use crate::adapters::driving::web::auth::is_events_authorized;
use crate::adapters::driving::web::queue::dispatch_message_to_queue;
use crate::adapters::driving::web::state::WebhookServerState;
use crate::core::domain::{ChatType, IncomingMessage, Platform, Sender, SessionRole};
use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{info, warn};

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct EventIngressPayload {
    /// Source system or tool generating the event (e.g. "sentry", "prometheus", "github-actions", "grafana", "argo", "datadog", "custom")
    pub source: Option<String>,
    /// Event category or name (e.g. "alert", "error_spike", "deployment", "incident", "build_failed", "pipeline_success")
    pub event: Option<String>,
    /// Severity level (e.g. "critical", "error", "warning", "info", "debug")
    pub severity: Option<String>,
    /// Target service or microservice name (e.g. "billing-service", "auth-api", "worker-queue")
    pub service: Option<String>,
    /// Target code repository (e.g. "organization/billing-service")
    pub repository: Option<String>,
    /// Short summary title or headline describing the event
    pub title: Option<String>,
    /// Detailed body, stacktrace, error logs, or markdown explanation
    pub details: Option<Value>,
    /// Contextual metadata, metrics, labels, or environment info
    pub metadata: Option<Value>,
    /// Optional target destination chat JID (e.g. "120363023456789@g.us" or "628123456789@s.whatsapp.net").
    /// Defaults to user companion JID or bot JID.
    pub target_chat: Option<String>,
    /// Recommended next steps or diagnostic suggestions for Aina
    pub suggested_actions: Option<Vec<String>>,
    /// Catch-all for any extra/arbitrary vendor fields
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

/// Ingress handler for `/api/v1/events` and `/api/events`.
/// Accepts context-agnostic event payloads from observability tools, CI/CD pipelines,
/// external webhooks, or scripts, formats an actionable investigation prompt,
/// dispatches it non-blockingly to Aina's processing queue, and returns HTTP 202 Accepted.
pub async fn events_ingress_handler(
    State(state): State<Arc<WebhookServerState>>,
    Query(query_params): Query<HashMap<String, String>>,
    headers: HeaderMap,
    Json(raw_payload): Json<Value>,
) -> impl IntoResponse {
    let query_key = query_params
        .get("api_key")
        .or_else(|| query_params.get("token"))
        .or_else(|| query_params.get("key"))
        .map(|s| s.as_str());

    if !is_events_authorized(&headers, query_key, &state) {
        warn!("Unauthorized access attempt to /api/v1/events");
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "status": "error",
                "error": "Akses ditolak. API Key untuk endpoint events tidak valid atau belum dikirimkan."
            })),
        );
    }

    // Deserialize into typed payload if possible, or fallback to default
    let payload: EventIngressPayload = serde_json::from_value(raw_payload.clone())
        .unwrap_or_default();

    let now_epoch = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let unique_suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_millis();

    let event_id = format!("evt_{}_{}", now_epoch, unique_suffix);

    // Resolve source, event name, severity, service, repo, title
    let source = payload
        .source
        .as_deref()
        .or_else(|| raw_payload.get("source").and_then(|v| v.as_str()))
        .unwrap_or("observability");

    let event_name = payload
        .event
        .as_deref()
        .or_else(|| raw_payload.get("event").and_then(|v| v.as_str()))
        .unwrap_or("alert");

    let severity = payload
        .severity
        .as_deref()
        .or_else(|| raw_payload.get("severity").and_then(|v| v.as_str()))
        .or_else(|| raw_payload.get("level").and_then(|v| v.as_str()))
        .unwrap_or("info");

    let service = payload
        .service
        .as_deref()
        .or_else(|| raw_payload.get("service").and_then(|v| v.as_str()))
        .or_else(|| raw_payload.get("app").and_then(|v| v.as_str()))
        .unwrap_or("unknown-service");

    let repository = payload
        .repository
        .as_deref()
        .or_else(|| raw_payload.get("repository").and_then(|v| v.as_str()))
        .or_else(|| raw_payload.get("repo").and_then(|v| v.as_str()))
        .unwrap_or("-");

    let title = payload
        .title
        .as_deref()
        .or_else(|| raw_payload.get("title").and_then(|v| v.as_str()))
        .or_else(|| raw_payload.get("summary").and_then(|v| v.as_str()))
        .or_else(|| raw_payload.get("message").and_then(|v| v.as_str()))
        .unwrap_or("Observability Event Triggered");

    // Format details
    let details_str = match &payload.details {
        Some(Value::String(s)) => s.clone(),
        Some(other) => serde_json::to_string_pretty(other).unwrap_or_default(),
        None => {
            if let Some(msg) = raw_payload.get("details").or_else(|| raw_payload.get("message")) {
                if let Some(s) = msg.as_str() {
                    s.to_string()
                } else {
                    serde_json::to_string_pretty(msg).unwrap_or_default()
                }
            } else if !payload.extra.is_empty() {
                serde_json::to_string_pretty(&payload.extra).unwrap_or_default()
            } else {
                "-".to_string()
            }
        }
    };

    // Format metadata
    let metadata_str = match &payload.metadata {
        Some(m) => serde_json::to_string_pretty(m).unwrap_or_else(|_| "-".to_string()),
        None => {
            if let Some(m) = raw_payload.get("metadata").or_else(|| raw_payload.get("labels")) {
                serde_json::to_string_pretty(m).unwrap_or_else(|_| "-".to_string())
            } else {
                "-".to_string()
            }
        }
    };

    // Format suggested actions
    let mut actions_section = String::new();
    if let Some(ref actions) = payload.suggested_actions {
        if !actions.is_empty() {
            actions_section.push_str("\n\n🛠️ *Tindakan Investigasi / Perbaikan yang Disarankan:*\n");
            for (idx, action) in actions.iter().enumerate() {
                actions_section.push_str(&format!("{}. {}\n", idx + 1, action));
            }
        }
    }

    let formatted_prompt = format!(
        "🚨 *[EVENT MASUK: {} / {}]*\n\
         • *Tingkat Keparahan:* {}\n\
         • *Layanan:* {}\n\
         • *Repositori:* {}\n\
         • *Judul:* {}\n\
         • *Event ID:* `{}`\n\
         • *Waktu:* <t:{}>\n\n\
         📝 *Rincian Masalah:*\n{}\n\n\
         🏷️ *Metadata & Konteks:*\n{}\
         {}\n\n\
         ---\n\
         💡 *Konteks & Arahan untuk Aina:*\n\
         Event di atas diterima via Ingress API `/api/v1/events`. Mohon lakukan analisis akar masalah (root cause analysis), periksa observability/log terkait, dan lakukan tindakan investigasi atau perbaikan yang relevan menggunakan tools/skills yang tersedia.",
        source.to_uppercase(),
        event_name.to_uppercase(),
        severity.to_uppercase(),
        service,
        repository,
        title,
        event_id,
        now_epoch,
        details_str,
        metadata_str,
        actions_section
    );

    // Resolve target chat destination
    let target_chat = payload
        .target_chat
        .filter(|c| !c.trim().is_empty())
        .or_else(|| query_params.get("chat_jid").cloned())
        .or_else(|| state.companion_jid.clone())
        .unwrap_or_else(|| {
            if !state.bot_jid.trim().is_empty() {
                state.bot_jid.clone()
            } else {
                "events@system".to_string()
            }
        });

    let chat_type = if target_chat.ends_with("@g.us") {
        ChatType::Group
    } else {
        ChatType::DirectMessage
    };

    let source_clean = if source.trim().is_empty() {
        "observability".to_string()
    } else {
        source.to_lowercase().replace(' ', "-")
    };

    let msg = IncomingMessage {
        id: event_id.clone(),
        platform: Platform::Custom("EventsIngress".to_string()),
        session_role: SessionRole::PrimaryBot,
        chat_jid: target_chat.clone(),
        chat_type,
        sender: Sender {
            jid: format!("{}@system.event", source_clean),
            name: Some(format!("Event: {}", source)),
        },
        text: formatted_prompt,
        timestamp: now_epoch as i64,
        is_from_me: false,
        quoted_message: None,
        mentioned_jids: vec![],
        is_bot_mentioned: true,
        bot_lid: state.bot_lid.clone(),
        has_media: false,
        media_type: None,
        media_path: None,
    };

    info!(
        "Enqueued incoming event '{}' (id: {}, source: {}, severity: {}, service: {}) to chat {}",
        title, event_id, source, severity, service, target_chat
    );

    dispatch_message_to_queue(&state, msg).await;

    (
        StatusCode::ACCEPTED,
        Json(json!({
            "status": "accepted",
            "event_id": event_id,
            "message": "Event berhasil diterima dan dimasukkan ke antrean proses Aina.",
            "source": source,
            "severity": severity,
            "service": service,
            "target_chat": target_chat
        })),
    )
}
