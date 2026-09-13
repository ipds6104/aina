use crate::core::domain::{
    ChatType, Gatekeeper, GatekeeperDecision, IncomingMessage, PersonaEngine, PresenceState,
};
use crate::core::ports::{AgentEnginePort, SessionStorePort, WhatsAppPort};
use std::sync::Arc;
use tracing::{error, info, warn};

pub struct ProcessIncomingMessageUseCase {
    session_store: Arc<dyn SessionStorePort>,
    agent_engine: Arc<dyn AgentEnginePort>,
    whatsapp: Arc<dyn WhatsAppPort>,
    persona_engine: Arc<PersonaEngine>,
    bot_jid: String,
    bot_name: String,
    bot_lid: Option<String>,
}

impl ProcessIncomingMessageUseCase {
    pub fn new(
        session_store: Arc<dyn SessionStorePort>,
        agent_engine: Arc<dyn AgentEnginePort>,
        whatsapp: Arc<dyn WhatsAppPort>,
        persona_engine: Arc<PersonaEngine>,
        bot_jid: String,
        bot_name: String,
        bot_lid: Option<String>,
    ) -> Self {
        Self {
            session_store,
            agent_engine,
            whatsapp,
            persona_engine,
            bot_jid,
            bot_name,
            bot_lid,
        }
    }

