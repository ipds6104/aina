//! Autonomous Background Task Watcher & Self-Trigger pipeline.

use crate::core::domain::{ActionAuditFilter, AuditEngine, NewWhatsAppActionAudit, PresenceState, SessionRole};
use crate::core::ports::{AgentEnginePort, SessionStorePort, WhatsAppPort};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

pub struct BackgroundWatcher;

impl BackgroundWatcher {
    /// Periodically inspects active conversations with running background tasks.
    /// If a background task has finished but Aina previously exited early (turn-ending),
    /// this autonomously wakes up Aina to deliver the final results to WhatsApp.
    pub async fn check_and_trigger(
        agent: &dyn AgentEnginePort,
        session_store: &Arc<dyn SessionStorePort>,
        whatsapp: &Arc<dyn WhatsAppPort>,
        last_self_triggered: &Arc<RwLock<HashMap<String, (usize, i64)>>>,
        now_epoch: i64,
    ) -> anyhow::Result<()> {
        let brain_path = AuditEngine::default_brain_path();
        if !brain_path.is_dir() {
            return Ok(());
        }

        // Query recent action audits (within last 3 hours) that responded and have a conversation
        let filter = ActionAuditFilter {
            since_epoch: Some(now_epoch.saturating_sub(10800)),
            decision: Some("respond".to_string()),
            status: Some("success".to_string()),
            limit: Some(10),
            ..Default::default()
        };

        let recent_audits = session_store.query_action_audits(&filter).await.unwrap_or_default();
        if recent_audits.is_empty() {
            return Ok(());
        }

        for audit in recent_audits {
            let conv_id = match audit.conversation_id.as_deref() {
                Some(id) if !id.trim().is_empty() => id,
                _ => continue,
            };

            // Must have passed at least 60 seconds since the audit was created
            if now_epoch - audit.created_at_epoch < 60 {
                continue;
            }

            // Check recent actions for this chat to prevent waking up on superseded or cancelled tasks
            let chat_filter = ActionAuditFilter {
                chat_jid: Some(audit.chat_jid.clone()),
                since_epoch: Some(now_epoch.saturating_sub(7200)), // check last 2 hours
                limit: Some(10),
                ..Default::default()
            };
            let chat_audits = session_store.query_action_audits(&chat_filter).await.unwrap_or_default();

            // 1. If there is any newer audit for this chat (id > audit.id or created_at > audit.created_at_epoch),
            // this audit is superseded and must not be used to resume background tasks.
            let has_newer = chat_audits.iter().any(|a| a.id > audit.id || a.created_at_epoch > audit.created_at_epoch);
            if has_newer {
                debug!(
                    "Autonomous Watcher: Skipping audit #{} for chat {} because newer interactions exist",
                    audit.id, audit.chat_jid
                );
                continue;
            }

            // 2. If any recent audit was cancelled by the user, do not auto-wakeup
            let was_cancelled = chat_audits.iter().any(|a| {
                a.status == "cancelled"
                    || a.error_message
                        .as_deref()
                        .map(|e| e.contains("Dibatalkan") || e.contains("killed"))
                        .unwrap_or(false)
            });
            if was_cancelled {
                info!(
                    "Autonomous Watcher: Skipping audit #{} for chat {} because a task was recently cancelled by user",
                    audit.id, audit.chat_jid
                );
                continue;
            }

            // 3. If the chat currently has an in_progress task running, do not interfere
            if let Some(latest) = chat_audits.first() {
                if latest.status == "in_progress" {
                    debug!(
                        "Autonomous Watcher: Skipping audit #{} for chat {} because a task is currently in_progress",
                        audit.id, audit.chat_jid
                    );
                    continue;
                }
            }

            // Load transcript for this conversation
            let (steps, _) = AuditEngine::load_transcript_for_conversation(&brain_path, conv_id);
            if steps.is_empty() {
                continue;
            }

            // Find if there is any running background task step (status == "RUNNING")
            let running_step_idx = steps.iter().rposition(|s| s.status.as_deref() == Some("RUNNING"));
            let running_idx = match running_step_idx {
                Some(idx) => idx,
                None => continue,
            };

            // Dynamic Pipeline Cooldown:
            // If it's a NEW background task in a multi-stage pipeline (running_idx > last_idx),
            // allow wake-up after only 45s. If it's the SAME task, enforce 180s cooldown.
            {
                let triggered = last_self_triggered.read().await;
                if let Some(&(last_idx, last_time)) = triggered.get(conv_id) {
                    let required_cooldown = if running_idx > last_idx { 45 } else { 180 };
                    if now_epoch - last_time < required_cooldown {
                        continue;
                    }
                }
            }

            // Check if there has been any user message OR subsequent resolution after the running step
            let steps_after = &steps[running_idx + 1..];

            // If the user already messaged after this step, don't interfere
            let has_user_input_after = steps_after.iter().any(|s| {
                s.step_type.as_deref() == Some("USER_INPUT")
                    || s.source.as_deref() == Some("USER_EXPLICIT")
            });
            if has_user_input_after {
                continue;
            }

            // If there are already 2 or more PLANNER_RESPONSE after the running step,
            // Aina already replied with the resolution
            let planner_responses_after = steps_after
                .iter()
                .filter(|s| s.step_type.as_deref() == Some("PLANNER_RESPONSE"))
                .count();
            if planner_responses_after > 1 {
                continue;
            }

            info!(
                "Autonomous Watcher: Found unhandled background task in conversation {} (Action #{} for {}). Triggering auto-wakeup...",
                conv_id, audit.id, audit.chat_jid
            );

            // Record trigger timestamp and task index to support multi-stage pipelines
            {
                let mut triggered = last_self_triggered.write().await;
                triggered.insert(conv_id.to_string(), (running_idx, now_epoch));
            }

            let prompt = format!(
                "🔔 [SISTEM AINA - AUTO WAKE UP / PIPELINE RESUMPTION]\n\
                Permintaan Asli Pengguna: \"{}\"\n\n\
                Konteks: Tugas latar belakang sebelumnya telah selesai dieksekusi di server.\n\n\
                Instruksi Evaluasi Pipeline & Tindak Lanjut:\n\
                1. Periksa output dan status tugas yang baru saja selesai.\n\
                2. Evaluasi Rangkaian Permintaan Pengguna: Apakah seluruh permintaan pengguna di atas sudah tuntas 100% (misal: generate data, crosscheck, komparasi, dan upload)?\n\
                3. Jika masih ada tahapan lanjutan yang HARUS dijalankan (misal: perlu upload ke Google Drive atau perlu komparasi data):\n\
                    - Lanjutkan eksekusi tahapan berikutnya sekarang (jalankan perintah/tool yang diperlukan).\n\
                    - Berikan kabar progres singkat ke pengguna jika perintah berikutnya membutuhkan waktu.\n\
                4. Jika seluruh rangkaian pekerjaan sudah SELESAI 100%:\n\
                    - Susun laporan rekapitulasi final yang lengkap, terstruktur, ramah, dan siap dibaca pengguna di WhatsApp (sertakan link Drive/Sheets jika ada).\n\
                    5. Jika tugas ternyata masih berjalan di server, berikan kabar progres singkat.",
                audit.input_text
            );

            let start_inst = std::time::Instant::now();
            match agent.execute(Some(conv_id), &prompt).await {
                Ok(res) => {
                    let dur = start_inst.elapsed().as_secs_f64();
                    let clean = res.response_text.trim();
                    if !clean.is_empty() {
                        info!(
                            "Autonomous Watcher: Self-trigger executed for conv {} in {:.2}s. Delivering to WhatsApp {}.",
                            conv_id, dur, audit.chat_jid
                        );

                        if let Err(e) = whatsapp.send_text_with_session(&audit.chat_jid, clean, None, SessionRole::PrimaryBot).await {
                            error!("Autonomous Watcher: Failed to send self-trigger response to WhatsApp: {}", e);
                        } else {
                            let _ = session_store.record_message(&audit.chat_jid, "bot", clean, true).await;
                        }

                        let _ = whatsapp.send_presence_with_session(&audit.chat_jid, PresenceState::Paused, SessionRole::PrimaryBot).await;

                        let audit_entry = NewWhatsAppActionAudit {
                            message_id: format!("auto-wakeup-{}", now_epoch),
                            chat_jid: audit.chat_jid.clone(),
                            chat_type: audit.chat_type.clone(),
                            sender_jid: audit.sender_jid.clone(),
                            sender_name: audit.sender_name.clone(),
                            decision: "respond".to_string(),
                            decision_reason: "Autonomous background task completion wake-up".to_string(),
                            conversation_id: Some(conv_id.to_string()),
                            status: "success".to_string(),
                            input_text: format!("[Auto Wakeup for: {}]", audit.input_text),
                            has_media: false,
                            media_path: None,
                            response_text: Some(clean.to_string()),
                            error_message: None,
                            duration_seconds: Some(dur),
                            tools_invoked: vec!["autonomous_task_watcher".to_string(), "builtin:scheduler".to_string()],
                            usecase: "task_and_scheduling".to_string(),
                            created_at_epoch: now_epoch,
                            completed_at_epoch: Some(now_epoch + dur as i64),
                        };
                        let _ = session_store.record_action_audit(&audit_entry).await;
                    }
                }
                Err(e) => {
                    warn!("Autonomous Watcher: Self-trigger execution failed for conv {}: {}", conv_id, e);
                }
            }
        }

        Ok(())
    }
}
