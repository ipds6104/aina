pub mod auth;
pub mod controllers;
pub mod ingress;
pub mod queue;
pub mod state;
pub mod ui;

#[allow(unused_imports)]
pub use auth::*;
pub use controllers::*;
#[allow(unused_imports)]
pub use ingress::*;
#[allow(unused_imports)]
pub use queue::*;
pub use state::*;
#[allow(unused_imports)]
pub use ui::*;

use axum::{
    extract::DefaultBodyLimit,
    routing::{get, post},
    Router,
};
use std::sync::Arc;

pub fn create_router(state: Arc<WebhookServerState>) -> Router {
    Router::new()
        .route("/", get(dashboard_or_setup_handler))
        .route("/setup", get(setup_page_handler))
        .route("/health", get(health_handler))
        .route("/api/status", get(api_status_handler))
        .route("/api/version", get(api_version_handler))
        .route("/api/models", get(api_get_models_handler))
        .route("/api/model", post(api_set_model_handler))
        .route("/api/setup", post(api_setup_handler))
        .route("/api/auth/accounts", get(api_get_accounts_handler))
        .route("/api/auth/token", post(api_add_token_handler))
        .route("/api/auth/verify", post(api_verify_admin_handler))
        .route("/api/simulate", post(simulate_handler))
        .route("/api/simulate/reset", post(simulate_reset_handler))
        .route("/api/simulate/job/{id}", get(simulate_job_status_handler))
        .route("/api/schedule/tasks", get(api_schedule_tasks_handler))
        .route("/api/schedule/runs", get(api_schedule_runs_handler))
        .route("/api/schedule/diagnostics", get(api_schedule_diagnostics_handler))
        .route("/api/persona/diagnostics", get(api_persona_diagnostics_handler))
        .route("/api/persona/journal", get(api_persona_journal_handler))
        .route("/api/metacognition/diagnostics", get(api_metacog_diagnostics_handler))
        .route("/api/metacognition/capabilities", get(api_metacog_capabilities_handler))
        .route("/api/metacognition/calibration", get(api_metacog_calibration_handler))
        .route("/api/audit/actions", get(api_audit_actions_handler))
        .route("/api/audit/actions/{id}", get(api_audit_action_detail_handler))
        .route("/api/audit/summary", get(api_audit_summary_handler))
        .route("/api/audit/diagnostics", get(api_audit_diagnostics_handler))
        .route("/api/audit/transcripts", get(api_audit_transcripts_handler))
        .route("/api/audit/presence", get(api_audit_presence_handler))
        .route("/api/audit/presence/stop", post(api_audit_presence_stop_handler))
        .route("/webhook", post(webhook_handler))
        .layer(DefaultBodyLimit::max(100 * 1024 * 1024))
        .with_state(state)
}


