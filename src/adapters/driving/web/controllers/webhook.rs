use crate::adapters::driving::web::ingress::{parse_whatsmeow_message, resolve_media_extension};
use crate::adapters::driving::web::queue::dispatch_message_to_queue;
use crate::adapters::driving::web::state::WebhookServerState;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

pub async fn webhook_handler(
    State(state): State<Arc<WebhookServerState>>,
    Query(query_params): Query<HashMap<String, String>>,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    debug!("Received webhook payload: {:?}, query: {:?}", payload, query_params);

    let mut msg_opt = parse_whatsmeow_message(
        &payload,
        Some(&query_params),
        Some(&state.bot_jid),
        state.companion_jid.as_deref(),
        state.companion_session_id.as_deref(),
    );

    if let Some(ref mut msg) = msg_opt {
        let root = if let Some(data_obj) = payload.get("data").and_then(|d| d.as_object()) {
            data_obj
        } else if let Some(root_obj) = payload.as_object() {
            root_obj
        } else {
            &serde_json::Map::new()
        };

        let media_base64 = root
            .get("media_base64")
            .or_else(|| payload.get("media_base64"))
            .and_then(|v| v.as_str());

        let mime_type = root
            .get("mime_type")
            .or_else(|| payload.get("mime_type"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let filename_opt = root
            .get("filename")
            .or_else(|| payload.get("filename"))
            .or_else(|| root.get("media_info").and_then(|m| m.get("filename")))
            .or_else(|| payload.get("media_info").and_then(|m| m.get("filename")))
            .and_then(|v| v.as_str());

        let media_type_opt = root
            .get("media_type")
            .or_else(|| payload.get("media_type"))
            .and_then(|v| v.as_str())
            .or_else(|| msg.media_type.as_deref());

        let download_url_opt = root
            .get("download_url")
            .or_else(|| root.get("media_url"))
            .or_else(|| payload.get("download_url"))
            .or_else(|| payload.get("media_url"))
            .and_then(|v| v.as_str());

        let mut media_bytes: Option<Vec<u8>> = None;
        let mut effective_mime = mime_type.to_string();

        if let Some(b64) = media_base64 {
            let clean = b64.trim();
            if !clean.is_empty() {
                use base64::Engine;
                if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(clean) {
                    media_bytes = Some(bytes);
                }
            }
        }

        let is_heavy_media = msg.media_type.as_deref() == Some("audio")
            || msg.media_type.as_deref() == Some("video")
            || msg.media_type.as_deref() == Some("ptt")
            || mime_type.starts_with("audio/")
            || mime_type.starts_with("video/");

        if is_heavy_media {
            info!("Skipping media download for heavy media type (audio/video): mime='{}', type='{:?}'", effective_mime, media_type_opt);
            msg.has_media = false;
        } else if media_bytes.is_none() {
            // CLAIM-CHECK PATTERN:
            // If media_base64 is absent or omitted, retrieve media stream on-demand
            // using the download_url claim check ticket or fallback to media id.
            let (target_url, target_key) = if msg.session_role == crate::core::domain::SessionRole::UserCompanion
                && state.companion_base_url.is_some()
            {
                (
                    state.companion_base_url.as_deref().unwrap(),
                    state.companion_api_key.as_deref().unwrap_or(&state.whatsmeow_api_key),
                )
            } else {
                (state.whatsmeow_url.as_str(), state.whatsmeow_api_key.as_str())
            };

            let full_url = if let Some(url_str) = download_url_opt {
                if url_str.starts_with("http://") || url_str.starts_with("https://") {
                    url_str.to_string()
                } else {
                    format!(
                        "{}{}",
                        target_url.trim_end_matches('/'),
                        if url_str.starts_with('/') {
                            url_str.to_string()
                        } else {
                            format!("/{}", url_str)
                        }
                    )
                }
            } else if msg.has_media
                && !msg.id.is_empty()
                && msg.id != "unknown_id"
                && !target_url.is_empty()
            {
                format!(
                    "{}/api/v1/media/{}/download",
                    target_url.trim_end_matches('/'),
                    msg.id
                )
            } else {
                String::new()
            };

            if !full_url.is_empty() {
                info!("Claim-Check: Fetching media for msg {} from {}", msg.id, full_url);
                let client = reqwest::Client::builder()
                    .timeout(std::time::Duration::from_secs(60))
                    .build()
                    .unwrap_or_else(|_| reqwest::Client::new());

                let mut req = client.get(&full_url);
                if !target_key.is_empty() {
                    req = req
                        .header("Authorization", format!("Bearer {}", target_key))
                        .header("X-API-Key", target_key);
                }

                match req.send().await {
                    Ok(resp) => {
                        if resp.status().is_success() {
                            if effective_mime.is_empty() {
                                if let Some(ct) = resp
                                    .headers()
                                    .get(reqwest::header::CONTENT_TYPE)
                                    .and_then(|v| v.to_str().ok())
                                {
                                    effective_mime = ct.to_string();
                                }
                            }
                            match resp.bytes().await {
                                Ok(b) => {
                                    info!(
                                        "Successfully streamed {} bytes of media for msg {}",
                                        b.len(),
                                        msg.id
                                    );
                                    media_bytes = Some(b.to_vec());
                                }
                                Err(e) => {
                                    error!(
                                        "Failed to read media response body from {}: {:?}",
                                        full_url, e
                                    );
                                }
                            }
                        } else {
                            warn!(
                                "Failed to stream media from {}: HTTP status {}",
                                full_url,
                                resp.status()
                            );
                        }
                    }
                    Err(e) => {
                        error!("Network error fetching media from {}: {:?}", full_url, e);
                    }
                }
            }
        }

        if let Some(bytes) = media_bytes {
            let ext = resolve_media_extension(&effective_mime, filename_opt, media_type_opt);
            let media_dir = state.workspace_dir.join("media");
            if let Err(e) = tokio::fs::create_dir_all(&media_dir).await {
                error!("Failed to create media directory {:?}: {:?}", media_dir, e);
            } else {
                let file_name = format!("{}.{}", msg.id, ext);
                let target_path = media_dir.join(&file_name);
                let sidecar_txt_path = media_dir.join(format!("{}.txt", msg.id));

                if let Err(e) = tokio::fs::write(&target_path, &bytes).await {
                    error!("Failed to write media file to {:?}: {:?}", target_path, e);
                } else {
                    info!("Saved incoming media to {:?} ({} bytes)", target_path, bytes.len());
                    let abs_path_str = target_path.to_string_lossy().to_string();
                    msg.media_path = Some(abs_path_str.clone());
                    msg.has_media = true;

                    if msg.text.trim().is_empty() {
                        if let Some(fname) = filename_opt {
                            msg.text = format!("[Dokumen terlampir: {}]", fname);
                        } else if let Some(m_type) = media_type_opt {
                            msg.text = format!("[{} terlampir]", match m_type {
                                "document" => "Dokumen",
                                "video" => "Video",
                                "audio" => "Audio",
                                _ => "Foto / Media",
                            });
                        }
                    }

                    // Create initial .txt companion file with metadata & caption for searchable indexing
                    let initial_txt = format!(
                        "ID: {}\nPengirim: {} ({})\nWaktu: {}\nCaption/Pesan: {}\nPath File: {}\nNama File Asli: {}\nMIME Type: {}\nUkuran: {} bytes\n\n--- Catatan & Hasil Transkripsi / Analisis Aina ---\n",
                        msg.id,
                        msg.sender.name.as_deref().unwrap_or("Anonim"),
                        msg.sender.jid,
                        msg.timestamp,
                        msg.text,
                        abs_path_str,
                        filename_opt.unwrap_or("-"),
                        effective_mime,
                        bytes.len()
                    );
                    let _ = tokio::fs::write(&sidecar_txt_path, initial_txt).await;
                }
            }
        }

        dispatch_message_to_queue(&state, msg.clone()).await;
    } else {
        debug!("Webhook received event that was not a parseable user message");
    }

    (StatusCode::OK, "OK")
}
