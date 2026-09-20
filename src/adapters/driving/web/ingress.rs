use crate::adapters::driving::web::ui::chrono_now_secs;
use crate::core::domain::{ChatType, IncomingMessage, QuotedMessage, Sender};
use serde_json::Value;
use std::collections::HashMap;

pub fn resolve_media_extension(
    mime_type: &str,
    filename: Option<&str>,
    media_type: Option<&str>,
) -> String {
    if let Some(fname) = filename {
        if let Some(ext) = std::path::Path::new(fname).extension().and_then(|e| e.to_str()) {
            let clean_ext = ext.trim().to_lowercase();
            if !clean_ext.is_empty() && clean_ext.len() <= 6 && clean_ext.chars().all(|c| c.is_alphanumeric()) {
                return clean_ext;
            }
        }
    }

    let mime_clean = mime_type.split(';').next().unwrap_or(mime_type).trim().to_lowercase();
    match mime_clean.as_str() {
        "image/jpeg" | "image/jpg" => "jpg".to_string(),
        "image/png" => "png".to_string(),
        "image/webp" => "webp".to_string(),
        "image/gif" => "gif".to_string(),
        "video/mp4" => "mp4".to_string(),
        "video/quicktime" => "mov".to_string(),
        "video/x-matroska" => "mkv".to_string(),
        "audio/ogg" => "ogg".to_string(),
        "audio/mp4" | "audio/m4a" => "m4a".to_string(),
        "audio/mpeg" | "audio/mp3" => "mp3".to_string(),
        "audio/wav" | "audio/x-wav" => "wav".to_string(),
        "application/pdf" => "pdf".to_string(),
        "application/zip" => "zip".to_string(),
        "application/x-tar" => "tar".to_string(),
        "application/gzip" => "gz".to_string(),
        "text/plain" => "txt".to_string(),
        "text/csv" => "csv".to_string(),
        "application/json" => "json".to_string(),
        "application/msword" => "doc".to_string(),
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => "docx".to_string(),
        "application/vnd.ms-excel" => "xls".to_string(),
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" => "xlsx".to_string(),
        "application/vnd.ms-powerpoint" => "ppt".to_string(),
        "application/vnd.openxmlformats-officedocument.presentationml.presentation" => "pptx".to_string(),
        _ => match media_type.unwrap_or("") {
            "image" => "jpg".to_string(),
            "video" => "mp4".to_string(),
            "audio" => "ogg".to_string(),
            "document" => "pdf".to_string(),
            _ => "bin".to_string(),
        },
    }
}

/// Parses a vCard string into a clean, human-readable and AI-friendly summary format.
fn parse_vcard_summary(vcard: &str, display_name_fallback: Option<&str>) -> String {
    let mut fn_name = String::new();
    let mut phones = Vec::new();
    let mut emails = Vec::new();
    let mut org = String::new();
    let mut title = String::new();
    let mut notes = Vec::new();

    for line in vcard.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let upper = trimmed.to_uppercase();
        if upper.starts_with("FN:") || upper.starts_with("FN;") {
            if let Some((_, val)) = trimmed.split_once(':') {
                fn_name = val.trim().to_string();
            }
        } else if upper.starts_with("TEL:") || upper.starts_with("TEL;") {
            if let Some((params, val)) = trimmed.split_once(':') {
                let clean_phone = val.trim();
                let waid = params
                    .split(';')
                    .find(|p| p.to_uppercase().starts_with("WAID="))
                    .and_then(|p| p.split_once('='))
                    .map(|(_, id)| id.trim())
                    .unwrap_or("");
                if !waid.is_empty() {
                    phones.push(format!("{} (WA ID: {})", clean_phone, waid));
                } else {
                    phones.push(clean_phone.to_string());
                }
            }
        } else if upper.starts_with("EMAIL:") || upper.starts_with("EMAIL;") {
            if let Some((_, val)) = trimmed.split_once(':') {
                emails.push(val.trim().to_string());
            }
        } else if upper.starts_with("ORG:") || upper.starts_with("ORG;") {
            if let Some((_, val)) = trimmed.split_once(':') {
                org = val.trim().replace(';', " - ");
            }
        } else if upper.starts_with("TITLE:") || upper.starts_with("TITLE;") {
            if let Some((_, val)) = trimmed.split_once(':') {
                title = val.trim().to_string();
            }
        } else if upper.starts_with("NOTE:") || upper.starts_with("NOTE;") {
            if let Some((_, val)) = trimmed.split_once(':') {
                notes.push(val.trim().to_string());
            }
        }
    }

    let name = if !fn_name.is_empty() {
        fn_name
    } else {
        display_name_fallback.unwrap_or("Tanpa Nama").to_string()
    };

    let mut out = format!("📇 [Kartu Kontak WhatsApp Dibagikan]\n• Nama: {}", name);
    if !phones.is_empty() {
        out.push_str(&format!("\n• Nomor Telepon / WA: {}", phones.join(", ")));
    }
    if !org.is_empty() {
        out.push_str(&format!("\n• Organisasi / Instansi: {}", org));
    }
    if !title.is_empty() {
        out.push_str(&format!("\n• Jabatan: {}", title));
    }
    if !emails.is_empty() {
        out.push_str(&format!("\n• Email: {}", emails.join(", ")));
    }
    if !notes.is_empty() {
        out.push_str(&format!("\n• Catatan: {}", notes.join("; ")));
    }

    out.push_str("\n\n--- vCard Mentah ---\n");
    out.push_str(vcard.trim());
    out
}

