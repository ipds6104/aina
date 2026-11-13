//! Built-in WhatsApp commands handling (/reset, /model, /token, and media gatekeeper).

use crate::core::domain::{ChatType, IncomingMessage};
use crate::core::ports::{AgentEnginePort, SessionStorePort};
use std::sync::Arc;

pub struct BuiltinCommandOutcome {
    pub reply: String,
    pub tool_name: &'static str,
    pub usecase: &'static str,
    pub is_error: bool,
    pub error_detail: Option<String>,
}

fn is_admin_sender(msg: &IncomingMessage, admin_jid: &str) -> bool {
    if msg.is_from_me {
        return true;
    }
    if admin_jid.trim().is_empty() {
        return false;
    }
    let s_clean = msg.sender.jid.split('@').next().unwrap_or(&msg.sender.jid);
    let a_clean = admin_jid.split('@').next().unwrap_or(admin_jid);
    s_clean == a_clean
}

fn extract_target_jid(msg: &IncomingMessage, text: &str) -> Option<String> {
    if let Some(first_mention) = msg.mentioned_jids.first() {
        if !first_mention.trim().is_empty() {
            let jid = if first_mention.contains('@') {
                first_mention.clone()
            } else {
                format!("{}@s.whatsapp.net", first_mention)
            };
            return Some(jid);
        }
    }

    for word in text.split_whitespace() {
        let clean = word.trim_matches(|c: char| !c.is_numeric() && c != '@');
        if clean.starts_with('@') && clean.len() >= 9 {
            let num = &clean[1..];
            if num.chars().all(|c| c.is_numeric()) {
                return Some(format!("{}@s.whatsapp.net", num));
            }
        } else if clean.len() >= 9 && clean.chars().all(|c| c.is_numeric()) {
            return Some(format!("{}@s.whatsapp.net", clean));
        }
    }
    None
}