#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::Path;
    use axum::http::{HeaderMap, StatusCode};
    use crate::core::domain::{ChatType, IncomingMessage, PersonaEngine, Platform, Sender, SessionRole};
    use crate::core::usecases::ProcessIncomingMessageUseCase;
    use serde_json::json;
    use std::collections::HashMap;
    use tokio::sync::RwLock;

    #[test]
    fn test_parse_whatsmeow_unassigned_bot_defaults_to_companion() {
        let payload = json!({
            "from": "628111222333@s.whatsapp.net",
            "body": "Halo, ini pesan dari rekan kerja",
            "id": "MSG_TEST_001",
            "is_from_me": false
        });

        // Bot is unassigned, but companion is active
        let msg = parse_whatsmeow_message(
            &payload,
            None,
            Some("unassigned@s.whatsapp.net"),
            Some("628999888777@s.whatsapp.net"),
            Some("default"),
        )
        .expect("Message should be parsed");

        assert_eq!(msg.session_role, SessionRole::UserCompanion);
        assert_eq!(msg.text, "Halo, ini pesan dari rekan kerja");
    }

    #[test]
    fn test_parse_whatsmeow_query_params_role() {
        let payload = json!({
            "from": "1203630123456789@g.us",
            "body": "!aina tolong rangkum",
            "id": "MSG_TEST_002",
            "is_from_me": false
        });

        let mut query_params = HashMap::new();
        query_params.insert("role".to_string(), "companion".to_string());

        let msg = parse_whatsmeow_message(
            &payload,
            Some(&query_params),
            Some("628123456789@s.whatsapp.net"),
            Some("628999888777@s.whatsapp.net"),
            Some("default"),
        )
        .expect("Message should be parsed");

        assert_eq!(msg.session_role, SessionRole::UserCompanion);
    }

    #[test]
    fn test_parse_whatsmeow_session_id_matching() {
        let payload = json!({
            "session_id": "companion_office",
            "from": "628999888777@s.whatsapp.net",
            "body": "Catatan tugas",
            "id": "MSG_TEST_003",
            "is_from_me": true
        });

        let msg = parse_whatsmeow_message(
            &payload,
            None,
            Some("628123456789@s.whatsapp.net"),
            Some("628999888777@s.whatsapp.net"),
            Some("companion_office"),
        )
        .expect("Message should be parsed");

        assert_eq!(msg.session_role, SessionRole::UserCompanion);
    }

    #[tokio::test]
    async fn test_dispatch_incoming_message_to_queue_fifo_ordering() {
        let chat_queues = Arc::new(tokio::sync::Mutex::new(HashMap::new()));
        let processed = Arc::new(tokio::sync::Mutex::new(Vec::new()));

        let make_msg = |id: &str, text: &str| IncomingMessage {
            id: id.to_string(),
            platform: Platform::WhatsApp,
            session_role: SessionRole::PrimaryBot,
            chat_jid: "group123@g.us".to_string(),
            chat_type: ChatType::Group,
            sender: Sender {
                jid: "user456@s.whatsapp.net".to_string(),
                name: Some("Owner".to_string()),
            },
            text: text.to_string(),
            timestamp: 123456,
            is_from_me: false,
            quoted_message: None,
            mentioned_jids: vec![],
            is_bot_mentioned: true,
            bot_lid: None,
            has_media: false,
            media_type: None,
            media_path: None,
        };

        let p1 = Arc::clone(&processed);
        dispatch_incoming_message_to_queue(&chat_queues, make_msg("M1", "First"), 1, 0, move |msg| {
            let p_inner = Arc::clone(&p1);
            async move {
                tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                p_inner.lock().await.push(msg.id);
                Ok(())
            }
        })
        .await;

        let p2 = Arc::clone(&processed);
        dispatch_incoming_message_to_queue(&chat_queues, make_msg("M2", "Second"), 1, 0, move |msg| {
            let p_inner = Arc::clone(&p2);
            async move {
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                p_inner.lock().await.push(msg.id);
                Ok(())
            }
        })
        .await;

        let p3 = Arc::clone(&processed);
        dispatch_incoming_message_to_queue(&chat_queues, make_msg("M3", "Third"), 1, 0, move |msg| {
            let p_inner = Arc::clone(&p3);
            async move {
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                p_inner.lock().await.push(msg.id);
                Ok(())
            }
        })
        .await;

        let start = std::time::Instant::now();
        loop {
            let count = processed.lock().await.len();
            if count == 3 || start.elapsed() > std::time::Duration::from_secs(3) {
                break;
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
        }

        let results = processed.lock().await.clone();
        assert_eq!(results, vec!["M1", "M2", "M3"]);

        // Verify worker cleaned up from chat_queues after idle timeout
        let cleanup_start = std::time::Instant::now();
        loop {
            let queues = chat_queues.lock().await;
            if !queues.contains_key("group123@g.us") || cleanup_start.elapsed() > std::time::Duration::from_secs(6) {
                break;
            }
            drop(queues);
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        }
        let queues = chat_queues.lock().await;
        assert!(!queues.contains_key("group123@g.us"));
    }

    #[tokio::test]
    async fn test_dispatch_incoming_message_to_queue_debouncing() {
        let chat_queues = Arc::new(tokio::sync::Mutex::new(HashMap::new()));
        let aggregated_results = Arc::new(tokio::sync::Mutex::new(Vec::new()));

        let make_msg = |id: &str, text: &str| IncomingMessage {
            id: id.to_string(),
            platform: crate::core::domain::Platform::WhatsApp,
            session_role: SessionRole::PrimaryBot,
            chat_jid: "user_debounce@s.whatsapp.net".to_string(),
            chat_type: ChatType::DirectMessage,
            sender: Sender {
                jid: "user_debounce@s.whatsapp.net".to_string(),
                name: Some("Debounce User".to_string()),
            },
            text: text.to_string(),
            timestamp: 1000,
            is_from_me: false,
            quoted_message: None,
            mentioned_jids: vec![],
            is_bot_mentioned: true,
            bot_lid: None,
            has_media: false,
            media_type: None,
            media_path: None,
        };

        let res_clone = Arc::clone(&aggregated_results);
        let handler = move |msg: IncomingMessage| {
            let inner = Arc::clone(&res_clone);
            async move {
                inner.lock().await.push((msg.id, msg.text));
                Ok(())
            }
        };

        // Send 3 rapid messages within 50ms with 200ms debounce
        dispatch_incoming_message_to_queue(&chat_queues, make_msg("M1", "Pesan 1"), 1, 200, handler.clone()).await;
        tokio::time::sleep(tokio::time::Duration::from_millis(30)).await;
        dispatch_incoming_message_to_queue(&chat_queues, make_msg("M2", "Pesan 2"), 1, 200, handler.clone()).await;
        tokio::time::sleep(tokio::time::Duration::from_millis(30)).await;
        dispatch_incoming_message_to_queue(&chat_queues, make_msg("M3", "Pesan 3"), 1, 200, handler.clone()).await;

        // Wait for debounce timer (200ms) to expire
        tokio::time::sleep(tokio::time::Duration::from_millis(350)).await;

        let results = aggregated_results.lock().await.clone();
        // Should be aggregated into exactly 1 execution!
        assert_eq!(results.len(), 1);
        let (id, combined_text) = &results[0];
        assert_eq!(id, "M3"); // Points to latest message ID
        assert_eq!(combined_text, "Pesan 1\nPesan 2\nPesan 3");
    }

    #[tokio::test]
    async fn test_webhook_large_payload_limit() {
        let app = Router::new()
            .route(
                "/test_limit",
                post(|body: String| async move {
                    (StatusCode::OK, format!("len: {}", body.len()))
                }),
            )
            .layer(DefaultBodyLimit::max(100 * 1024 * 1024));

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });

        let client = reqwest::Client::new();
        // Generate a 5MB payload (exceeds standard 2MB Axum default limit)
        let payload_size = 5 * 1024 * 1024;
        let large_data = "x".repeat(payload_size);
        let resp = client
            .post(format!("http://{}/test_limit", addr))
            .body(large_data)
            .send()
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let text = resp.text().await.unwrap();
        assert_eq!(text, format!("len: {}", payload_size));
    }

    #[test]
    fn test_resolve_media_extension_variants() {
        assert_eq!(resolve_media_extension("application/pdf", Some("doc.pdf"), Some("document")), "pdf");
        assert_eq!(resolve_media_extension("application/pdf", None, Some("document")), "pdf");
        assert_eq!(resolve_media_extension("image/png", Some("snapshot.png"), Some("image")), "png");
        assert_eq!(resolve_media_extension("image/jpeg; charset=utf-8", None, Some("image")), "jpg");
        assert_eq!(resolve_media_extension("video/mp4", None, Some("video")), "mp4");
        assert_eq!(resolve_media_extension("audio/ogg", None, Some("audio")), "ogg");
        assert_eq!(resolve_media_extension("audio/mp4", None, Some("audio")), "m4a");
        assert_eq!(resolve_media_extension("application/vnd.openxmlformats-officedocument.wordprocessingml.document", Some("report.docx"), Some("document")), "docx");
    }

    #[test]
    fn test_parse_whatsmeow_claim_check_payload() {
        let payload = json!({
            "id": "MSG_CLAIM_CHECK_999",
            "from": "6282234120921@s.whatsapp.net",
            "body": "ini pdfnya",
            "has_media": true,
            "media_type": "document",
            "download_url": "/api/v1/media/MSG_CLAIM_CHECK_999/download",
            "filename": "laporan_keuangan_q3.pdf",
            "mime_type": "application/pdf",
            "file_length": 15518976
        });

        let msg = parse_whatsmeow_message(
            &payload,
            None,
            Some("628123456789@s.whatsapp.net"),
            None,
            None,
        )
        .expect("Message should parse");

        assert_eq!(msg.id, "MSG_CLAIM_CHECK_999");
        assert_eq!(msg.has_media, true);
        assert_eq!(msg.media_type, Some("document".to_string()));
        assert_eq!(msg.text, "ini pdfnya");
    }

    #[test]
    fn test_parse_whatsmeow_contact_message() {
        let vcard_content = "BEGIN:VCARD\nVERSION:3.0\nFN:Ihza Karunia\nTEL;type=CELL;waid=628123456789:+62 812-3456-789\nORG:BPS Mempawah\nTITLE:Pranata Komputer\nEMAIL:ihza@example.com\nEND:VCARD";
        let payload = json!({
            "id": "MSG_CONTACT_001",
            "from": "6282234120921@s.whatsapp.net",
            "message": {
                "contactMessage": {
                    "displayName": "Ihza Karunia",
                    "vcard": vcard_content
                }
            }
        });

        let msg = parse_whatsmeow_message(
            &payload,
            None,
            Some("628123456789@s.whatsapp.net"),
            None,
            None,
        )
        .expect("Contact message should parse");

        assert_eq!(msg.id, "MSG_CONTACT_001");
        assert_eq!(msg.media_type, Some("contact".to_string()));
        assert!(msg.text.contains("📇 [Kartu Kontak WhatsApp Dibagikan]"));
        assert!(msg.text.contains("Ihza Karunia"));
        assert!(msg.text.contains("+62 812-3456-789 (WA ID: 628123456789)"));
        assert!(msg.text.contains("BPS Mempawah"));
        assert!(msg.text.contains("Pranata Komputer"));
        assert!(msg.text.contains("ihza@example.com"));
        assert!(msg.text.contains("BEGIN:VCARD"));
    }

    #[test]
    fn test_parse_whatsmeow_contacts_array_message() {
        let payload = json!({
            "id": "MSG_CONTACTS_ARRAY_002",
            "from": "6282234120921@s.whatsapp.net",
            "message": {
                "contactsArrayMessage": {
                    "displayName": "2 Kontak",
                    "contacts": [
                        {
                            "displayName": "Budi Santoso",
                            "vcard": "BEGIN:VCARD\nVERSION:3.0\nFN:Budi Santoso\nTEL;waid=628111111:+628111111\nORG:Kantor Wilayah\nEND:VCARD"
                        },
                        {
                            "displayName": "Siti Rahma",
                            "vcard": "BEGIN:VCARD\nVERSION:3.0\nFN:Siti Rahma\nTEL;waid=628222222:+628222222\nEMAIL:siti@example.com\nEND:VCARD"
                        }
                    ]
                }
            }
        });

        let msg = parse_whatsmeow_message(
            &payload,
            None,
            Some("628123456789@s.whatsapp.net"),
            None,
            None,
        )
        .expect("Contacts array message should parse");

        assert_eq!(msg.id, "MSG_CONTACTS_ARRAY_002");
        assert_eq!(msg.media_type, Some("contact".to_string()));
        assert!(msg.text.contains("📇 [2 Kontak WhatsApp Dibagikan]"));
        assert!(msg.text.contains("Kontak #1"));
        assert!(msg.text.contains("Budi Santoso"));
        assert!(msg.text.contains("Kontak #2"));
        assert!(msg.text.contains("Siti Rahma"));
    }

    #[test]
    fn test_parse_whatsmeow_location_message() {
        let payload = json!({
            "id": "MSG_LOCATION_003",
            "from": "6282234120921@s.whatsapp.net",
            "message": {
                "locationMessage": {
                    "degreesLatitude": -0.0263,
                    "degreesLongitude": 109.3425,
                    "name": "BPS Kabupaten Mempawah",
                    "address": "Jl. Daeng Menambon, Mempawah, Kalimantan Barat"
                }
            }
        });

        let msg = parse_whatsmeow_message(
            &payload,
            None,
            Some("628123456789@s.whatsapp.net"),
            None,
            None,
        )
        .expect("Location message should parse");

        assert_eq!(msg.id, "MSG_LOCATION_003");
        assert_eq!(msg.media_type, Some("location".to_string()));
        assert!(msg.text.contains("📍 [Lokasi WhatsApp Dibagikan]"));
        assert!(msg.text.contains("BPS Kabupaten Mempawah"));
        assert!(msg.text.contains("-0.026300, 109.342500"));
        assert!(msg.text.contains("https://www.google.com/maps?q=-0.026300,109.342500"));
    }

    #[test]
    fn test_parse_whatsmeow_audio_video_heavy_skip() {
        // 1. Audio message
        let audio_payload = json!({
            "id": "MSG_AUDIO_004",
            "from": "6282234120921@s.whatsapp.net",
            "message": {
                "audioMessage": {
                    "mimetype": "audio/ogg; codecs=opus",
                    "seconds": 15
                }
            },
            "download_url": "/api/v1/media/audio.ogg",
            "media_type": "audio"
        });

        let audio_msg = parse_whatsmeow_message(
            &audio_payload,
            None,
            Some("628123456789@s.whatsapp.net"),
            None,
            None,
        )
        .expect("Audio message should parse");

        assert_eq!(audio_msg.id, "MSG_AUDIO_004");
        assert_eq!(audio_msg.has_media, false);
        assert!(audio_msg.text.contains("[Pesan Audio/Voice Note diabaikan: Format audio tidak diproses]"));

        // 2. Video message
        let video_payload = json!({
            "id": "MSG_VIDEO_005",
            "from": "6282234120921@s.whatsapp.net",
            "message": {
                "videoMessage": {
                    "mimetype": "video/mp4",
                    "seconds": 45
                }
            },
            "download_url": "/api/v1/media/video.mp4",
            "media_type": "video"
        });

        let video_msg = parse_whatsmeow_message(
            &video_payload,
            None,
            Some("628123456789@s.whatsapp.net"),
            None,
            None,
        )
        .expect("Video message should parse");

        assert_eq!(video_msg.id, "MSG_VIDEO_005");
        assert_eq!(video_msg.has_media, false);
        assert!(video_msg.text.contains("[Pesan Video diabaikan: Format video tidak diproses]"));
    }

    struct DummySessionStore;
    #[async_trait::async_trait]
    impl crate::core::ports::SessionStorePort for DummySessionStore {
        async fn get_conversation_id(&self, _chat_jid: &str) -> anyhow::Result<Option<String>> { Ok(None) }
        async fn save_conversation_id(&self, _chat_jid: &str, _conv_uuid: &str) -> anyhow::Result<()> { Ok(()) }
        async fn delete_conversation_id(&self, _chat_jid: &str) -> anyhow::Result<()> { Ok(()) }
        async fn record_message(&self, _chat_jid: &str, _sender_jid: &str, _text: &str, _is_from_me: bool) -> anyhow::Result<()> { Ok(()) }
        async fn get_user_profile(&self, _sender_jid: &str) -> anyhow::Result<Option<crate::core::ports::UserProfile>> { Ok(None) }
        async fn save_user_profile(&self, _profile: &crate::core::ports::UserProfile) -> anyhow::Result<()> { Ok(()) }
        async fn create_scheduled_task(&self, _task: &crate::core::domain::NewScheduledTask) -> anyhow::Result<i64> { Ok(1) }
        async fn list_scheduled_tasks(&self, _active_only: bool) -> anyhow::Result<Vec<crate::core::domain::ScheduledTask>> { Ok(vec![]) }
        async fn get_due_scheduled_tasks(&self, _current_epoch: i64) -> anyhow::Result<Vec<crate::core::domain::ScheduledTask>> { Ok(vec![]) }
        async fn update_scheduled_task_run(&self, _id: i64, _last_run: i64, _next_run: Option<i64>, _is_active: bool) -> anyhow::Result<()> { Ok(()) }
        async fn delete_scheduled_task(&self, _id: i64) -> anyhow::Result<bool> { Ok(true) }
        async fn get_scheduled_task(&self, _id: i64) -> anyhow::Result<Option<crate::core::domain::ScheduledTask>> { Ok(None) }
        async fn record_scheduled_task_run(&self, _task_id: i64, _task_title: &str, _target_jid: &str, _status: &str, _duration_secs: f64, _error_message: Option<&str>, _output_preview: Option<&str>) -> anyhow::Result<i64> { Ok(1) }
        async fn list_scheduled_task_runs(&self, _limit: usize) -> anyhow::Result<Vec<crate::core::domain::ScheduledTaskRun>> { Ok(vec![]) }
        async fn update_scheduled_task_result(&self, _id: i64, _status: &str, _error_message: Option<&str>, _duration_secs: f64) -> anyhow::Result<()> { Ok(()) }
        async fn get_scheduler_diagnostics(&self) -> anyhow::Result<crate::core::domain::SchedulerDiagnostics> {
            Ok(crate::core::domain::SchedulerDiagnostics {
                total_tasks: 0,
                active_tasks: 0,
                total_runs: 0,
                successful_runs: 0,
                failed_runs: 0,
                last_run: None,
                last_failure: None,
                next_task: None,
            })
        }
        async fn record_action_audit(&self, _audit: &crate::core::domain::NewWhatsAppActionAudit) -> anyhow::Result<i64> { Ok(1) }
        async fn update_action_audit_result(&self, _id: i64, _conversation_id: Option<&str>, _response_text: Option<&str>, _error_message: Option<&str>, _status: &str, _duration_seconds: Option<f64>, _tools_invoked: &[String]) -> anyhow::Result<()> { Ok(()) }
        async fn query_action_audits(&self, _filter: &crate::core::domain::ActionAuditFilter) -> anyhow::Result<Vec<crate::core::domain::WhatsAppActionAudit>> { Ok(vec![]) }
        async fn get_action_audit_by_id(&self, _id: i64) -> anyhow::Result<Option<crate::core::domain::WhatsAppActionAudit>> { Ok(None) }
        async fn get_action_audit_by_message_id(&self, _message_id: &str) -> anyhow::Result<Option<crate::core::domain::WhatsAppActionAudit>> { Ok(None) }
        async fn get_action_audit_summary(&self) -> anyhow::Result<crate::core::domain::AuditSummaryReport> {
            Ok(crate::core::domain::AuditSummaryReport {
                total_actions: 0,
                total_responses: 0,
                total_recorded_only: 0,
                total_ignored: 0,
                total_errors: 0,
                avg_duration_seconds: 0.0,
                most_active_chats: vec![],
                most_active_senders: vec![],
                decision_breakdown: vec![],
                status_breakdown: vec![],
                top_tools_used: vec![],
            })
        }
        async fn record_metacognitive_prediction(&self, _pred: &crate::core::domain::NewMetacognitivePrediction) -> anyhow::Result<i64> { Ok(1) }
        async fn resolve_metacognitive_prediction(&self, _prediction_id: &str, _actual_outcome: f64, _duration_secs: f64, _error_detail: Option<&str>) -> anyhow::Result<()> { Ok(()) }
        async fn list_metacognitive_predictions(&self, _limit: usize, _domain_filter: Option<&str>) -> anyhow::Result<Vec<crate::core::domain::MetacognitivePrediction>> { Ok(vec![]) }
        async fn get_metacognitive_calibration_stats(&self) -> anyhow::Result<crate::core::domain::MetacognitiveCalibrationStats> {
            Ok(crate::core::domain::MetacognitiveCalibrationStats {
                total_predictions: 0,
                resolved_predictions: 0,
                mean_brier_score: 0.0,
                base_rate: 0.0,
                brier_skill_score: 0.0,
                calibration_status: "uncalibrated".to_string(),
                reliability_buckets: vec![],
                domain_brier_scores: std::collections::HashMap::new(),
            })
        }
    }

    struct DummyAgentEngine;
    #[async_trait::async_trait]
    impl crate::core::ports::AgentEnginePort for DummyAgentEngine {
        async fn execute_with_model(&self, _conv_id: Option<&str>, _prompt: &str, _model: Option<&str>) -> anyhow::Result<crate::core::ports::AgentResponse> {
            Ok(crate::core::ports::AgentResponse {
                conversation_id: "dummy".into(),
                response_text: "OK".into(),
                duration_seconds: 0.1,
            })
        }
        async fn get_model(&self) -> String { "dummy".into() }
        async fn set_model(&self, _model: &str) -> anyhow::Result<()> { Ok(()) }
        async fn is_authenticated(&self) -> bool { true }
        async fn save_auth_token(&self, _token_content: &str) -> anyhow::Result<()> { Ok(()) }
    }

    struct DummyWhatsApp;
    #[async_trait::async_trait]
    impl crate::core::ports::WhatsAppPort for DummyWhatsApp {
        async fn send_text(&self, _to: &str, _text: &str, _qid: Option<&str>) -> anyhow::Result<()> { Ok(()) }
        async fn send_presence(&self, _to: &str, _state: crate::core::domain::PresenceState) -> anyhow::Result<()> { Ok(()) }
    }

    #[tokio::test]
    async fn test_webhook_claim_check_stream_download() {
        const DUMMY_PDF_CONTENT: &[u8] = b"%PDF-1.4 Mock Claim Check Large Document Content";
        let mock_app = Router::new().route(
            "/api/v1/media/{id}/download",
            get(|Path(id): Path<String>, headers: HeaderMap| async move {
                let api_key = headers.get("X-API-Key").and_then(|v| v.to_str().ok()).unwrap_or("");
                if api_key != "secret-whatsmeow-key" {
                    return (StatusCode::UNAUTHORIZED, axum::http::HeaderMap::new(), vec![]);
                }
                let mut resp_headers = HeaderMap::new();
                resp_headers.insert("Content-Type", "application/pdf".parse().unwrap());
                resp_headers.insert("Content-Disposition", format!("attachment; filename=\"{}.pdf\"", id).parse().unwrap());
                (StatusCode::OK, resp_headers, DUMMY_PDF_CONTENT.to_vec())
            }),
        );

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let _ = axum::serve(listener, mock_app).await;
        });

        let test_id = format!("aina_cc_test_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos());
        let temp_dir = std::env::temp_dir().join(test_id);
        let _ = tokio::fs::create_dir_all(&temp_dir).await;

        let session_store = Arc::new(DummySessionStore);
        let agent_engine = Arc::new(DummyAgentEngine);
        let whatsapp = Arc::new(DummyWhatsApp);
        let persona_engine = Arc::new(PersonaEngine::new(
            "Aina".to_string(),
            "Org".to_string(),
            "admin@s.whatsapp.net".to_string(),
            "Asia/Jakarta".to_string(),
            7,
            "id".to_string(),
            format!("http://{}", addr),
            "bot@s.whatsapp.net".to_string(),
            Some(temp_dir.to_string_lossy().to_string()),
        ));

        let usecase = Arc::new(ProcessIncomingMessageUseCase::new(
            session_store.clone(),
            agent_engine.clone(),
            whatsapp.clone(),
            persona_engine.clone(),
            "bot@s.whatsapp.net".to_string(),
            "Aina".to_string(),
            None,
        ));

        let state = Arc::new(WebhookServerState {
            usecase,
            agent_engine,
            session_store,
            persona_engine,
            bot_name: "Aina".to_string(),
            bot_jid: "bot@s.whatsapp.net".to_string(),
            bot_lid: None,
            companion_jid: None,
            companion_name: None,
            companion_session_id: None,
            model: "dummy".to_string(),
            whatsmeow_url: format!("http://{}", addr),
            whatsmeow_api_key: "secret-whatsmeow-key".to_string(),
            companion_base_url: None,
            companion_api_key: None,
            setup_code: "SECRET123".to_string(),
            timezone: "Asia/Jakarta".to_string(),
            locale: "id".to_string(),
            sim_jobs: Arc::new(RwLock::new(HashMap::new())),
            chat_queues: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            active_tasks: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            presence_tracker: Arc::new(crate::core::domain::PresenceTracker::new()),
            workspace_dir: temp_dir.clone(),
        });

        let app = create_router(state);
        let aina_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let aina_addr = aina_listener.local_addr().unwrap();
        tokio::spawn(async move {
            let _ = axum::serve(aina_listener, app).await;
        });

        let client = reqwest::Client::new();
        let payload = json!({
            "id": "MSG_CC_TEST_001",
            "chat_jid": "user123@s.whatsapp.net",
            "sender_jid": "user123@s.whatsapp.net",
            "download_url": "/api/v1/media/MSG_CC_TEST_001/download",
            "media_url": "/api/v1/media/MSG_CC_TEST_001/download",
            "has_media": true,
            "media_type": "document",
            "filename": "laporan_keuangan.pdf",
            "mime_type": "application/pdf",
            "file_length": DUMMY_PDF_CONTENT.len(),
            "text": "tolong cek pdf ini"
        });

        let resp = client
            .post(format!("http://{}/webhook", aina_addr))
            .json(&payload)
            .send()
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);

        let target_pdf = temp_dir.join("media").join("MSG_CC_TEST_001.pdf");
        let target_txt = temp_dir.join("media").join("MSG_CC_TEST_001.txt");

        let mut downloaded = false;
        for _ in 0..50 {
            if target_pdf.exists() && target_txt.exists() {
                downloaded = true;
                break;
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        }

        assert!(downloaded, "Media file or sidecar .txt was not created by claim-check handler");
        let read_bytes = tokio::fs::read(&target_pdf).await.unwrap();
        assert_eq!(read_bytes, DUMMY_PDF_CONTENT);

        let sidecar_content = tokio::fs::read_to_string(&target_txt).await.unwrap();
        assert!(sidecar_content.contains("ID: MSG_CC_TEST_001"));
        assert!(sidecar_content.contains("laporan_keuangan.pdf"));
        assert!(sidecar_content.contains("application/pdf"));

        let _ = tokio::fs::remove_dir_all(&temp_dir).await;
    }

    #[tokio::test]
    async fn test_api_audit_endpoints_secured_and_queries() {
        let test_id = format!("aina_test_audit_api_{}", rand::random::<u32>());
        let temp_dir = std::env::temp_dir().join(test_id);
        let _ = tokio::fs::create_dir_all(&temp_dir).await;

        let dummy_session_store = Arc::new(DummySessionStore);
        let dummy_agent_engine = Arc::new(DummyAgentEngine);
        let dummy_whatsapp = Arc::new(DummyWhatsApp);
        let persona_engine = Arc::new(PersonaEngine::new(
            "Aina".to_string(),
            "Org".to_string(),
            "admin@s.whatsapp.net".to_string(),
            "Asia/Jakarta".to_string(),
            7,
            "id".to_string(),
            "http://127.0.0.1:8080".to_string(),
            "aina@s.whatsapp.net".to_string(),
            Some(temp_dir.to_string_lossy().to_string()),
        ));

        let usecase = Arc::new(ProcessIncomingMessageUseCase::new(
            dummy_session_store.clone(),
            dummy_agent_engine.clone(),
            dummy_whatsapp.clone(),
            persona_engine.clone(),
            "aina@s.whatsapp.net".to_string(),
            "Aina".to_string(),
            None,
        ));

        let state = Arc::new(WebhookServerState {
            usecase,
            agent_engine: dummy_agent_engine,
            session_store: dummy_session_store,
            persona_engine,
            bot_name: "Aina".to_string(),
            bot_jid: "aina@s.whatsapp.net".to_string(),
            bot_lid: None,
            companion_jid: None,
            companion_name: None,
            companion_session_id: None,
            model: "dummy-model".to_string(),
            whatsmeow_url: "http://127.0.0.1:8080".to_string(),
            whatsmeow_api_key: "WM_KEY_999".to_string(),
            companion_base_url: None,
            companion_api_key: None,
            setup_code: "SETUP_SECRET_777".to_string(),
            timezone: "Asia/Jakarta".to_string(),
            locale: "id".to_string(),
            sim_jobs: Arc::new(RwLock::new(HashMap::new())),
            chat_queues: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            active_tasks: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            presence_tracker: Arc::new(crate::core::domain::PresenceTracker::new()),
            workspace_dir: temp_dir.clone(),
        });

        let app = create_router(state);
        let aina_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let aina_addr = aina_listener.local_addr().unwrap();
        tokio::spawn(async move {
            let _ = axum::serve(aina_listener, app).await;
        });

        let client = reqwest::Client::new();

        // 1. Unauthorized request without key should return 401
        let unauth_resp = client
            .get(format!("http://{}/api/audit/actions", aina_addr))
            .send()
            .await
            .unwrap();
        assert_eq!(unauth_resp.status(), StatusCode::UNAUTHORIZED);

        // 2. Authorized request with Bearer token matching setup_code
        let bearer_resp = client
            .get(format!("http://{}/api/audit/actions", aina_addr))
            .header("Authorization", "Bearer SETUP_SECRET_777")
            .send()
            .await
            .unwrap();
        assert_eq!(bearer_resp.status(), StatusCode::OK);
        let json_data: serde_json::Value = bearer_resp.json().await.unwrap();
        assert_eq!(json_data["success"], true);

        // 3. Authorized request with X-API-Key matching whatsmeow_api_key
        let api_key_resp = client
            .get(format!("http://{}/api/audit/summary", aina_addr))
            .header("X-API-Key", "WM_KEY_999")
            .send()
            .await
            .unwrap();
        assert_eq!(api_key_resp.status(), StatusCode::OK);
        let summary_data: serde_json::Value = api_key_resp.json().await.unwrap();
        assert_eq!(summary_data["success"], true);

        // 4. Authorized request with X-Admin-Key matching setup_code
        let admin_key_resp = client
            .get(format!("http://{}/api/audit/diagnostics", aina_addr))
            .header("X-Admin-Key", "SETUP_SECRET_777")
            .send()
            .await
            .unwrap();
        assert_eq!(admin_key_resp.status(), StatusCode::OK);
        let diag_data: serde_json::Value = admin_key_resp.json().await.unwrap();
        assert_eq!(diag_data["success"], true);
        assert!(diag_data["diagnostics"]["memory_rss_bytes"].is_number());
        assert_eq!(diag_data["diagnostics"]["bot_name"], "Aina");

        // 5. Authorized request via query parameter ?key=...
        let query_resp = client
            .get(format!("http://{}/api/audit/transcripts?key=SETUP_SECRET_777", aina_addr))
            .send()
            .await
            .unwrap();
        assert_eq!(query_resp.status(), StatusCode::OK);
        let trans_data: serde_json::Value = query_resp.json().await.unwrap();
        assert_eq!(trans_data["success"], true);

        // 6. Test /api/persona/diagnostics & /api/persona/journal
        let persona_diag_resp = client
            .get(format!("http://{}/api/persona/diagnostics", aina_addr))
            .send()
            .await
            .unwrap();
        assert_eq!(persona_diag_resp.status(), StatusCode::OK);
        let pdiag_data: serde_json::Value = persona_diag_resp.json().await.unwrap();
        assert_eq!(pdiag_data["success"], true);
        assert!(pdiag_data["diagnostics"]["today_quota"].is_object());

        let persona_journal_resp = client
            .get(format!("http://{}/api/persona/journal?limit=5", aina_addr))
            .send()
            .await
            .unwrap();
        assert_eq!(persona_journal_resp.status(), StatusCode::OK);
        let pjournal_data: serde_json::Value = persona_journal_resp.json().await.unwrap();
        assert_eq!(pjournal_data["success"], true);

        // 7. Test /api/schedule/diagnostics
        let sched_diag_resp = client
            .get(format!("http://{}/api/schedule/diagnostics", aina_addr))
            .send()
            .await
            .unwrap();
        assert_eq!(sched_diag_resp.status(), StatusCode::OK);
        let sdiag_data: serde_json::Value = sched_diag_resp.json().await.unwrap();
        assert_eq!(sdiag_data["success"], true);

        let _ = tokio::fs::remove_dir_all(&temp_dir).await;
    }
}