/// Formats a WhatsApp location payload into human and AI readable structure with Google Maps link.
fn format_location_message(loc_obj: &serde_json::Map<String, Value>) -> String {
    let lat = loc_obj.get("degreesLatitude").or_else(|| loc_obj.get("latitude")).and_then(|v| v.as_f64()).unwrap_or(0.0);
    let lng = loc_obj.get("degreesLongitude").or_else(|| loc_obj.get("longitude")).and_then(|v| v.as_f64()).unwrap_or(0.0);
    let name = loc_obj.get("name").and_then(|v| v.as_str()).unwrap_or("");
    let address = loc_obj.get("address").and_then(|v| v.as_str()).unwrap_or("");
    let comment = loc_obj.get("comment").or_else(|| loc_obj.get("caption")).and_then(|v| v.as_str()).unwrap_or("");

    let mut out = "📍 [Lokasi WhatsApp Dibagikan]".to_string();
    if !name.is_empty() {
        out.push_str(&format!("\n• Nama Tempat: {}", name));
    }
    if !address.is_empty() {
        out.push_str(&format!("\n• Alamat: {}", address));
    }
    if lat != 0.0 || lng != 0.0 {
        out.push_str(&format!("\n• Koordinat: {:.6}, {:.6}", lat, lng));
        out.push_str(&format!("\n• Google Maps: https://www.google.com/maps?q={:.6},{:.6}", lat, lng));
    }
    if !comment.is_empty() {
        out.push_str(&format!("\n• Keterangan: {}", comment));
    }
    out
}

