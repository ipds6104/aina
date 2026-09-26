//! Error diagnostics, quota recovery advice, and admin alerting policies.

use crate::core::domain::{ChatType, IncomingMessage, PersonaEngine, SessionRole};
use crate::core::ports::{AgentEnginePort, WhatsAppPort};
use std::sync::Arc;

pub struct ErrorHandler;

impl ErrorHandler {
    /// Formats a complete diagnostic alert message for the system administrator/companion.
    pub fn format_admin_alert(
        msg: &IncomingMessage,
        raw_err: &str,
        persona_engine: &PersonaEngine,
    ) -> String {
        let is_target_group_or_status =
            msg.chat_jid.ends_with("@g.us") || msg.chat_jid.contains("@broadcast");
        let chat_desc = if is_target_group_or_status {
            format!("Grup WhatsApp (`{}`)", msg.chat_jid)
        } else {
            format!("Obrolan Pribadi (`{}`)", msg.chat_jid)
        };

        let preview = if msg.text.len() > 120 {
            format!("{}...", &msg.text[..120])
        } else {
            msg.text.clone()
        };

        let sender_label = msg.sender.name.as_deref().unwrap_or("Pengguna");
        let time_str = persona_engine.current_local_time_string();

        format!(
            "🚨 *Laporan Kegagalan Pemrosesan Pesan Aina*\n\n\
             • *Ruang Obrolan*: {}\n\
             • *Pengirim*: {} (`{}`)\n\
             • *Waktu Kejadian*: {}\n\
             • *Pesan Pengirim*:\n\
             > {}\n\n\
             📋 *Rincian Lengkap Masalah / Error:*\n\
             ```\n{}\n```\n\n\
             💡 _Sesuai etika platform, rincian teknis kegagalan ini tidak dikirimkan ke ruang obrolan grup/publik dan hanya dilaporkan kepada Companion/Admin._",
            chat_desc,
            sender_label,
            msg.sender.jid,
            time_str,
            preview,
            raw_err.trim()
        )
    }

    /// Determines if an error string indicates quota exhaustion or rate limiting.
    pub fn is_quota_error(raw_err: &str) -> bool {
        let err_str = raw_err.to_lowercase();
        err_str.contains("503")
            || err_str.contains("429")
            || err_str.contains("quota")
            || err_str.contains("resource has been exhausted")
            || err_str.contains("rate limit")
    }

    /// Executes the failure reporting policy:
    /// - Quota explanation if applicable in DM
    /// - Sends detailed alert to admin
    /// - Never spams technical details to groups
    /// - Sends polite apology in private DM for non-admins
    pub async fn handle_execution_error(
        msg: &IncomingMessage,
        raw_err: &str,
        persona_engine: &PersonaEngine,
        agent_engine: &Arc<dyn AgentEnginePort>,
        whatsapp: &Arc<dyn WhatsAppPort>,
        quote_id: Option<&str>,
    ) {
        let admin_jid = persona_engine.admin_jid();
        let is_target_group_or_status =
            msg.chat_jid.ends_with("@g.us") || msg.chat_jid.contains("@broadcast");
        let is_sender_admin = !admin_jid.trim().is_empty()
            && (msg.sender.jid == admin_jid
                || msg.sender.jid.replace("@s.whatsapp.net", "")
                    == admin_jid.replace("@s.whatsapp.net", ""));

        let admin_alert = Self::format_admin_alert(msg, raw_err, persona_engine);
        let is_quota = Self::is_quota_error(raw_err);

        // 1. Quota handling: In private DM, send friendly quota guide
        if is_quota && msg.chat_type == ChatType::DirectMessage && !is_target_group_or_status {
            let pool_status = agent_engine.get_account_pool_status().await;
            let total_accs = pool_status.len();
            let friendly_quota = if total_accs <= 1 {
                "Aduh, kuota akses AI untuk akun saat ini lagi penuh/cooling down nih dari Google (429 Rate Limit).\n\n💡 *Solusi Cepat & Seamless:*\nAnda bisa menambahkan akun Pro cadangan agar Aina otomatis bergantian tanpa putus:\n1. Buka dashboard `/setup` di browser untuk paste token akun cadangan, ATAU\n2. Kirim token langsung di chat DM ini dengan perintah:\n   `/token {\"token\":\"...\"}`\n3. Atau pasang `AINA_OAUTH_TOKEN_2=...` di environment.\n\n_Konteks obrolan ini tersimpan aman dan tidak akan hilang!_".to_string()
            } else {
                "Aduh, seluruh akun AI di pool sedang cooling down dari Google. Tunggu sekitar 2-3 menit yaa, nanti salah satu akun akan otomatis aktif kembali!".to_string()
            };
            let _ = whatsapp
                .send_text_with_session(&msg.chat_jid, &friendly_quota, quote_id, msg.session_role)
                .await;
        }

        // 2. Error reporting per policy:
        // NEVER send technical errors to group or status broadcast.
        // ALWAYS report failure details to companion/admin.
        if is_sender_admin && !is_target_group_or_status {
            // Admin asked in DM: provide the full diagnostic failure directly in this chat!
            if !is_quota {
                let _ = whatsapp
                    .send_text_with_session(&msg.chat_jid, &admin_alert, quote_id, msg.session_role)
                    .await;
            }
        } else if !admin_jid.trim().is_empty() {
            // Non-admin DM or Group/Status: send private alert to admin
            let _ = whatsapp
                .send_text_with_session(admin_jid, &admin_alert, None, SessionRole::PrimaryBot)
                .await;

            // In non-admin private DM: provide a polite, non-technical reassurance
            if msg.chat_type == ChatType::DirectMessage && !is_target_group_or_status && !is_quota {
                let friendly_apology = "Maaf yaa, Aina sedang mengalami kendala teknis internal pada sistem. Pesan ini sudah otomatis dilaporkan ke admin untuk diperiksa. Mohon coba lagi beberapa saat lagi yaa 🙏";
                let _ = whatsapp
                    .send_text_with_session(&msg.chat_jid, friendly_apology, quote_id, msg.session_role)
                    .await;
            }
        }
    }
}