    pub async fn execute(&self, msg: IncomingMessage) -> anyhow::Result<()> {
        let decision = Gatekeeper::evaluate(&msg, &self.bot_jid, &self.bot_name, self.bot_lid.as_deref());

        match decision {
            GatekeeperDecision::Ignore { reason } => {
                info!("Ignoring message {}: {}", msg.id, reason);
                Ok(())
            }
            GatekeeperDecision::RecordOnly { reason } => {
                info!(
                    "Recording ambient group message from {} in {}: {}",
                    msg.sender.jid, msg.chat_jid, reason
                );
                self.session_store
                    .record_message(&msg.chat_jid, &msg.sender.jid, &msg.text, false)
                    .await?;
                Ok(())
            }
            GatekeeperDecision::Respond { reason } => {
                info!(
                    "Responding to message from {} in {}: {}",
                    msg.sender.jid, msg.chat_jid, reason
                );

                // 1. Record incoming message
                self.session_store
                    .record_message(&msg.chat_jid, &msg.sender.jid, &msg.text, false)
                    .await?;

                // Check for built-in quick command: /reset, /clear, /new
                let trimmed_text = msg.text.trim();
                if trimmed_text.eq_ignore_ascii_case("/reset")
                    || trimmed_text.eq_ignore_ascii_case("/clear")
                    || trimmed_text.eq_ignore_ascii_case("/new")
                    || trimmed_text.eq_ignore_ascii_case("/restart")
                {
                    let _ = self.session_store.delete_conversation_id(&msg.chat_jid).await;
                    let _ = std::fs::remove_file(std::env::temp_dir().join("aina_gh_device_session.json"));
                    let reply = "🔄 *Sesi Percakapan Berhasil Direset*\n\nMemori konteks percakapan untuk ruang obrolan ini telah dibersihkan. Sesi berikutnya akan dimulai sebagai percakapan baru yang segar. Silakan ajukan pertanyaan atau instruksi baru Anda!".to_string();
                    self.session_store.record_message(&msg.chat_jid, &self.bot_jid, &reply, true).await?;
                    self.whatsapp.send_text_with_session(&msg.chat_jid, &reply, Some(&msg.id), msg.session_role).await?;
                    return Ok(());
                }

                // Check for built-in quick command: /model
                if trimmed_text.starts_with("/model") {
                    let parts: Vec<&str> = trimmed_text.split_whitespace().collect();
                    if parts.len() == 1 || (parts.len() >= 2 && (parts[1] == "status" || parts[1] == "list")) {
                        let current = self.agent_engine.get_model().await;
                        let reply = format!(
                            "🤖 *Status Model AI Aina*\n\nModel aktif saat ini: *{}*\n\n*Pilihan Model Tersedia:*\n• `gemini-3.8-flash-medium` (Default Cepat & Seimbang)\n• `gemini-3.8-flash-high` (Penalaran Tinggi / Deep Thinking)\n• `gemini-3.8-flash-low` (Respons Kilat & Kasual)\n• `gemini-3.1-pro-high` (Deep Coding & Arsitektur)\n• `claude-opus-4-6-thinking` (Claude Opus Thinking - Khusus Eksplisit)\n• `claude-sonnet-4-6` (Claude Sonnet 4.6)\n\n_Untuk mengganti model, ketik:_ `/model <nama_model>`",
                            current
                        );
                        self.session_store.record_message(&msg.chat_jid, &self.bot_jid, &reply, true).await?;
                        self.whatsapp.send_text_with_session(&msg.chat_jid, &reply, Some(&msg.id), msg.session_role).await?;
                        return Ok(());
                    } else if parts.len() >= 2 {
                        let target_model = parts[1];
                        match self.agent_engine.set_model(target_model).await {
                            Ok(_) => {
                                let new_model = self.agent_engine.get_model().await;
                                let reply = format!(
                                    "✅ *Model AI Berhasil Diubah*\n\nAina sekarang menggunakan model: *{}*.\nRespons berikutnya akan diproses menggunakan mesin ini.",
                                    new_model
                                );
                                self.session_store.record_message(&msg.chat_jid, &self.bot_jid, &reply, true).await?;
                                self.whatsapp.send_text_with_session(&msg.chat_jid, &reply, Some(&msg.id), msg.session_role).await?;
                                return Ok(());
                            }
                            Err(e) => {
                                let reply = format!(
                                    "⚠️ *Gagal Mengganti Model*\n\n{}\n\nContoh: `/model gemini-3.8-flash-medium`",
                                    e
                                );
                                self.session_store.record_message(&msg.chat_jid, &self.bot_jid, &reply, true).await?;
                                self.whatsapp.send_text_with_session(&msg.chat_jid, &reply, Some(&msg.id), msg.session_role).await?;
                                return Ok(());
                            }
                        }
                    }
                }

                // 2. Send 'typing...' indicator immediately
                if let Err(e) = self
                    .whatsapp
                    .send_presence_with_session(&msg.chat_jid, PresenceState::Composing, msg.session_role)
                    .await
                {
                    warn!("Failed to send typing presence: {}", e);
                }

                // 3. Retrieve conversation ID for this chat
                let existing_conv_id = self
                    .session_store
                    .get_conversation_id(&msg.chat_jid)
                    .await?;

                // 4. Retrieve or auto-seed sender profile from profiling memory
                let profile = match self.session_store.get_user_profile(&msg.sender.jid).await {
                    Ok(Some(p)) => Some(p),
                    Ok(None) => {
                        let new_profile = crate::core::ports::UserProfile {
                            sender_jid: msg.sender.jid.clone(),
                            name: msg.sender.name.clone(),
                            role: Some("Rekan Kerja".to_string()),
                            authority_level: "staff".to_string(),
                            notes: Some("Terdaftar otomatis saat interaksi pertama".to_string()),
                        };
                        let _ = self.session_store.save_user_profile(&new_profile).await;
                        Some(new_profile)
                    }
                    Err(e) => {
                        warn!("Failed to fetch user profile for {}: {}", msg.sender.jid, e);
                        None
                    }
                };

                // 5. Build prompt incorporating persona, organization context, and profiling
                let prompt = self.persona_engine.build_prompt(&msg, profile.as_ref());

                // Start async presence heartbeat + fast ack timer if execution takes long
                let whatsapp = Arc::clone(&self.whatsapp);
                let chat_jid = msg.chat_jid.clone();
                let session_role = msg.session_role;

                let quote_id = match msg.chat_type {
                    ChatType::Group => Some(msg.id.as_str()),
                    ChatType::DirectMessage => None,
                };

                // Background typing heartbeat: keep "sedang mengetik..." active until response completes
                let heartbeat_handle = tokio::spawn(async move {
                    loop {
                        tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;

                        // Keep typing presence alive on WhatsApp
                        let _ = whatsapp
                            .send_presence_with_session(&chat_jid, PresenceState::Composing, session_role)
                            .await;
                    }
                });

                // 6. Execute Antigravity agent CLI
                let agent_res = match self
                    .agent_engine
                    .execute(existing_conv_id.as_deref(), &prompt)
                    .await
                {
                    Ok(res) => {
                        heartbeat_handle.abort();
                        res
                    }
                    Err(e) => {
                        heartbeat_handle.abort();
                        error!("Agent engine failed to execute: {}", e);
                        let err_reply = "Maaf, terjadi kesalahan saat memproses permintaan Anda. Silakan coba sesaat lagi.";
                        let _ = self
                            .whatsapp
                            .send_text_with_session(&msg.chat_jid, err_reply, quote_id, msg.session_role)
                            .await;
                        return Err(e);
                    }
                };

                // 6. Save or update conversation mapping
                if existing_conv_id.as_deref() != Some(&agent_res.conversation_id) {
                    self.session_store
                        .save_conversation_id(&msg.chat_jid, &agent_res.conversation_id)
                        .await?;
                }

                // Check if agent mistakenly executed wa_tool.py send-text AND outputted an internal report
                let trimmed_res = agent_res.response_text.trim();
                let is_redundant_report = trimmed_res.starts_with("Pesan balasan sudah terkirim")
                    || trimmed_res.starts_with("Pesan tanggapan telah berhasil dikirim")
                    || trimmed_res.starts_with("Pesan telah berhasil dikirim")
                    || trimmed_res.starts_with("Pesan berhasil dikirim");

                if is_redundant_report {
                    info!(
                        "Suppressed redundant agent tool confirmation report to prevent double-posting: {}",
                        trimmed_res
                    );
                } else {
                    // 7. Record bot reply locally
                    self.session_store
                        .record_message(&msg.chat_jid, &self.bot_jid, &agent_res.response_text, true)
                        .await?;

                    // 8. Send reply back to WhatsApp
                    self.whatsapp
                        .send_text_with_session(&msg.chat_jid, &agent_res.response_text, quote_id, msg.session_role)
                        .await?;
                }

                // 9. Reset presence
                let _ = self
                    .whatsapp
                    .send_presence_with_session(&msg.chat_jid, PresenceState::Paused, msg.session_role)
                    .await;

                info!(
                    "Successfully replied to {} in {:.2}s",
                    msg.chat_jid, agent_res.duration_seconds
                );
                Ok(())
            }
        }
    }
}

