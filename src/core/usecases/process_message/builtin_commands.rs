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

/// Evaluates if the incoming message is a local administrative / quick command.
/// Returns Some(outcome) if handled, or None if it should proceed to AI agent processing.
pub async fn try_handle_builtin_command(
    msg: &IncomingMessage,
    session_store: &Arc<dyn SessionStorePort>,
    agent_engine: &Arc<dyn AgentEnginePort>,
) -> Option<BuiltinCommandOutcome> {
    let trimmed_text = msg.text.trim();

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
            let reply = format!(
                "🤖 *Status Model AI Aina*\n\nModel aktif saat ini: *{}*\n\n*Pilihan Model Tersedia:*\n• `gemini-3.8-flash-medium` (Default Cepat & Seimbang)\n• `gemini-3.8-flash-high` (Penalaran Tinggi / Deep Thinking)\n• `gemini-3.8-flash-low` (Respons Kilat & Kasual)\n• `gemini-3.1-pro-high` (Deep Coding & Arsitektur)\n• `claude-opus-4-6-thinking` (Claude Opus Thinking - Khusus Eksplisit)\n• `claude-sonnet-4-6` (Claude Sonnet 4.6)\n\n_Untuk mengganti model, ketik:_ `/model <nama_model>`",
                current
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
                    format!("⏳ Cooldown (sisa {})", acc.formatted_cooldown())
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