/// Extracts text and media types across varied WhatsApp message structures
/// (plain text, contact cards, locations, documents, images) while flagging heavy audio/video.
fn extract_whatsmeow_content(
    root: &serde_json::Map<String, Value>,
    val: &Value,
) -> (String, Option<String>, bool) {
    let direct_text = root
        .get("body")
        .or_else(|| root.get("text"))
        .or_else(|| root.get("conversation"))
        .or_else(|| val.get("body"))
        .or_else(|| val.get("text"))
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let mut media_type = root
        .get("media_type")
        .or_else(|| val.get("media_type"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_lowercase());

    let msg_obj = root
        .get("message")
        .or_else(|| val.get("message"))
        .and_then(|m| m.as_object());

    // 1. Check for Contact Message (Single Contact)
    let contact_obj = msg_obj
        .and_then(|m| m.get("contactMessage"))
        .or_else(|| root.get("contactMessage"))
        .or_else(|| root.get("contact"))
        .or_else(|| val.get("contact"))
        .and_then(|c| c.as_object());

    if let Some(c) = contact_obj {
        let vcard = c.get("vcard").and_then(|v| v.as_str()).unwrap_or("");
        let disp_name = c.get("displayName").or_else(|| c.get("display_name")).and_then(|v| v.as_str());
        let summary = parse_vcard_summary(vcard, disp_name);
        return (summary, Some("contact".to_string()), false);
    }

    // 2. Check for Contacts Array Message (Multiple Contacts)
    let contacts_array_obj = msg_obj
        .and_then(|m| m.get("contactsArrayMessage"))
        .or_else(|| root.get("contactsArrayMessage"))
        .or_else(|| root.get("contacts"))
        .or_else(|| val.get("contacts"))
        .and_then(|c| c.as_object());

    if let Some(ca) = contacts_array_obj {
        let contacts_arr = ca.get("contacts").and_then(|v| v.as_array());
        if let Some(arr) = contacts_arr {
            let mut parts = Vec::new();
            for (i, item) in arr.iter().enumerate() {
                if let Some(c) = item.as_object() {
                    let vcard = c.get("vcard").and_then(|v| v.as_str()).unwrap_or("");
                    let disp_name = c.get("displayName").or_else(|| c.get("display_name")).and_then(|v| v.as_str());
                    parts.push(format!("--- Kontak #{} ---\n{}", i + 1, parse_vcard_summary(vcard, disp_name)));
                }
            }
            let summary = format!("📇 [{} Kontak WhatsApp Dibagikan]\n\n{}", arr.len(), parts.join("\n\n"));
            return (summary, Some("contact".to_string()), false);
        }
    }

    // 3. Check for Location Message
    let loc_obj = msg_obj
        .and_then(|m| m.get("locationMessage").or_else(|| m.get("liveLocationMessage")))
        .or_else(|| root.get("locationMessage"))
        .or_else(|| root.get("liveLocationMessage"))
        .or_else(|| root.get("location"))
        .or_else(|| val.get("location"))
        .and_then(|l| l.as_object());

    if let Some(loc) = loc_obj {
        let summary = format_location_message(loc);
        return (summary, Some("location".to_string()), false);
    }

    // 4. Check for Extended Text Message
    if let Some(ext) = msg_obj.and_then(|m| m.get("extendedTextMessage")).and_then(|e| e.as_object()) {
        if let Some(t) = ext.get("text").and_then(|v| v.as_str()) {
            return (t.trim().to_string(), media_type, false);
        }
    }

    // 5. Check for Document Message
    if let Some(doc) = msg_obj.and_then(|m| m.get("documentMessage")).and_then(|d| d.as_object()) {
        media_type = Some("document".to_string());
        let cap = doc.get("caption").and_then(|v| v.as_str()).unwrap_or("").trim();
        let fname = doc.get("fileName").or_else(|| doc.get("title")).and_then(|v| v.as_str()).unwrap_or("").trim();
        let text = if !cap.is_empty() {
            cap.to_string()
        } else if !fname.is_empty() {
            format!("[Dokumen terlampir: {}]", fname)
        } else {
            "[Dokumen terlampir]".to_string()
        };
        return (text, media_type, false);
    }

    // 6. Check for Image Message
    if let Some(img) = msg_obj.and_then(|m| m.get("imageMessage")).and_then(|i| i.as_object()) {
        media_type = Some("image".to_string());
        let cap = img.get("caption").and_then(|v| v.as_str()).unwrap_or("").trim();
        let text = if !cap.is_empty() {
            cap.to_string()
        } else {
            "[Foto / Gambar terlampir]".to_string()
        };
        return (text, media_type, false);
    }

    // 7. Check for Audio / Voice Note Message (Heavy Media: Flagged)
    let is_audio = msg_obj.and_then(|m| m.get("audioMessage")).is_some()
        || media_type.as_deref() == Some("audio")
        || media_type.as_deref() == Some("ptt")
        || root.get("mime_type").and_then(|v| v.as_str()).map(|s| s.starts_with("audio/")).unwrap_or(false);

    if is_audio {
        return (
            "[Pesan Audio/Voice Note diabaikan: Format audio tidak diproses]".to_string(),
            Some("audio".to_string()),
            true,
        );
    }

    // 8. Check for Video Message (Heavy Media: Flagged)
    let is_video = msg_obj.and_then(|m| m.get("videoMessage")).is_some()
        || media_type.as_deref() == Some("video")
        || root.get("mime_type").and_then(|v| v.as_str()).map(|s| s.starts_with("video/")).unwrap_or(false);

    if is_video {
        return (
            "[Pesan Video diabaikan: Format video tidak diproses]".to_string(),
            Some("video".to_string()),
            true,
        );
    }

    // 9. Fallback to plain message string or direct text
    let plain_msg = root
        .get("message")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let final_text = direct_text
        .or(plain_msg)
        .unwrap_or_default();

    (final_text, media_type, false)
}

pub fn parse_whatsmeow_message(
    val: &Value,
    query_params: Option<&HashMap<String, String>>,
    bot_jid: Option<&str>,
    companion_jid: Option<&str>,
    companion_session_id: Option<&str>,
) -> Option<IncomingMessage> {
    let root = if let Some(data_obj) = val.get("data").and_then(|d| d.as_object()) {
        data_obj
    } else if let Some(root_obj) = val.as_object() {
        root_obj
    } else {
        return None;
    };

    let chat_jid = root
        .get("from")
        .or_else(|| root.get("chat_jid"))
        .or_else(|| root.get("remote_jid"))
        .or_else(|| root.get("chat"))
        .and_then(|v| v.as_str())?
        .to_string();

    let sender_jid = root
        .get("sender")
        .or_else(|| root.get("participant"))
        .and_then(|v| v.as_str())
        .unwrap_or(&chat_jid)
        .to_string();

    let sender_name = root
        .get("push_name")
        .or_else(|| root.get("sender_name"))
        .or_else(|| root.get("name"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let (text, extracted_media_type, is_audio_or_video) = extract_whatsmeow_content(root, val);

    let msg_id = root
        .get("id")
        .or_else(|| root.get("message_id"))
        .and_then(|v| v.as_str())
        .unwrap_or("unknown_id")
        .to_string();

    let is_from_me = root
        .get("is_from_me")
        .or_else(|| root.get("from_me"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let timestamp = root
        .get("timestamp")
        .and_then(|v| v.as_i64())
        .unwrap_or_else(chrono_now_secs);

    let chat_type = if chat_jid.ends_with("@g.us")
        || root
            .get("is_group")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    {
        ChatType::Group
    } else {
        ChatType::DirectMessage
    };

    let quoted_message = root
        .get("quoted_message")
        .or_else(|| root.get("context_info"))
        .and_then(|q| {
            let q_id = q.get("id").and_then(|v| v.as_str()).unwrap_or_default();
            let q_sender = q
                .get("participant")
                .or_else(|| q.get("sender"))
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            let q_text = q
                .get("body")
                .or_else(|| q.get("text"))
                .or_else(|| q.get("conversation"))
                .and_then(|v| v.as_str())
                .unwrap_or_default();

            if !q_text.is_empty() {
                Some(QuotedMessage {
                    id: q_id.to_string(),
                    sender_jid: q_sender.to_string(),
                    text: q_text.to_string(),
                })
            } else {
                None
            }
        });

    let mentioned_jids = root
        .get("mentioned_jids")
        .or_else(|| root.get("mentions"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|item| item.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    let is_bot_mentioned = root
        .get("is_bot_mentioned")
        .or_else(|| val.get("is_bot_mentioned"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let bot_lid = root
        .get("bot_lid")
        .or_else(|| val.get("bot_lid"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let raw_media_type = root
        .get("media_type")
        .or_else(|| val.get("media_type"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .or(extracted_media_type);

    let is_heavy_media = is_audio_or_video
        || raw_media_type.as_deref() == Some("audio")
        || raw_media_type.as_deref() == Some("video")
        || raw_media_type.as_deref() == Some("ptt")
        || root.get("mime_type").and_then(|v| v.as_str()).map(|s| s.starts_with("audio/") || s.starts_with("video/")).unwrap_or(false);

    let has_media = if is_heavy_media {
        false
    } else {
        root.get("has_media")
            .or_else(|| val.get("has_media"))
            .and_then(|v| v.as_bool())
            .or_else(|| {
                if root.get("download_url").is_some()
                    || root.get("media_url").is_some()
                    || val.get("download_url").is_some()
                    || val.get("media_url").is_some()
                    || root.get("media_base64").is_some()
                    || val.get("media_base64").is_some()
                {
                    Some(true)
                } else {
                    None
                }
            })
            .unwrap_or(false)
    };

    let media_type = raw_media_type;

    let is_bot_unassigned = bot_jid
        .map(|b| {
            let clean = b.trim().to_lowercase();
            clean.is_empty()
                || clean.starts_with("unassigned")
                || clean.starts_with("placeholder")
                || clean.starts_with("dummy")
                || clean.starts_with("6280000000000")
                || clean.starts_with("628123456789")
        })
        .unwrap_or(true);

    let explicit_role = query_params
        .and_then(|q| q.get("role").or_else(|| q.get("session_role")))
        .map(|s| s.as_str())
        .or_else(|| {
            root.get("session_role")
                .or_else(|| val.get("session_role"))
                .and_then(|v| v.as_str())
        });

    let session_id = query_params
        .and_then(|q| q.get("session_id").or_else(|| q.get("session")))
        .map(|s| s.as_str())
        .or_else(|| {
            root.get("session_id")
                .or_else(|| root.get("session"))
                .or_else(|| val.get("session_id"))
                .or_else(|| val.get("session"))
                .and_then(|v| v.as_str())
        });

    let to_jid = root
        .get("to")
        .or_else(|| root.get("receiver"))
        .or_else(|| root.get("recipient"))
        .and_then(|v| v.as_str());

    let session_role = if let Some(role_str) = explicit_role {
        if role_str.eq_ignore_ascii_case("user_companion") || role_str.eq_ignore_ascii_case("companion") {
            crate::core::domain::SessionRole::UserCompanion
        } else {
            crate::core::domain::SessionRole::PrimaryBot
        }
    } else if let (Some(sid), Some(comp_sid)) = (session_id, companion_session_id) {
        if sid.trim() == comp_sid.trim() {
            crate::core::domain::SessionRole::UserCompanion
        } else {
            crate::core::domain::SessionRole::PrimaryBot
        }
    } else if let (Some(to), Some(comp_jid)) = (to_jid, companion_jid) {
        let to_clean = to.split('@').next().unwrap_or(to);
        let comp_clean = comp_jid.split('@').next().unwrap_or(comp_jid);
        if to_clean == comp_clean {
            crate::core::domain::SessionRole::UserCompanion
        } else {
            crate::core::domain::SessionRole::PrimaryBot
        }
    } else if let Some(comp_jid) = companion_jid {
        let sender_clean = sender_jid.split('@').next().unwrap_or(&sender_jid);
        let comp_clean = comp_jid.split('@').next().unwrap_or(comp_jid);
        if is_from_me && sender_clean == comp_clean {
            crate::core::domain::SessionRole::UserCompanion
        } else if is_bot_unassigned {
            // If primary dedicated bot is not configured/unassigned, all traffic from this gateway belongs to the companion sensor
            crate::core::domain::SessionRole::UserCompanion
        } else {
            crate::core::domain::SessionRole::PrimaryBot
        }
    } else {
        crate::core::domain::SessionRole::PrimaryBot
    };

    Some(IncomingMessage {
        id: msg_id,
        platform: crate::core::domain::Platform::WhatsApp,
        session_role,
        chat_jid,
        chat_type,
        sender: Sender {
            jid: sender_jid,
            name: sender_name,
        },
        text,
        timestamp,
        is_from_me,
        quoted_message,
        mentioned_jids,
        is_bot_mentioned,
        bot_lid,
        has_media,
        media_type,
        media_path: None,
    })
}