/// Returns a fast, natural, pre-curated interim acknowledgment phrase in Indonesian
/// texting style with natural variations, preventing robotic monotony.
#[allow(dead_code)]
pub fn pick_interim_ack(msg_id: &str, text: &str) -> String {
    let lower = text.to_lowercase();
    let is_search_or_check = lower.contains("cek")
        || lower.contains("cari")
        || lower.contains("liat")
        || lower.contains("lihat")
        || lower.contains("baca");

    let seed: usize = msg_id.bytes().fold(0usize, |acc, b| acc.wrapping_add(b as usize))
        + (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| (d.subsec_micros() / 1000) as usize)
            .unwrap_or(0));

    if is_search_or_check {
        let search_options = [
            "sebentarr, lagi kucariin yaa...",
            "siaapp, otw dicek dulu yaa...",
            "otw dicek dulu yaa, sebentarr...",
            "sebentarr yaa, lagi dibuka catatannya...",
            "okeiss, lagi ditelusuri sebentarr...",
            "okee sebentarr yaa, lagi dicek...",
        ];
        search_options[seed % search_options.len()].to_string()
    } else {
        let general_options = [
            "okee sebentarr yaa...",
            "siaapp, sebentarr yaa...",
            "okeiss, tunggu sebentarr yaa...",
            "otw diproses dulu yaa, sebentarr...",
            "okee, sebentarr kusiapkan dulu...",
            "siapp sebentarr yaa...",
            "sebentarr yaa...",
            "okeiss, otw yaa sebentarr...",
        ];
        general_options[seed % general_options.len()].to_string()
    }
}