/// Evaluates if the incoming message is a local administrative / quick command.
/// Returns Some(outcome) if handled, or None if it should proceed to AI agent processing.
pub async fn try_handle_builtin_command(
    msg: &IncomingMessage,
    session_store: &Arc<dyn SessionStorePort>,
    agent_engine: &Arc<dyn AgentEnginePort>,
    admin_jid: &str,
) -> Option<BuiltinCommandOutcome> {
    let trimmed_text = msg.text.trim();
    let lower_text = trimmed_text.to_lowercase();

    // 0. Companion Delegation Commands (!aina izinkan, !aina cabut, !aina trust group, etc.)
    if lower_text.starts_with("!aina izinkan grup ini")
        || lower_text.starts_with("!aina authorize group")
        || lower_text.starts_with("!aina trust group")
    {
        if !is_admin_sender(msg, admin_jid) {
            return Some(BuiltinCommandOutcome {
                reply: "⚠️ *Akses Ditolak*\n\nHanya User Companion / Administrator (Bg Ihza) yang berwenang memverifikasi status kepercayaan grup.".to_string(),
                tool_name: "builtin:auth_group",
                usecase: "system_and_control",
                is_error: true,
                error_detail: Some("Unauthorized attempt to trust group".to_string()),
            });
        }

        let _ = session_store.set_group_trusted(&msg.chat_jid, true, Some(&msg.sender.jid)).await;
        return Some(BuiltinCommandOutcome {
            reply: "🛡️ *Grup Tim Kerja Terverifikasi*\n\nGrup ini sekarang telah diverifikasi sebagai grup tim kerja sah oleh Bg Ihza.\nAnggota di dalam grup ini diakui memiliki wewenang staf untuk tugas-tugas koordinasi kerja bersama Aina.".to_string(),
            tool_name: "builtin:auth_group",
            usecase: "system_and_control",
            is_error: false,
            error_detail: None,
        });
    }

    if lower_text.starts_with("!aina cabut grup ini")
        || lower_text.starts_with("!aina revoke group")
        || lower_text.starts_with("!aina untrust group")
    {
        if !is_admin_sender(msg, admin_jid) {
            return Some(BuiltinCommandOutcome {
                reply: "⚠️ *Akses Ditolak*\n\nHanya User Companion / Administrator (Bg Ihza) yang berwenang mencabut status kepercayaan grup.".to_string(),
                tool_name: "builtin:auth_group",
                usecase: "system_and_control",
                is_error: true,
                error_detail: Some("Unauthorized attempt to untrust group".to_string()),
            });
        }

        let _ = session_store.set_group_trusted(&msg.chat_jid, false, Some(&msg.sender.jid)).await;
        return Some(BuiltinCommandOutcome {
            reply: "🔒 *Status Kepercayaan Grup Dicabut*\n\nGrup ini kembali berstatus umum (Zero Implicit Trust).\nAnggota grup memerlukan izin personal untuk mengakses tugas-tugas staf internal.".to_string(),
            tool_name: "builtin:auth_group",
            usecase: "system_and_control",
            is_error: false,
            error_detail: None,
        });
    }

    if lower_text.starts_with("!aina izinkan") || lower_text.starts_with("!aina authorize") {
        if !is_admin_sender(msg, admin_jid) {
            return Some(BuiltinCommandOutcome {
                reply: "⚠️ *Akses Ditolak*\n\nHanya User Companion / Administrator (Bg Ihza) yang berwenang mendelegasikan hak akses rekan kerja.".to_string(),
                tool_name: "builtin:auth_user",
                usecase: "system_and_control",
                is_error: true,
                error_detail: Some("Unauthorized attempt to authorize user".to_string()),
            });
        }

        let target_opt = extract_target_jid(msg, trimmed_text);
        if let Some(target) = target_opt {
            let (authority, role) = if lower_text.contains("atasan") || lower_text.contains("pimpinan") {
                ("atasan", "Pimpinan / Atasan Organisasi")
            } else if lower_text.contains("admin") {
                ("admin", "Administrator Sistem")
            } else {
                ("staff", "Rekan Tim")
            };

            let _ = session_store.authorize_user(&target, role, authority, Some("Diizinkan oleh Companion via WhatsApp"), &msg.sender.jid).await;
            return Some(BuiltinCommandOutcome {
                reply: format!(
                    "✅ *Otorisasi Rekan Kerja Berhasil*\n\nPengguna *{}* telah diverifikasi sebagai *{}* (Tingkat Otoritas: *{}*) oleh Bg Ihza.\n\nWewenang ini aktif di grup ini dan berlaku penuh saat pengguna mengirim pesan pribadi (DM) ke Aina.",
                    target, role, authority.to_uppercase()
                ),
                tool_name: "builtin:auth_user",
                usecase: "system_and_control",
                is_error: false,
                error_detail: None,
            });
        } else {
            return Some(BuiltinCommandOutcome {
                reply: "⚠️ Format salah. Harap cantumkan mention (@nomor) atau nomor WhatsApp pengguna yang ingin diizinkan.\n\nContoh:\n`!aina izinkan @628123456789 sebagai staff`\n`!aina izinkan @628123456789 sebagai atasan`".to_string(),
                tool_name: "builtin:auth_user",
                usecase: "system_and_control",
                is_error: true,
                error_detail: Some("Missing target JID".to_string()),
            });
        }
    }

    if lower_text.starts_with("!aina cabut izin") || lower_text.starts_with("!aina revoke") {
        if !is_admin_sender(msg, admin_jid) {
            return Some(BuiltinCommandOutcome {
                reply: "⚠️ *Akses Ditolak*\n\nHanya User Companion / Administrator (Bg Ihza) yang berwenang mencabut izin rekan kerja.".to_string(),
                tool_name: "builtin:revoke_user",
                usecase: "system_and_control",
                is_error: true,
                error_detail: Some("Unauthorized attempt to revoke user".to_string()),
            });
        }

        let target_opt = extract_target_jid(msg, trimmed_text);
        if let Some(target) = target_opt {
            let _ = session_store.authorize_user(&target, "Tamu", "guest", Some("Wewenang dicabut oleh Companion"), &msg.sender.jid).await;
            return Some(BuiltinCommandOutcome {
                reply: format!(
                    "🔒 *Otorisasi Pengguna Dicabut*\n\nWewenang untuk *{}* telah dikembalikan ke status *Tamu (Guest)*.",
                    target
                ),
                tool_name: "builtin:revoke_user",
                usecase: "system_and_control",
                is_error: false,
                error_detail: None,
            });
        } else {
            return Some(BuiltinCommandOutcome {
                reply: "⚠️ Format salah. Harap cantumkan mention (@nomor) atau nomor WhatsApp pengguna yang ingin dicabut izinnya.\n\nContoh:\n`!aina cabut izin @628123456789`".to_string(),
                tool_name: "builtin:revoke_user",
                usecase: "system_and_control",
                is_error: true,
                error_detail: Some("Missing target JID".to_string()),
            });
        }
    }

    if lower_text.starts_with("!aina siapa") || lower_text.starts_with("!aina cek izin") || lower_text.starts_with("!aina status user") {
        let target = extract_target_jid(msg, trimmed_text).unwrap_or_else(|| msg.sender.jid.clone());
        if let Ok(Some(prof)) = session_store.get_user_profile(&target).await {
            let granter_str = prof.granted_by.as_deref().unwrap_or("Otomatis / Belum Ada");
            return Some(BuiltinCommandOutcome {
                reply: format!(
                    "👤 *Profil & Wewenang Pengguna*\n• *WhatsApp JID*: {}\n• *Nama*: {}\n• *Peran*: {}\n• *Otoritas*: *{}*\n• *Diberikan Oleh*: {}\n• *Catatan*: {}",
                    prof.sender_jid,
                    prof.name.as_deref().unwrap_or("-"),
                    prof.role.as_deref().unwrap_or("Tamu"),
                    prof.authority_level.to_uppercase(),
                    granter_str,
                    prof.notes.as_deref().unwrap_or("-")
                ),
                tool_name: "builtin:check_user",
                usecase: "system_and_control",
                is_error: false,
                error_detail: None,
            });
        } else {
            return Some(BuiltinCommandOutcome {
                reply: format!("ℹ️ Belum ada profil tersimpan untuk `{}` (status default: Tamu/Guest).", target),
                tool_name: "builtin:check_user",
                usecase: "system_and_control",
                is_error: false,
                error_detail: None,
            });
        }
    }

    // 1. Heavy Audio/Video messages skip
    if msg.text.starts_with("[Pesan Audio/Voice Note diabaikan")
        || msg.text.starts_with("[Pesan Video diabaikan")
    {
        return Some(BuiltinCommandOutcome {
            reply: "Maaf yaa, untuk saat ini Aina belum dapat memproses pesan audio/voice note atau video karena ukurannya yang berat. Silakan kirimkan dalam bentuk teks, dokumen, gambar, atau kartu kontak yaa! Terima kasih.".to_string(),
            tool_name: "builtin:media_gate",
            usecase: "media_and_whatsapp",
            is_error: false,
            error_detail: None,
        });
    }

    // 2. /reset, /clear, /new, /restart
    if trimmed_text.eq_ignore_ascii_case("/reset")
        || trimmed_text.eq_ignore_ascii_case("/clear")
        || trimmed_text.eq_ignore_ascii_case("/new")
        || trimmed_text.eq_ignore_ascii_case("/restart")
    {
        let _ = session_store.delete_conversation_id(&msg.chat_jid).await;
        let _ = std::fs::remove_file(std::env::temp_dir().join("aina_gh_device_session.json"));
        return Some(BuiltinCommandOutcome {
            reply: "🔄 *Sesi Percakapan Berhasil Direset*\n\nMemori konteks percakapan untuk ruang obrolan ini telah dibersihkan. Sesi berikutnya akan dimulai sebagai percakapan baru yang segar. Silakan ajukan pertanyaan atau instruksi baru Anda!".to_string(),
            tool_name: "builtin:reset",
            usecase: "system_and_control",
            is_error: false,
            error_detail: None,
        });
    }

    // 3. /model commands
    if trimmed_text.starts_with("/model") {
        let parts: Vec<&str> = trimmed_text.split_whitespace().collect();
        if parts.len() == 1 || (parts.len() >= 2 && (parts[1] == "status" || parts[1] == "list")) {
            let current = agent_engine.get_model().await;
            let catalog = agent_engine.list_models().await;
            let available = catalog.whatsapp_model_list(&current);
            let source_note = match catalog.source {
                "dynamic" => "_Sumber: Antigravity CLI (live)_".to_string(),
                "static_fallback" => "_Sumber: katalog bawaan (fallback)_".to_string(),
                _ => String::new(),
            };
            let reply = format!(
                "🤖 *Status Model AI Aina*\n\nModel aktif saat ini: *{}*\n\n*Pilihan Model Tersedia:*\n{}\n\n{} _Untuk mengganti model, ketik:_ `/model <nama_model>`",
                current, available, source_note
            );
            return Some(BuiltinCommandOutcome {
                reply,
                tool_name: "builtin:model",
                usecase: "system_and_control",
                is_error: false,
                error_detail: None,
            });
        } else if parts.len() >= 2 {
            let target_model = parts[1];
            return match agent_engine.set_model(target_model).await {
                Ok(_) => {
                    let new_model = agent_engine.get_model().await;
                    Some(BuiltinCommandOutcome {
                        reply: format!(
                            "✅ *Model AI Berhasil Diubah*\n\nAina sekarang menggunakan model: *{}*.\nRespons berikutnya akan diproses menggunakan mesin ini.",
                            new_model
                        ),
                        tool_name: "builtin:model",
                        usecase: "system_and_control",
                        is_error: false,
                        error_detail: None,
                    })
                }
                Err(e) => Some(BuiltinCommandOutcome {
                    reply: format!(
                        "⚠️ *Gagal Mengganti Model*\n\n{}\n\nContoh: `/model gemini-3.8-flash-medium`",
                        e
                    ),
                    tool_name: "builtin:model",
                    usecase: "system_and_control",
                    is_error: true,
                    error_detail: Some(e.to_string()),
                }),
            };
        }
    }

    // 4. /token, /auth, /account commands
    if trimmed_text.starts_with("/token") || trimmed_text.starts_with("/auth") || trimmed_text.starts_with("/account") {
        let parts: Vec<&str> = trimmed_text.split_whitespace().collect();
        let is_status = parts.len() == 1 || (parts.len() >= 2 && (parts[1] == "status" || parts[1] == "list" || parts[1] == "pool"));

        if is_status {
            let pool_status = agent_engine.get_account_pool_status().await;
            let mut status_lines = Vec::new();
            for acc in &pool_status {
                let state_str = if acc.is_cooldown {
                    let reason_str = match &acc.cooldown_reason {
                        Some(r) => format!(" - _{}_", r),
                        None => String::new(),
                    };
                    format!("⏳ Cooldown (sisa {}){}", acc.formatted_cooldown(), reason_str)
                } else {
                    "🟢 Aktif & Siap".to_string()
                };
                let email_str = match acc.masked_email() {
                    Some(e) => format!(" ({})", e),
                    None => String::new(),
                };
                status_lines.push(format!("• *#{} {}*{}: {}", acc.id, acc.label, email_str, state_str));
            }
            let list_str = if status_lines.is_empty() {
                "• _Belum ada akun di pool (menggunakan token file default)_".to_string()
            } else {
                status_lines.join("\n")
            };

            let reply = format!(
                "👥 *Status Pool Akun Antigravity Aina*\n\nTotal Akun Terdaftar: *{}*\n\n{}\n\n💡 *Perintah Kelola Akun:*\n• Tambah: `/token <oauth_json>`\n• Hapus: `/token remove <id>`\n• Kosongkan: `/token clear`\n• Dashboard Web: `/setup`",
                pool_status.len(),
                list_str
            );

            return Some(BuiltinCommandOutcome {
                reply,
                tool_name: "builtin:token",
                usecase: "system_and_control",
                is_error: false,
                error_detail: None,
            });
        }

        // Account pool clearing (enforce DM only)
        let is_clear = parts.len() >= 2 && parts[1] == "clear";
        if is_clear {
            if msg.chat_type != ChatType::DirectMessage {
                return Some(BuiltinCommandOutcome {
                    reply: "⚠️ *Demi Keamanan:* Perintah pengosongan akun HANYA boleh dikirim melalui Pesan Pribadi (DM) ke Aina.".to_string(),
                    tool_name: "builtin:token",
                    usecase: "system_and_control",
                    is_error: true,
                    error_detail: Some("Attempted /token clear in group chat".to_string()),
                });
            }

            return match agent_engine.clear_account_pool().await {
                Ok(count) => Some(BuiltinCommandOutcome {
                    reply: format!("🗑️ *Pool Akun Dikosongkan*\n\nSebanyak *{}* akun cadangan telah dihapus dari pool. Aina sekarang kembali menggunakan akun default.", count),
                    tool_name: "builtin:token",
                    usecase: "system_and_control",
                    is_error: false,
                    error_detail: None,
                }),
                Err(e) => Some(BuiltinCommandOutcome {
                    reply: format!("❌ Gagal mengosongkan pool akun: {}", e),
                    tool_name: "builtin:token",
                    usecase: "system_and_control",
                    is_error: true,
                    error_detail: Some(e.to_string()),
                }),
            };
        }

        // Account removal (enforce DM only)
        let is_remove = parts.len() >= 3 && (parts[1] == "remove" || parts[1] == "delete" || parts[1] == "rm" || parts[1] == "del");
        if is_remove {
            if msg.chat_type != ChatType::DirectMessage {
                return Some(BuiltinCommandOutcome {
                    reply: "⚠️ *Demi Keamanan:* Perintah penghapusan akun HANYA boleh dikirim melalui Pesan Pribadi (DM) ke Aina.".to_string(),
                    tool_name: "builtin:token",
                    usecase: "system_and_control",
                    is_error: true,
                    error_detail: Some("Attempted /token remove in group chat".to_string()),
                });
            }

            if let Ok(id) = parts[2].parse::<usize>() {
                return match agent_engine.remove_account(id).await {
                    Ok(true) => {
                        let pool = agent_engine.get_account_pool_status().await;
                        Some(BuiltinCommandOutcome {
                            reply: format!("🗑️ *Akun #{} Berhasil Dihapus*\n\nSisa akun aktif di pool: *{}* akun.", id, pool.len()),
                            tool_name: "builtin:token",
                            usecase: "system_and_control",
                            is_error: false,
                            error_detail: None,
                        })
                    }
                    Ok(false) => Some(BuiltinCommandOutcome {
                        reply: format!("⚠️ Akun dengan ID #{} tidak ditemukan di pool. Ketik `/token status` untuk melihat ID yang valid.", id),
                        tool_name: "builtin:token",
                        usecase: "system_and_control",
                        is_error: true,
                        error_detail: Some(format!("Account #{} not found", id)),
                    }),
                    Err(e) => Some(BuiltinCommandOutcome {
                        reply: format!("❌ Gagal menghapus akun #{}: {}", id, e),
                        tool_name: "builtin:token",
                        usecase: "system_and_control",
                        is_error: true,
                        error_detail: Some(e.to_string()),
                    }),
                };
            } else {
                return Some(BuiltinCommandOutcome {
                    reply: format!("⚠️ Format ID tidak valid: '{}'. Contoh: `/token remove 2`", parts[2]),
                    tool_name: "builtin:token",
                    usecase: "system_and_control",
                    is_error: true,
                    error_detail: Some("Invalid account ID format".to_string()),
                });
            }
        }

        if parts.len() >= 2 && (parts[1] == "remove" || parts[1] == "delete" || parts[1] == "rm" || parts[1] == "del") {
            return Some(BuiltinCommandOutcome {
                reply: "⚠️ Harap cantumkan ID akun yang ingin dihapus.\n\nContoh:\n`/token remove 2`\n\nKetik `/token status` untuk melihat daftar akun dan ID-nya.".to_string(),
                tool_name: "builtin:token",
                usecase: "system_and_control",
                is_error: true,
                error_detail: Some("Missing account ID argument".to_string()),
            });
        }

        // Adding / saving token (enforce DM only)
        if msg.chat_type != ChatType::DirectMessage {
            return Some(BuiltinCommandOutcome {
                reply: "⚠️ *Demi Keamanan:* Perintah pendaftaran token akun OAuth HANYA boleh dikirim melalui Pesan Pribadi (DM) ke Aina, dilarang di dalam grup kerja.".to_string(),
                tool_name: "builtin:token",
                usecase: "system_and_control",
                is_error: true,
                error_detail: Some("Attempted token registration in group chat".to_string()),
            });
        }

        let token_str = if let Some(stripped) = trimmed_text.strip_prefix("/token ") {
            stripped.trim()
        } else if let Some(stripped) = trimmed_text.strip_prefix("/auth ") {
            stripped.trim()
        } else if let Some(stripped) = trimmed_text.strip_prefix("/account add ") {
            stripped.trim()
        } else {
            ""
        };

        if token_str.is_empty() {
            return Some(BuiltinCommandOutcome {
                reply: "ℹ️ *Petunjuk Penggunaan Token:*\nUntuk menambahkan akun baru, ketik:\n`/token <oauth_json>`\n\nContoh:\n`/token {\"token\":\"...\"}`".to_string(),
                tool_name: "builtin:token",
                usecase: "system_and_control",
                is_error: false,
                error_detail: None,
            });
        }

        return match agent_engine.save_auth_token(token_str).await {
            Ok(_) => {
                let pool = agent_engine.get_account_pool_status().await;
                Some(BuiltinCommandOutcome {
                    reply: format!(
                        "✅ *Akun Antigravity Berhasil Ditambahkan!*\n\nAkun baru telah diverifikasi dan langsung aktif di dalam pool.\n• Total Akun di Pool: *{}*\n• Strategi Rotasi: *Round-Robin (Bergantian)*\n\nAina sekarang siap melanjutkan tugas tanpa gangguan kuota!",
                        pool.len()
                    ),
                    tool_name: "builtin:token",
                    usecase: "system_and_control",
                    is_error: false,
                    error_detail: None,
                })
            }
            Err(e) => Some(BuiltinCommandOutcome {
                reply: format!(
                    "❌ *Gagal Menyimpan Token:*\n{}\n\nPastikan format token berupa JSON yang valid dari file `antigravity-oauth-token`.",
                    e
                ),
                tool_name: "builtin:token",
                usecase: "system_and_control",
                is_error: true,
                error_detail: Some(e.to_string()),
            }),
        };
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::driven::SqliteSessionStore;
    use crate::core::domain::message::{ChatType, Platform, Sender, SessionRole};

    struct DummyAgent;
    #[async_trait::async_trait]
    impl AgentEnginePort for DummyAgent {
        async fn execute_with_model(
            &self,
            _: Option<&str>,
            _: &str,
            _: Option<&str>,
        ) -> anyhow::Result<crate::core::ports::AgentResponse> {
            Ok(crate::core::ports::AgentResponse {
                conversation_id: "test".into(),
                response_text: "ok".into(),
                duration_seconds: 0.1,
            })
        }
        async fn get_model(&self) -> String {
            "default".into()
        }
        async fn set_model(&self, _: &str) -> anyhow::Result<()> {
            Ok(())
        }
        async fn is_authenticated(&self) -> bool {
            true
        }
        async fn save_auth_token(&self, _: &str) -> anyhow::Result<()> {
            Ok(())
        }
    }

    fn make_msg(sender: &str, text: &str, is_from_me: bool, mentions: Vec<String>) -> IncomingMessage {
        IncomingMessage {
            id: "msg_test_1".into(),
            platform: Platform::WhatsApp,
            session_role: SessionRole::PrimaryBot,
            chat_jid: "12036399999@g.us".into(),
            chat_type: ChatType::Group,
            sender: Sender {
                jid: sender.to_string(),
                name: Some("Tester".into()),
            },
            text: text.to_string(),
            timestamp: 1726000000,
            quoted_message: None,
            mentioned_jids: mentions,
            is_bot_mentioned: true,
            bot_lid: None,
            is_from_me,
            has_media: false,
            media_type: None,
            media_path: None,
        }
    }

    #[tokio::test]
    async fn test_companion_can_trust_and_untrust_group() {
        let dir = std::env::temp_dir().join(format!("aina_test_trust_group_{}", rand::random::<u32>()));
        let db_file = dir.join("test.db");
        let store: Arc<dyn SessionStorePort> = Arc::new(SqliteSessionStore::new(&db_file).unwrap());
        let agent: Arc<dyn AgentEnginePort> = Arc::new(DummyAgent);
        let admin_jid = "6289625345646@s.whatsapp.net";

        let trust_msg = make_msg(admin_jid, "!aina izinkan grup ini", false, vec![]);
        let outcome = try_handle_builtin_command(&trust_msg, &store, &agent, admin_jid).await.unwrap();
        assert!(outcome.reply.contains("Grup Tim Kerja Terverifikasi"));
        assert!(!outcome.is_error);

        // Check store
        assert!(store.is_group_trusted("12036399999@g.us").await.unwrap());

        // Revoke
        let revoke_msg = make_msg(admin_jid, "!aina cabut grup ini", false, vec![]);
        let outcome_rev = try_handle_builtin_command(&revoke_msg, &store, &agent, admin_jid).await.unwrap();
        assert!(outcome_rev.reply.contains("Status Kepercayaan Grup Dicabut"));
        assert!(!store.is_group_trusted("12036399999@g.us").await.unwrap());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_non_companion_cannot_trust_group() {
        let dir = std::env::temp_dir().join(format!("aina_test_unauth_trust_{}", rand::random::<u32>()));
        let db_file = dir.join("test.db");
        let store: Arc<dyn SessionStorePort> = Arc::new(SqliteSessionStore::new(&db_file).unwrap());
        let agent: Arc<dyn AgentEnginePort> = Arc::new(DummyAgent);
        let admin_jid = "6289625345646@s.whatsapp.net";

        let stranger_msg = make_msg("628111222333@s.whatsapp.net", "!aina izinkan grup ini", false, vec![]);
        let outcome = try_handle_builtin_command(&stranger_msg, &store, &agent, admin_jid).await.unwrap();
        assert!(outcome.reply.contains("Akses Ditolak"));
        assert!(outcome.is_error);
        assert!(!store.is_group_trusted("12036399999@g.us").await.unwrap());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_companion_can_authorize_and_revoke_user() {
        let dir = std::env::temp_dir().join(format!("aina_test_auth_user_{}", rand::random::<u32>()));
        let db_file = dir.join("test.db");
        let store: Arc<dyn SessionStorePort> = Arc::new(SqliteSessionStore::new(&db_file).unwrap());
        let agent: Arc<dyn AgentEnginePort> = Arc::new(DummyAgent);
        let admin_jid = "6289625345646@s.whatsapp.net";
        let target_jid = "6287711223344@s.whatsapp.net";

        let auth_msg = make_msg(admin_jid, &format!("!aina izinkan @{}", target_jid), false, vec![target_jid.to_string()]);
        let outcome = try_handle_builtin_command(&auth_msg, &store, &agent, admin_jid).await.unwrap();
        assert!(outcome.reply.contains("Otorisasi Rekan Kerja Berhasil"));
        assert!(!outcome.is_error);

        let prof = store.get_user_profile(target_jid).await.unwrap().unwrap();
        assert_eq!(prof.authority_level, "staff");
        assert_eq!(prof.granted_by.as_deref(), Some(admin_jid));

        // Revoke
        let rev_msg = make_msg(admin_jid, &format!("!aina cabut izin @{}", target_jid), false, vec![target_jid.to_string()]);
        let outcome_rev = try_handle_builtin_command(&rev_msg, &store, &agent, admin_jid).await.unwrap();
        assert!(outcome_rev.reply.contains("Otorisasi Pengguna Dicabut"));

        let prof_rev = store.get_user_profile(target_jid).await.unwrap().unwrap();
        assert_eq!(prof_rev.authority_level, "guest");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
