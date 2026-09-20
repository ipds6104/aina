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
        let now_epoch = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        let chat_type_str = match msg.chat_type {
            ChatType::DirectMessage => "direct",
            ChatType::Group => "group",
        };

        let decision = Gatekeeper::evaluate(&msg, &self.bot_jid, &self.bot_name, self.bot_lid.as_deref());

        match decision {
            GatekeeperDecision::Ignore { reason } => {
                info!("Ignoring message {}: {}", msg.id, reason);
                let audit = crate::core::domain::NewWhatsAppActionAudit {
                    message_id: msg.id.clone(),
                    chat_jid: msg.chat_jid.clone(),
                    chat_type: chat_type_str.to_string(),
                    sender_jid: msg.sender.jid.clone(),
                    sender_name: msg.sender.name.clone(),
                    decision: "ignore".to_string(),
                    decision_reason: reason.clone(),
                    conversation_id: None,
                    status: "ignored".to_string(),
                    input_text: msg.text.clone(),
                    has_media: msg.has_media,
                    media_path: msg.media_path.clone(),
                    response_text: None,
                    error_message: None,
                    duration_seconds: Some(0.0),
                    tools_invoked: vec![],
                    created_at_epoch: now_epoch,
                    completed_at_epoch: Some(now_epoch),
                };
                let _ = self.session_store.record_action_audit(&audit).await;
                Ok(())
            }
            GatekeeperDecision::RecordOnly { reason } => {
                info!(
                    "Recording ambient group message from {} in {}: {}",
                    msg.sender.jid, msg.chat_jid, reason
                );
                let record_text = if msg.has_media {
                    if let Some(ref path) = msg.media_path {
                        format!("{} [Media: {}]", msg.text, path)
                    } else {
                        format!("{} [Media]", msg.text)
                    }
                } else {
                    msg.text.clone()
                };
                self.session_store
                    .record_message(&msg.chat_jid, &msg.sender.jid, &record_text, false)
                    .await?;

                let audit = crate::core::domain::NewWhatsAppActionAudit {
                    message_id: msg.id.clone(),
                    chat_jid: msg.chat_jid.clone(),
                    chat_type: chat_type_str.to_string(),
                    sender_jid: msg.sender.jid.clone(),
                    sender_name: msg.sender.name.clone(),
                    decision: "record_only".to_string(),
                    decision_reason: reason.clone(),
                    conversation_id: None,
                    status: "recorded".to_string(),
                    input_text: record_text,
                    has_media: msg.has_media,
                    media_path: msg.media_path.clone(),
                    response_text: None,
                    error_message: None,
                    duration_seconds: Some(0.0),
                    tools_invoked: vec![],
                    created_at_epoch: now_epoch,
                    completed_at_epoch: Some(now_epoch),
                };
                let _ = self.session_store.record_action_audit(&audit).await;
                Ok(())
            }
            GatekeeperDecision::Respond { reason } => {
                info!(
                    "Responding to message from {} in {}: {}",
                    msg.sender.jid, msg.chat_jid, reason
                );

                // 1. Record incoming message
                let record_text = if msg.has_media {
                    if let Some(ref path) = msg.media_path {
                        format!("{} [Media: {}]", msg.text, path)
                    } else {
                        format!("{} [Media]", msg.text)
                    }
                } else {
                    msg.text.clone()
                };
                self.session_store
                    .record_message(&msg.chat_jid, &msg.sender.jid, &record_text, false)
                    .await?;

                let audit = crate::core::domain::NewWhatsAppActionAudit {
                    message_id: msg.id.clone(),
                    chat_jid: msg.chat_jid.clone(),
                    chat_type: chat_type_str.to_string(),
                    sender_jid: msg.sender.jid.clone(),
                    sender_name: msg.sender.name.clone(),
                    decision: "respond".to_string(),
                    decision_reason: reason.clone(),
                    conversation_id: None,
                    status: "in_progress".to_string(),
                    input_text: record_text,
                    has_media: msg.has_media,
                    media_path: msg.media_path.clone(),
                    response_text: None,
                    error_message: None,
                    duration_seconds: None,
                    tools_invoked: vec![],
                    created_at_epoch: now_epoch,
                    completed_at_epoch: None,
                };
                let audit_id = self.session_store.record_action_audit(&audit).await.ok();
                let start_instant = std::time::Instant::now();

                // Reject heavy audio and video messages immediately without LLM invocation
                if msg.text.starts_with("[Pesan Audio/Voice Note diabaikan")
                    || msg.text.starts_with("[Pesan Video diabaikan")
                {
                    let reply = "Maaf yaa, untuk saat ini Aina belum dapat memproses pesan audio/voice note atau video karena ukurannya yang berat. Silakan kirimkan dalam bentuk teks, dokumen, gambar, atau kartu kontak yaa! Terima kasih.".to_string();
                    if let Some(aid) = audit_id {
                        let dur = start_instant.elapsed().as_secs_f64();
                        let _ = self
                            .session_store
                            .update_action_audit_result(aid, None, Some(&reply), None, "success", Some(dur), &[])
                            .await;
                    }
                    self.session_store.record_message(&msg.chat_jid, &self.bot_jid, &reply, true).await?;
                    self.whatsapp.send_text_with_session(&msg.chat_jid, &reply, Some(&msg.id), msg.session_role).await?;
                    return Ok(());
                }

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
                    if let Some(aid) = audit_id {
                        let dur = start_instant.elapsed().as_secs_f64();
                        let _ = self.session_store.update_action_audit_result(aid, None, Some(&reply), None, "success", Some(dur), &[]).await;
                    }
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
                        if let Some(aid) = audit_id {
                            let dur = start_instant.elapsed().as_secs_f64();
                            let _ = self.session_store.update_action_audit_result(aid, None, Some(&reply), None, "success", Some(dur), &[]).await;
                        }
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
                                if let Some(aid) = audit_id {
                                    let dur = start_instant.elapsed().as_secs_f64();
                                    let _ = self.session_store.update_action_audit_result(aid, None, Some(&reply), None, "success", Some(dur), &[]).await;
                                }
                                self.session_store.record_message(&msg.chat_jid, &self.bot_jid, &reply, true).await?;
                                self.whatsapp.send_text_with_session(&msg.chat_jid, &reply, Some(&msg.id), msg.session_role).await?;
                                return Ok(());
                            }
                            Err(e) => {
                                let reply = format!(
                                    "⚠️ *Gagal Mengganti Model*\n\n{}\n\nContoh: `/model gemini-3.8-flash-medium`",
                                    e
                                );
                                if let Some(aid) = audit_id {
                                    let dur = start_instant.elapsed().as_secs_f64();
                                    let _ = self.session_store.update_action_audit_result(aid, None, Some(&reply), Some(&e.to_string()), "failed", Some(dur), &[]).await;
                                }
                                self.session_store.record_message(&msg.chat_jid, &self.bot_jid, &reply, true).await?;
                                self.whatsapp.send_text_with_session(&msg.chat_jid, &reply, Some(&msg.id), msg.session_role).await?;
                                return Ok(());
                            }
                        }
                    }
                }

                // Check for built-in quick command: /token or /auth or /account (Multi-Account Management)
                if trimmed_text.starts_with("/token") || trimmed_text.starts_with("/auth") || trimmed_text.starts_with("/account") {
                    let parts: Vec<&str> = trimmed_text.split_whitespace().collect();
                    let is_status = parts.len() == 1 || (parts.len() >= 2 && (parts[1] == "status" || parts[1] == "list" || parts[1] == "pool"));

                    if is_status {
                        let pool_status = self.agent_engine.get_account_pool_status().await;
                        let mut status_lines = Vec::new();
                        for acc in &pool_status {
                            let state_str = if acc.is_cooldown {
                                format!("⏳ Cooldown (sisa {}s)", acc.cooldown_remaining_secs)
                            } else {
                                "🟢 Aktif & Siap".to_string()
                            };
                            status_lines.push(format!("• *{}*: {}", acc.label, state_str));
                        }
                        let list_str = if status_lines.is_empty() {
                            "• _Belum ada akun di pool (menggunakan token file default)_".to_string()
                        } else {
                            status_lines.join("\n")
                        };

                        let reply = format!(
                            "👥 *Status Pool Akun Antigravity Aina*\n\nTotal Akun Terdaftar: *{}*\n\n{}\n\n💡 *Cara Menambah Akun Cadangan:*\nKirimkan token di DM ini dengan format:\n`/token <oauth_json>`\nAtau buka Dashboard Setup di Web.",
                            pool_status.len(),
                            list_str
                        );

                        if let Some(aid) = audit_id {
                            let dur = start_instant.elapsed().as_secs_f64();
                            let _ = self.session_store.update_action_audit_result(aid, None, Some(&reply), None, "success", Some(dur), &[]).await;
                        }
                        self.session_store.record_message(&msg.chat_jid, &self.bot_jid, &reply, true).await?;
                        self.whatsapp.send_text_with_session(&msg.chat_jid, &reply, Some(&msg.id), msg.session_role).await?;
                        return Ok(());
                    }

                    // For adding / saving token, enforce security: MUST be in Direct Message (DM)
                    if msg.chat_type != ChatType::DirectMessage {
                        let reply = "⚠️ *Demi Keamanan:* Perintah pendaftaran token akun OAuth HANYA boleh dikirim melalui Pesan Pribadi (DM) ke Aina, dilarang di dalam grup kerja.".to_string();
                        self.whatsapp.send_text_with_session(&msg.chat_jid, &reply, Some(&msg.id), msg.session_role).await?;
                        return Ok(());
                    }

                    // Extract token JSON from the rest of the string
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
                        let reply = "ℹ️ *Petunjuk Penggunaan Token:*\nUntuk menambahkan akun baru, ketik:\n`/token <oauth_json>`\n\nContoh:\n`/token {\"token\":\"...\"}`".to_string();
                        self.whatsapp.send_text_with_session(&msg.chat_jid, &reply, Some(&msg.id), msg.session_role).await?;
                        return Ok(());
                    }

                    match self.agent_engine.save_auth_token(token_str).await {
                        Ok(_) => {
                            let pool = self.agent_engine.get_account_pool_status().await;
                            let reply = format!(
                                "✅ *Akun Antigravity Berhasil Ditambahkan!*\n\nAkun baru telah diverifikasi dan langsung aktif di dalam pool.\n• Total Akun di Pool: *{}*\n• Strategi Rotasi: *Round-Robin (Bergantian)*\n\nAina sekarang siap melanjutkan tugas tanpa gangguan kuota!",
                                pool.len()
                            );
                            if let Some(aid) = audit_id {
                                let dur = start_instant.elapsed().as_secs_f64();
                                let _ = self.session_store.update_action_audit_result(aid, None, Some(&reply), None, "success", Some(dur), &[]).await;
                            }
                            self.session_store.record_message(&msg.chat_jid, &self.bot_jid, &reply, true).await?;
                            self.whatsapp.send_text_with_session(&msg.chat_jid, &reply, Some(&msg.id), msg.session_role).await?;
                            return Ok(());
                        }
                        Err(e) => {
                            let reply = format!(
                                "❌ *Gagal Menyimpan Token:*\n{}\n\nPastikan format token berupa JSON yang valid dari file `antigravity-oauth-token`.",
                                e
                            );
                            if let Some(aid) = audit_id {
                                let dur = start_instant.elapsed().as_secs_f64();
                                let _ = self.session_store.update_action_audit_result(aid, None, Some(&reply), Some(&e.to_string()), "failed", Some(dur), &[]).await;
                            }
                            self.session_store.record_message(&msg.chat_jid, &self.bot_jid, &reply, true).await?;
                            self.whatsapp.send_text_with_session(&msg.chat_jid, &reply, Some(&msg.id), msg.session_role).await?;
                            return Ok(());
                        }
                    }
                }

                // Check for polite conversational closing / acknowledgment / gratitude:
                // Rather than intimidating users with walls of text for short closing messages like "Sama-sama kak",
                // react politely with an appropriate emoji (e.g. 🙏 or 👍) and close the interaction smoothly.
                if let Some(reaction_emoji) = detect_conversational_closing(&msg.text) {
                    info!(
                        "Detected conversational closing in message '{}' from {}, responding with reaction {}",
                        msg.text, msg.sender.jid, reaction_emoji
                    );
                    if let Err(e) = self
                        .whatsapp
                        .send_reaction_with_session(&msg.chat_jid, &msg.id, reaction_emoji, msg.session_role)
                        .await
                    {
                        warn!("Failed to send reaction {} to {}: {}", reaction_emoji, msg.chat_jid, e);
                    }
                    let reaction_note = format!("[Reaksi WhatsApp: {}]", reaction_emoji);
                    if let Some(aid) = audit_id {
                        let dur = start_instant.elapsed().as_secs_f64();
                        let _ = self.session_store.update_action_audit_result(aid, None, Some(&reaction_note), None, "success", Some(dur), &[]).await;
                    }
                    let _ = self
                        .session_store
                        .record_message(&msg.chat_jid, &self.bot_jid, &reaction_note, true)
                        .await;
                    return Ok(());
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
                let mut prompt = self.persona_engine.build_prompt(&msg, profile.as_ref());

                // Epistemic Vigilance & Anti-Confabulation Gate:
                let resolution = crate::core::domain::metacognition::AntiConfabGate::evaluate_discrepancy(
                    "WhatsApp Story media upload with caption field",
                    &msg.text,
                );
                if let Some(guardrail) = crate::core::domain::metacognition::AntiConfabGate::format_epistemic_guardrail(&resolution) {
                    prompt.push_str("\n\n---\n");
                    prompt.push_str(&guardrail);
                }

                // Task Triage for novel/held-out tasks:
                let manifest = crate::core::domain::metacognition::AgentCapabilityManifest::default_manifest();
                let triage = crate::core::domain::metacognition::TaskTriageEngine::triage_task(&msg.text, &manifest);
                match triage {
                    crate::core::domain::metacognition::TriageDecision::ElegantRejection { reason, missing_capabilities } => {
                        info!("Task triage rejected novel impossible task for chat {}: {}", msg.chat_jid, reason);
                        let reject_msg = format!("Aina belum bisa menjalankan permintaan ini yaa 🙏\n\n*Alasan:* {}\n*Batasan Teknis:* Kapabilitas {} belum tersedia di lingkungan saat ini.", reason, missing_capabilities.join(", "));
                        let _ = self.session_store.record_message(&msg.chat_jid, &self.bot_jid, &reject_msg, true).await;
                        let quote_id = match msg.chat_type {
                            ChatType::Group => Some(msg.id.as_str()),
                            ChatType::DirectMessage => None,
                        };
                        let _ = self.whatsapp.send_text_with_session(&msg.chat_jid, &reject_msg, quote_id, msg.session_role).await;
                        return Ok(());
                    }
                    crate::core::domain::metacognition::TriageDecision::GracefulDegradation { suggested_alternative, reason, .. } => {
                        prompt.push_str(&format!(
                            "\n\n---\n[CATATAN TRIAGE KAPABILITAS]: Tugas ini melampaui kemampuan native ({}). Alihkan atau tawarkan alternatif elegan: {}.",
                            reason, suggested_alternative
                        ));
                    }
                    _ => {}
                }

                // Start async presence heartbeat + 3-minute progress check
                let whatsapp = Arc::clone(&self.whatsapp);
                let chat_jid = msg.chat_jid.clone();
                let session_role = msg.session_role;

                let quote_id = match msg.chat_type {
                    ChatType::Group => Some(msg.id.as_str()),
                    ChatType::DirectMessage => None,
                };

                // Background typing heartbeat: keep "sedang mengetik..." active and notify at 3 minutes if still running
                let heartbeat_chat_jid = chat_jid.clone();
                let heartbeat_quote_id = quote_id.map(|s| s.to_string());
                let heartbeat_handle = tokio::spawn(async move {
                    let mut elapsed_secs: u64 = 0;

                    loop {
                        tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;
                        elapsed_secs += 4;

                        // Keep typing presence alive on WhatsApp
                        let _ = whatsapp
                            .send_presence_with_session(&heartbeat_chat_jid, PresenceState::Composing, session_role)
                            .await;

                        // Send in-flight progressive reassurance every 3 minutes (180s, 360s, 540s...)
                        if elapsed_secs > 0 && elapsed_secs % 180 == 0 {
                            let minutes = elapsed_secs / 60;
                            let progress_note = match minutes {
                                3 => "Masih proses Aina kerjakan yaa, ditunggu sebentar...".to_string(),
                                6 => "Masih terus Aina proses yaa, tugas ini cukup panjang tapi tetap berjalan lancar...".to_string(),
                                9 => "Masih intensif Aina proses yaa, sedang menuju tahap akhir...".to_string(),
                                _ => format!("Masih terus Aina proses yaa (berjalan {} menit), mohon ditunggu sebentar lagi...", minutes),
                            };
                            let _ = whatsapp
                                .send_text_with_session(
                                    &heartbeat_chat_jid,
                                    &progress_note,
                                    heartbeat_quote_id.as_deref(),
                                    session_role,
                                )
                                .await;
                        }
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
                        error!("Agent engine failed to execute for chat {}: {}", msg.chat_jid, e);
                        if let Some(aid) = audit_id {
                            let dur = start_instant.elapsed().as_secs_f64();
                            let _ = self.session_store.update_action_audit_result(
                                aid,
                                None,
                                None,
                                Some(&e.to_string()),
                                "failed",
                                Some(dur),
                                &[],
                            ).await;
                        }

                        let err_str = e.to_string().to_lowercase();
                        let is_quota = err_str.contains("503")
                            || err_str.contains("429")
                            || err_str.contains("quota")
                            || err_str.contains("resource has been exhausted")
                            || err_str.contains("rate limit");

                        if is_quota {
                            let pool_status = self.agent_engine.get_account_pool_status().await;
                            let total_accs = pool_status.len();
                            let friendly_quota = if msg.chat_type == ChatType::DirectMessage {
                                if total_accs <= 1 {
                                    "Aduh, kuota akses AI untuk akun saat ini lagi penuh/cooling down nih dari Google (429 Rate Limit).\n\n💡 *Solusi Cepat & Seamless:*\nAnda bisa menambahkan akun Pro cadangan agar Aina otomatis bergantian tanpa putus:\n1. Buka dashboard `/setup` di browser untuk paste token akun cadangan, ATAU\n2. Kirim token langsung di chat DM ini dengan perintah:\n   `/token {\"token\":\"...\"}`\n3. Atau pasang `AINA_OAUTH_TOKEN_2=...` di environment.\n\n_Konteks obrolan ini tersimpan aman dan tidak akan hilang!_".to_string()
                                } else {
                                    "Aduh, seluruh akun AI di pool sedang cooling down dari Google. Tunggu sekitar 2-3 menit yaa, nanti salah satu akun akan otomatis aktif kembali!".to_string()
                                }
                            } else {
                                "Aduh, kuota akses AI untuk sementara lagi penuh/cooling down nih dari Google. Tunggu sekitar 2-3 menit yaa, atau admin bisa menambahkan akun cadangan di dashboard setup!".to_string()
                            };
                            let _ = self
                                .whatsapp
                                .send_text_with_session(&msg.chat_jid, &friendly_quota, quote_id, msg.session_role)
                                .await;
                        }

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

                    // 8. Send reply back to WhatsApp (supports multi-bubble splitting)
                    let bubbles = split_response_into_bubbles(&agent_res.response_text);
                    for (i, bubble) in bubbles.iter().enumerate() {
                        let quote = if i == 0 { quote_id } else { None };
                        if i > 0 {
                            let _ = self
                                .whatsapp
                                .send_presence_with_session(&msg.chat_jid, PresenceState::Composing, msg.session_role)
                                .await;
                            tokio::time::sleep(tokio::time::Duration::from_millis(600)).await;
                        }

                        self.whatsapp
                            .send_text_with_session(&msg.chat_jid, bubble, quote, msg.session_role)
                            .await?;
                    }
                }

                // 8b. If message had media, append AI response analysis to the companion .txt transcript sidecar
                if let Some(ref media_path) = msg.media_path {
                    let sidecar_txt_path = std::path::Path::new(media_path).with_extension("txt");
                    if sidecar_txt_path.exists() {
                        use tokio::io::AsyncWriteExt;
                        let append_content = format!("\n--- Analisis & Respon Aina ---\n{}\n", agent_res.response_text);
                        if let Ok(mut file) = tokio::fs::OpenOptions::new().append(true).open(&sidecar_txt_path).await {
                            let _ = file.write_all(append_content.as_bytes()).await;
                            info!("Appended AI response transcript to sidecar {:?}", sidecar_txt_path);
                        }
                    }
                }

                // 9. Reset presence
                let _ = self
                    .whatsapp
                    .send_presence_with_session(&msg.chat_jid, PresenceState::Paused, msg.session_role)
                    .await;

                let actual_duration = start_instant.elapsed().as_secs_f64();
                info!(
                    "Successfully replied to {} in {:.2}s",
                    msg.chat_jid, actual_duration
                );

                let tools_invoked = crate::core::domain::AuditEngine::extract_tools_for_conversation(
                    &crate::core::domain::AuditEngine::default_brain_path(),
                    &agent_res.conversation_id,
                );
                if let Some(aid) = audit_id {
                    let _ = self.session_store.update_action_audit_result(
                        aid,
                        Some(&agent_res.conversation_id),
                        Some(&agent_res.response_text),
                        None,
                        "success",
                        Some(actual_duration),
                        &tools_invoked,
                    ).await;
                }

                Ok(())
            }
        }
    }
}

