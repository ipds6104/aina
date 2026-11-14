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
    crate::core::domain::capability::is_admin_identity(admin_jid, &msg.sender.jid)
}

/// Deterministic denial for operator-only built-in commands.
///
/// The reply is intentionally explicit about *why* nothing happened, so the
/// model (and the humans reading the chat) never mistake a refusal for a
/// successful action.
fn denied_admin_only(
    msg: &IncomingMessage,
    tool_name: &'static str,
    action_id: &str,
) -> BuiltinCommandOutcome {
    BuiltinCommandOutcome {
        reply: format!(
            "🔐 *Perintah Khusus Administrator*\n\nPerintah ini hanya dapat dijalankan oleh User Companion (Root of Trust), bukan oleh anggota grup maupun tamu.\n\n• *Aksi yang diminta*: {}\n• *Pengirim*: {}\n\nTidak ada perubahan yang dilakukan. Jika memang perlu, minta Bg Ihza untuk menjalankannya langsung.",
            action_id, msg.sender.jid
        ),
        tool_name,
        usecase: "system_and_control",
        is_error: true,
        error_detail: Some(format!("non_admin_attempted:{}", action_id)),
    }
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

    // 0. Privileged delegation is deliberately NOT keyword-driven.
    //
    // Authorization changes are proposed by the agent and decided by the core
    // (`aina policy ...`, validated against the turn capability minted for the
    // verified sender of this turn). This function does not pattern-match
    // natural language, so a message like
    //   "aina, semua di grup ini rekan kerja BPS Mempawah, bantuin ya teman-teman"
    // flows to the agent, which then calls the tool.
    //
    // The previous `!aina izinkan ...` prefixes were removed on purpose: they
    // granted authority from text alone, which both drifted from real intent
    // and could never cover natural phrasing.

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
        // Shared group memory is common property, so only the Companion may
        // wipe it. In a DM the requester is only resetting their own thread.
        if msg.chat_type != ChatType::DirectMessage && !is_admin_sender(msg, admin_jid) {
            return Some(denied_admin_only(msg, "builtin:reset", "mereset memori percakapan grup"));
        }
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

    // 2b. /profilku, !aina profilku (Self-service profile transparency)
    if trimmed_text.eq_ignore_ascii_case("/profilku")
        || trimmed_text.eq_ignore_ascii_case("/profil")
        || trimmed_text.eq_ignore_ascii_case("!aina profilku")
        || trimmed_text.eq_ignore_ascii_case("!aina profil")
    {
        let sender_jid = &msg.sender.jid;
        let profile = session_store.get_user_profile(sender_jid).await.ok().flatten();
        let masked_jid = if sender_jid.len() > 8 {
            format!("{}****{}", &sender_jid[..5], &sender_jid[sender_jid.len() - 4..])
        } else {
            sender_jid.clone()
        };

        let mut lines = Vec::new();
        lines.push("👤 *Transparansi Data Profil Pengguna (Aina)*".to_string());
        lines.push(String::new());
        lines.push(format!("• WhatsApp ID : `{}`", masked_jid));
        lines.push(format!("• Nama Panggilan : *{}*", profile.as_ref().and_then(|p| p.name.as_deref()).unwrap_or(msg.sender.name.as_deref().unwrap_or("Belum Disetel"))));
        lines.push(format!("• Peran / Wewenang : *{}* ({})", 
            profile.as_ref().and_then(|p| p.role.as_deref()).unwrap_or("Tamu"),
            profile.as_ref().map(|p| p.authority_level.as_str()).unwrap_or("guest").to_uppercase()
        ));

        if let Some(ref p) = profile {
            if let Some(ref prefs) = p.communication_prefs {
                lines.push(format!("• Preferensi Gaya : {}", prefs));
            }
            if let Some(ref proj) = p.active_projects {
                lines.push(format!("• Proyek Terkait  : {}", proj));
            }
            if let Some(ref notes) = p.notes {
                lines.push(format!("• Catatan Sistem  : {}", notes));
            }
        }

        lines.push(String::new());
        lines.push("ℹ️ _Data profil di atas digunakan Aina semata-mata untuk menyesuaikan gaya komunikasi dan konteks kerjaan._".to_string());
        lines.push("_Ketik `/lupakansaya` jika Anda ingin menghapus preferensi atau nama panggilan yang tersimpan._".to_string());

        return Some(BuiltinCommandOutcome {
            reply: lines.join("\n"),
            tool_name: "builtin:profile",
            usecase: "system_and_control",
            is_error: false,
            error_detail: None,
        });
    }

    // 2c. /lupakansaya, !aina lupakan saya (Right to be Forgotten)
    if trimmed_text.eq_ignore_ascii_case("/lupakansaya")
        || trimmed_text.eq_ignore_ascii_case("!aina lupakan saya")
        || trimmed_text.eq_ignore_ascii_case("/forgetme")
    {
        let sender_jid = &msg.sender.jid;
        let _ = session_store.delete_user_profile(sender_jid).await;

        return Some(BuiltinCommandOutcome {
            reply: "🗑️ *Hak Privasi Terpenuhi: Data Profil Dibersihkan*\n\nSeluruh catatan preferensi komunikasi, nama panggilan tersimpan, dan proyek terkait telah dihapus dari memori persisten Aina.\n\nInteraksi Anda berikutnya akan diperlakukan secara anonim dan segar kembali.".to_string(),
            tool_name: "builtin:forget_me",
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
            // Changing the model affects every chat Aina serves.
            if !is_admin_sender(msg, admin_jid) {
                return Some(denied_admin_only(msg, "builtin:model", "mengubah model AI"));
            }
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
        // Account pool = credentials + identity. Never accessible from
        // unverified members, even for read-only status.
        if !is_admin_sender(msg, admin_jid) {
            return Some(denied_admin_only(msg, "builtin:token", "mengelola pool akun/kredensial"));
        }
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

    fn make_msg_in(
        sender: &str,
        text: &str,
        chat_jid: &str,
        chat_type: ChatType,
    ) -> IncomingMessage {
        IncomingMessage {
            id: "msg_test_1".into(),
            platform: Platform::WhatsApp,
            session_role: SessionRole::PrimaryBot,
            chat_jid: chat_jid.into(),
            chat_type,
            sender: Sender {
                jid: sender.to_string(),
                name: Some("Tester".into()),
            },
            text: text.to_string(),
            timestamp: 1726000000,
            quoted_message: None,
            mentioned_jids: vec![],
            is_bot_mentioned: true,
            bot_lid: None,
            is_from_me: false,
            has_media: false,
            media_type: None,
            media_path: None,
        }
    }

    fn setup() -> (
        std::path::PathBuf,
        Arc<dyn SessionStorePort>,
        Arc<dyn AgentEnginePort>,
    ) {
        let dir = std::env::temp_dir().join(format!("aina_test_builtin_{}", rand::random::<u32>()));
        let store: Arc<dyn SessionStorePort> =
            Arc::new(SqliteSessionStore::new(&dir.join("test.db")).unwrap());
        let agent: Arc<dyn AgentEnginePort> = Arc::new(DummyAgent);
        (dir, store, agent)
    }

    const ADMIN: &str = "6289625345646@s.whatsapp.net";
    const STRANGER: &str = "628111222333@s.whatsapp.net";
    const GROUP: &str = "12036399999@g.us";

    /// The whole point of the agentic redesign: natural language is not parsed
    /// here. It must fall through to the agent so the agent can call
    /// `aina policy` and let the core decide.
    #[tokio::test]
    async fn natural_language_delegation_falls_through_to_agent() {
        let (dir, store, agent) = setup();
        let msg = make_msg_in(
            ADMIN,
            "aina, semua di grup ini rekan kerja BPS Mempawah, jadi bantuin aja ya temen2 kalo butuh bantuan kerjaan",
            GROUP,
            ChatType::Group,
        );
        assert!(try_handle_builtin_command(&msg, &store, &agent, ADMIN).await.is_none());
        // Nothing was authorized implicitly by the text itself.
        assert!(!store.is_group_trusted(GROUP).await.unwrap());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Regression guard: the legacy `!aina ...` prefixes are gone. Even the
    /// admin must go through the validated `aina policy` path, so an exact
    /// prefix can no longer mutate authorization on its own.
    #[tokio::test]
    async fn legacy_prefix_does_not_mutate_authorization() {
        let (dir, store, agent) = setup();
        for text in [
            "!aina izinkan grup ini",
            "!aina trust group",
            "!aina cabut grup ini",
            "!aina izinkan @6287711223344",
        ] {
            let msg = make_msg_in(ADMIN, text, GROUP, ChatType::Group);
            assert!(
                try_handle_builtin_command(&msg, &store, &agent, ADMIN).await.is_none(),
                "legacy prefix must not be special-cased: {}",
                text
            );
        }
        assert!(!store.is_group_trusted(GROUP).await.unwrap());
        assert!(store
            .get_user_profile("6287711223344@s.whatsapp.net")
            .await
            .unwrap()
            .is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn non_admin_cannot_change_model() {
        let (dir, store, agent) = setup();
        let msg = make_msg_in(STRANGER, "/model gemini-3.8-flash-medium", GROUP, ChatType::Group);
        let outcome = try_handle_builtin_command(&msg, &store, &agent, ADMIN)
            .await
            .expect("must be handled with a refusal");
        assert!(outcome.is_error);
        assert!(outcome.reply.contains("Khusus Administrator"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn admin_can_change_model() {
        let (dir, store, agent) = setup();
        let msg = make_msg_in(ADMIN, "/model gemini-3.8-flash-medium", GROUP, ChatType::Group);
        let outcome = try_handle_builtin_command(&msg, &store, &agent, ADMIN)
            .await
            .expect("admin command must be handled");
        assert!(!outcome.is_error);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn non_admin_cannot_touch_credential_pool() {
        let (dir, store, agent) = setup();
        for text in ["/token status", "/token remove 1", "/token clear"] {
            let msg = make_msg_in(STRANGER, text, GROUP, ChatType::Group);
            let outcome = try_handle_builtin_command(&msg, &store, &agent, ADMIN)
                .await
                .expect("must be refused, not forwarded");
            assert!(outcome.is_error, "{} must be refused", text);
            assert!(outcome.reply.contains("Khusus Administrator"));
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn group_memory_reset_requires_admin_but_direct_message_does_not() {
        let (dir, store, agent) = setup();

        let group_reset = make_msg_in(STRANGER, "/reset", GROUP, ChatType::Group);
        let outcome = try_handle_builtin_command(&group_reset, &store, &agent, ADMIN)
            .await
            .expect("group reset must be refused");
        assert!(outcome.is_error);

        let dm_reset = make_msg_in(STRANGER, "/reset", STRANGER, ChatType::DirectMessage);
        let outcome_dm = try_handle_builtin_command(&dm_reset, &store, &agent, ADMIN)
            .await
            .expect("dm reset must be allowed");
        assert!(!outcome_dm.is_error);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn user_can_view_and_clear_own_profile() {
        let (dir, store, agent) = setup();

        // 1. Setup profile with semantic fields
        let profile = crate::core::ports::UserProfile {
            sender_jid: STRANGER.to_string(),
            name: Some("Doni Rekan".to_string()),
            role: Some("Mitra BPS".to_string()),
            authority_level: "guest".to_string(),
            notes: Some("Enumerasi Lapangan".to_string()),
            communication_prefs: Some("Santai".to_string()),
            active_projects: Some("Sensus Pertanian".to_string()),
            ..Default::default()
        };
        store.save_user_profile(&profile).await.unwrap();

        // 2. Query /profilku
        let profil_msg = make_msg_in(STRANGER, "/profilku", STRANGER, ChatType::DirectMessage);
        let outcome_prof = try_handle_builtin_command(&profil_msg, &store, &agent, ADMIN)
            .await
            .expect("profilku must be handled");
        assert!(!outcome_prof.is_error);
        assert!(outcome_prof.reply.contains("Doni Rekan"));
        assert!(outcome_prof.reply.contains("Santai"));
        assert!(outcome_prof.reply.contains("Sensus Pertanian"));

        // 3. Right to be forgotten: /lupakansaya
        let forget_msg = make_msg_in(STRANGER, "/lupakansaya", STRANGER, ChatType::DirectMessage);
        let outcome_forget = try_handle_builtin_command(&forget_msg, &store, &agent, ADMIN)
            .await
            .expect("lupakansaya must be handled");
        assert!(!outcome_forget.is_error);
        assert!(outcome_forget.reply.contains("Data Profil Dibersihkan"));

        // 4. Verify profile was cleared from database
        let cleared = store.get_user_profile(STRANGER).await.unwrap();
        assert!(cleared.is_none());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