/// Evaluates whether an incoming message is a substantive, actionable task
/// that warrants an interim acknowledgment if it takes longer than 8 seconds.
/// Greetings, short pings, acknowledgments, introductions, gratitude, and casual social chit-chat are strictly excluded.
#[allow(dead_code)]
pub fn should_send_interim_ack(text: &str) -> bool {
    let lower = text.trim().to_lowercase();
    if lower.is_empty() {
        return false;
    }

    // 1. Fast reject commands (like /model, /reset, /clear)
    if lower.starts_with('/') || lower.starts_with('!') {
        return false;
    }

    // 2. Reject greetings, pings, thanks, introductions, and casual social chit-chat
    let social_phrases = [
        "halo", "hai", "hei", "hey", "p", "ping", "aina",
        "pagi", "siang", "sore", "malam",
        "assalamualaikum", "assalamu'alaikum", "assalamu alaikum",
        "tes", "test", "testing", "ok", "oke", "okee", "okeis", "okeiss", "sip", "sipp", "siap", "siapp",
        "makasih", "terimakasih", "terima kasih", "thanks", "thx", "tq",
        "sama sama", "sama-sama",
        "apa kabar", "gimana kabar", "lagi apa",
        "salam kenal", "kenalan",
        "wkwk", "wkwkwk", "haha", "hahaha", "hehe", "hehehe", "plis", "please",
    ];

    let stripped = lower
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect::<String>();
    let clean = stripped.trim();

    // Check for obvious introductory / gratitude / laugh phrases without task verbs
    let is_intro_or_thanks = clean.contains("salam kenal")
        || clean.contains("makasih")
        || clean.contains("terima kasih")
        || clean.contains("thanks")
        || clean.contains("sama sama")
        || clean.contains("wkwk")
        || clean.contains("haha")
        || clean.contains("hehe")
        || clean.contains("kaget")
        || clean.starts_with("aku ")
        || clean.starts_with("saya ");

    let task_keywords = [
        "cek", "cari", "buat", "bikin", "tolong", "bisa", "apa", "kenapa",
        "gimana", "bagaimana", "run", "script", "log", "analisis", "hitung", "bantu",
        "olah", "rekap", "data", "excel", "csv", "coding", "debug", "perbaiki", "ubah",
    ];

    let has_task_keyword = task_keywords.iter().any(|k| lower.contains(k));

    if is_intro_or_thanks && !has_task_keyword {
        return false;
    }

    for g in &social_phrases {
        if clean == *g
            || clean == format!("halo {}", g)
            || clean == format!("hai {}", g)
            || clean == format!("selamat {}", g)
        {
            return false;
        }
    }

    // If message contains NO actionable task keywords and is casual/short (<= 7 words), do not ack
    let words: Vec<&str> = lower.split_whitespace().collect();
    if words.len() <= 7 && !has_task_keyword {
        return false;
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_not_ack_greetings_and_pings() {
        assert!(!should_send_interim_ack("Halo aina"));
        assert!(!should_send_interim_ack("halo"));
        assert!(!should_send_interim_ack("hai"));
        assert!(!should_send_interim_ack("@Aina"));
        assert!(!should_send_interim_ack("Aina"));
        assert!(!should_send_interim_ack("P"));
        assert!(!should_send_interim_ack("ping"));
        assert!(!should_send_interim_ack("pagi"));
        assert!(!should_send_interim_ack("selamat malam"));
        assert!(!should_send_interim_ack("makasih ya"));
        assert!(!should_send_interim_ack("oke sip"));
        assert!(!should_send_interim_ack("/model status"));
        assert!(!should_send_interim_ack("Halo aina aku sukma hehe"));
        assert!(!should_send_interim_ack("siap makasih mbak aina, salam kenal ya🤭"));
        assert!(!should_send_interim_ack("Baru bangun kaget, tiba2 ada orang baru"));
    }

    #[test]
    fn test_should_ack_actionable_tasks() {
        assert!(should_send_interim_ack("Coba kamu buatkan semua list chat hari ini"));
        assert!(should_send_interim_ack("Tolong cek log docker container sekarang"));
        assert!(should_send_interim_ack("Bisa buatkan script python untuk backup database?"));
        assert!(should_send_interim_ack("Kenapa server tadi sempat restart?"));
        assert!(should_send_interim_ack("Apa kamu tau chat yang aku reply ini tulisannya apa?"));
    }

    #[test]
    fn test_pick_interim_ack_variations() {
        let ack1 = pick_interim_ack("msg-1", "tolong cek data penjualan");
        assert!(ack1.contains("sebentar") || ack1.contains("cek") || ack1.contains("cari") || ack1.contains("oke"));

        let ack2 = pick_interim_ack("msg-2", "buatkan script python");
        assert!(ack2.contains("sebentar") || ack2.contains("proses") || ack2.contains("siap") || ack2.contains("oke"));
    }
}