/// Splits a combined AI response string into multiple WhatsApp message bubbles
/// if explicit delimiter tokens are present.
pub fn split_response_into_bubbles(text: &str) -> Vec<String> {
    let delimiters = ["<<<SPLIT_CHAT>>>", "<<<NEXT_CHAT>>>", "<<<SPLIT>>>", "[SPLIT_CHAT]"];
    for delim in &delimiters {
        if text.contains(delim) {
            let parts: Vec<String> = text
                .split(delim)
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            if !parts.is_empty() {
                return parts;
            }
        }
    }
    let trimmed = text.trim();
    if trimmed.is_empty() {
        vec![]
    } else {
        vec![trimmed.to_string()]
    }
}

/// Detects if a message is a pure conversational closing/acknowledgment/gratitude
/// that should receive a polite emoji reaction instead of an intimidating text reply.
pub fn detect_conversational_closing(text: &str) -> Option<&'static str> {
    let clean = text.trim();
    if clean.is_empty() || clean.len() > 60 {
        return None;
    }

    // Never auto-react if text has question mark
    if clean.contains('?') {
        return None;
    }

    let lower = clean.to_lowercase();

    // Check for negative or request keywords that indicate a follow-up inquiry
    let question_keywords = [
        "kenapa", "mengapa", "bagaimana", "gimana", "kapan", "siapa", "dimana", "mana",
        "tolong", "bisa tolong", "mohon bantuan", "jadwalkan", "kirimkan", "carikan",
        "tapi", "namun", "tetapi", "masih ada", "belum",
    ];
    for kw in &question_keywords {
        if lower.contains(kw) {
            return None;
        }
    }

    // Strip punctuation to normalize
    let normalized: String = lower
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect();
    let words: Vec<&str> = normalized.split_whitespace().collect();

    if words.is_empty() || words.len() > 6 {
        return None;
    }

    let joined = words.join(" ");

    // 1. Gratitude & polite warmth -> "🙏"
    let gratitude_phrases = [
        "sama sama", "samasama", "sama2", "samik samik", "sam2",
        "terima kasih", "terimakasih", "makasih", "makasi", "makasihh", "tengkyu",
        "thank you", "thanks", "thx", "tks", "matur nuwun", "nuhun",
        "sukses selalu", "sehat selalu", "aamiin", "amin ya rabbal alamin",
        "semoga lancar", "semangat", "semangat kak",
    ];

    for pat in &gratitude_phrases {
        if joined == *pat
            || joined.starts_with(&format!("{} ", pat))
            || joined.ends_with(&format!(" {}", pat))
            || joined == format!("{} kak", pat)
            || joined == format!("{} mas", pat)
            || joined == format!("{} mba", pat)
            || joined == format!("{} pak", pat)
            || joined == format!("{} bu", pat)
            || joined == format!("{} aina", pat)
            || joined == format!("{} ya", pat)
            || joined == format!("{} yaa", pat)
            || joined == format!("{} banyak", pat)
            || joined == format!("{} infonya", pat)
        {
            return Some("🙏");
        }
    }

    // 2. Acknowledgment & confirmation -> "👍"
    let ack_phrases = [
        "siap", "siapp", "siappp", "siap kak", "siap pak", "siap bu", "siap mba", "siap mas",
        "oke siap", "ok siap", "oke siap kak", "ok siap kak",
        "noted", "noted kak", "noted pak", "noted bu",
        "oke", "ok", "okee", "okey", "sip", "sipp", "sippp", "oke sip", "ok sip", "mantap",
        "baik", "baik kak", "baik pak", "baik bu", "baik siap", "siap laksanakan",
        "paham", "paham kak", "mengerti", "mengerti kak", "sudah kak", "siap terima kasih",
    ];

    for pat in &ack_phrases {
        if joined == *pat
            || joined == format!("{} kak", pat)
            || joined == format!("{} pak", pat)
            || joined == format!("{} bu", pat)
            || joined == format!("{} mba", pat)
            || joined == format!("{} mas", pat)
            || joined == format!("{} aina", pat)
            || joined == format!("{} ya", pat)
        {
            return Some("👍");
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_response_single_bubble() {
        let text = "Halo Bang, ini satu pesan utuh.";
        let bubbles = split_response_into_bubbles(text);
        assert_eq!(bubbles, vec!["Halo Bang, ini satu pesan utuh."]);
    }

    #[test]
    fn test_split_response_multiple_bubbles() {
        let text = "Ini pesan 1 untuk Abang\n<<<SPLIT_CHAT>>>\nIni pesan 2 draf siap forward";
        let bubbles = split_response_into_bubbles(text);
        assert_eq!(bubbles, vec![
            "Ini pesan 1 untuk Abang",
            "Ini pesan 2 draf siap forward"
        ]);
    }

    #[test]
    fn test_split_response_multiple_aliases() {
        let text = "Bagian A<<<NEXT_CHAT>>>Bagian B<<<NEXT_CHAT>>>Bagian C";
        let bubbles = split_response_into_bubbles(text);
        assert_eq!(bubbles, vec!["Bagian A", "Bagian B", "Bagian C"]);
    }

    #[test]
    fn test_split_response_empty_chunks_filtered() {
        let text = "<<<SPLIT_CHAT>>>Pesan Tunggal<<<SPLIT_CHAT>>>   ";
        let bubbles = split_response_into_bubbles(text);
        assert_eq!(bubbles, vec!["Pesan Tunggal"]);
    }

    #[test]
    fn test_detect_conversational_closing_gratitude() {
        assert_eq!(detect_conversational_closing("Sama-sama kak"), Some("🙏"));
        assert_eq!(detect_conversational_closing("sama2 yaa"), Some("🙏"));
        assert_eq!(detect_conversational_closing("Terima kasih banyak!"), Some("🙏"));
        assert_eq!(detect_conversational_closing("Makasih infonya"), Some("🙏"));
        assert_eq!(detect_conversational_closing("tks"), Some("🙏"));
        assert_eq!(detect_conversational_closing("Aamiin"), Some("🙏"));
    }

    #[test]
    fn test_detect_conversational_closing_acknowledgment() {
        assert_eq!(detect_conversational_closing("Siap kak"), Some("👍"));
        assert_eq!(detect_conversational_closing("Oke siap!"), Some("👍"));
        assert_eq!(detect_conversational_closing("Noted"), Some("👍"));
        assert_eq!(detect_conversational_closing("Siap laksanakan"), Some("👍"));
        assert_eq!(detect_conversational_closing("Mantap"), Some("👍"));
        assert_eq!(detect_conversational_closing("Sipp"), Some("👍"));
    }

    #[test]
    fn test_detect_conversational_closing_ignores_inquiries_and_questions() {
        // Question mark present
        assert_eq!(detect_conversational_closing("Kenapa SLS belum selesai?"), None);
        assert_eq!(detect_conversational_closing("Makasih kak, tapi ada kendala?"), None);
        // Conjunctions indicating follow-up inquiry
        assert_eq!(detect_conversational_closing("Siap kak tapi masih ada selisih"), None);
        assert_eq!(detect_conversational_closing("Terima kasih tolong cek kembali"), None);
        // Long messages
        assert_eq!(
            detect_conversational_closing("Terima kasih banyak atas infonya, nanti saya koordinasikan lagi dengan PPL desa sebelah agar cepat tuntas"),
            None
        );
    }
}
