use crate::core::domain::{KnowledgeEngine, PersonaEngine, ScheduleParser, ScheduledTaskType, SessionRole};
use crate::core::ports::{AgentEnginePort, KnowledgePort, SessionStorePort, WhatsAppPort};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

pub struct ScheduledTickUseCase {
    session_store: Arc<dyn SessionStorePort>,
    whatsapp: Arc<dyn WhatsAppPort>,
    agent_engine: Option<Arc<dyn AgentEnginePort>>,
    persona_engine: Option<Arc<PersonaEngine>>,
    knowledge_port: Arc<dyn KnowledgePort>,
    workspace_dir: Option<PathBuf>,
    timezone_offset_hours: i32,
    last_self_triggered: Arc<RwLock<HashMap<String, (usize, i64)>>>,
}

impl ScheduledTickUseCase {
    pub fn new(
        session_store: Arc<dyn SessionStorePort>,
        whatsapp: Arc<dyn WhatsAppPort>,
        agent_engine: Option<Arc<dyn AgentEnginePort>>,
        persona_engine: Option<Arc<PersonaEngine>>,
        workspace_dir: Option<PathBuf>,
        timezone_offset_hours: i32,
    ) -> Self {
        Self::with_knowledge(
            session_store,
            whatsapp,
            agent_engine,
            persona_engine,
            Arc::new(KnowledgeEngine::new()),
            workspace_dir,
            timezone_offset_hours,
        )
    }

    pub fn with_knowledge(
        session_store: Arc<dyn SessionStorePort>,
        whatsapp: Arc<dyn WhatsAppPort>,
        agent_engine: Option<Arc<dyn AgentEnginePort>>,
        persona_engine: Option<Arc<PersonaEngine>>,
        knowledge_port: Arc<dyn KnowledgePort>,
        workspace_dir: Option<PathBuf>,
        timezone_offset_hours: i32,
    ) -> Self {
        Self {
            session_store,
            whatsapp,
            agent_engine,
            persona_engine,
            knowledge_port,
            workspace_dir,
            timezone_offset_hours,
            last_self_triggered: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn execute(&self) -> anyhow::Result<()> {
        debug!("Running periodic scheduler tick...");

        // 1. In-process deterministic Knowledge Base neatness check & auto-heal
        if let Some(ref ws) = self.workspace_dir {
            if ws.join("knowledge").is_dir() {
                let report = self.knowledge_port.lint(ws, true);
                if report.auto_healed {
                    info!(
                        "Scheduler auto-healed knowledge base for workspace {:?}",
                        ws
                    );
                } else if !report.is_clean {
                    info!(
                        "Scheduler found {} knowledge base violations in {:?}",
                        report.violations_count, ws
                    );
                }
            }
        }

        // 2. Query and process due scheduled tasks (Alarms, Reminders, and Autonomous Web Research)
        let now_epoch = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        let due_tasks = match self.session_store.get_due_scheduled_tasks(now_epoch).await {
            Ok(tasks) => tasks,
            Err(e) => {
                error!("Scheduler failed to query due tasks: {}", e);
                return Ok(());
            }
        };

        if !due_tasks.is_empty() {
            info!("Scheduler found {} due task(s) to execute at epoch {}", due_tasks.len(), now_epoch);
        }

        for task in due_tasks {
            info!(
                "Executing scheduled task #{}: '{}' (Type: {:?}, Target: {})",
                task.id, task.title, task.task_type, task.target_jid
            );

            // 1. Lock/update the task state immediately to prevent duplicate burst execution
            let (next_run, is_active) = match task.schedule_type.as_str() {
                "once" => (None, false),
                "daily" => {
                    let next = ScheduleParser::compute_next_run(
                        "daily",
                        &task.schedule_expr,
                        self.timezone_offset_hours,
                        now_epoch + 60,
                    )
                    .unwrap_or(now_epoch + 86400);
                    (Some(next), true)
                }
                "interval" => {
                    let next = ScheduleParser::compute_next_run(
                        "interval",
                        &task.schedule_expr,
                        self.timezone_offset_hours,
                        now_epoch,
                    )
                    .unwrap_or(now_epoch + 3600);
                    (Some(next), true)
                }
                _ => (None, false),
            };

            if let Err(e) = self.session_store.update_scheduled_task_run(task.id, now_epoch, next_run, is_active).await {
                error!("Failed to lock/update scheduled task #{} run state: {}", task.id, e);
                continue;
            }

            // 2. Execute task payload with duration & diagnostics recording
            let start_instant = std::time::Instant::now();
            match task.task_type {
                ScheduledTaskType::DirectNotification => {
                    if let Err(e) = self
                        .whatsapp
                        .send_text_with_session(&task.target_jid, &task.payload, None, SessionRole::PrimaryBot)
                        .await
                    {
                        let duration = start_instant.elapsed().as_secs_f64();
                        let err_str = e.to_string();
                        error!("Failed to deliver direct notification for task #{}: {}", task.id, e);
                        let _ = self.session_store.update_scheduled_task_result(task.id, "failed", Some(&err_str), duration).await;
                        let _ = self.session_store.record_scheduled_task_run(task.id, &task.title, &task.target_jid, "failed", duration, Some(&err_str), None).await;
                    } else {
                        let duration = start_instant.elapsed().as_secs_f64();
                        info!("Delivered scheduled notification for task #{} to {}", task.id, task.target_jid);
                        let _ = self.session_store.record_message(&task.target_jid, "bot", &task.payload, true).await;
                        let _ = self.session_store.update_scheduled_task_result(task.id, "success", None, duration).await;
                        let _ = self.session_store.record_scheduled_task_run(task.id, &task.title, &task.target_jid, "success", duration, None, Some(&task.payload)).await;
                    }
                }
                ScheduledTaskType::AgentAction => {
                    if let Some(ref agent) = self.agent_engine {
                        let current_time_str = self
                            .persona_engine
                            .as_ref()
                            .map(|p| p.current_local_time_string())
                            .unwrap_or_else(|| format!("Epoch: {}", now_epoch));

                        let is_story = task.target_jid == "status@broadcast" || task.target_jid == "status";

                        let story_delivery_rule = if is_story {
                            "Target adalah Status/Story WhatsApp (24 jam).\n\
                            4. ATURAN STATUS STORY (ANTI STATUS GANDA): Seluruh publikasi story (gambar berserta caption terpasang) WAJIB dilakukan langsung melalui tool: 'python3 scripts/persona_status.py post' atau 'python3 skills/whatsmeow/scripts/wa_tool.py status-send-media --file <path> --caption <caption>'. Scheduler backend TIDAK AKAN mengirim teks percakapan Anda ke status@broadcast agar TIDAK TERJADI STATUS GANDA (satu gambar + satu teks terpisah)!\n"
                        } else {
                            "Format ramah obrolan chat.\n\
                            4. ATURAN PENGIRIMAN: Untuk pesan teks biasa, DILARANG memanggil 'wa_tool.py send-text' di terminal karena teks respons Anda akan dikirim otomatis oleh scheduler backend! Namun, jika tugas ini secara spesifik meminta pengiriman berkas, dokumen, atau GAMBAR/SCREENSHOT, Anda DIPERBOLEHKAN memanggil 'python3 skills/whatsmeow/scripts/wa_tool.py send-media --to <target> --file <path_file> --caption <keterangan_singkat>'.\n"
                        };

                        let prompt = format!(
                            "🔔 [TUGAS TERJADWAL OTOMATIS - WAKE UP CALL]\n\
                            Judul Tugas: {}\n\
                            Waktu Eksekusi: {}\n\
                            Target Pengiriman: WhatsApp ({})\n\
                            Instruksi Utama:\n{}\n\n\
                            PETUNJUK FORMAT RESPON & EFISIENSI KUOTA UNTUK AINA:\n\
                            1. EFISIENSI KUOTA: Lakukan maksimal 1 hingga 2 kali pencarian web (search_web) yang paling esensial. DILARANG KERAS melakukan pencarian berulang-ulang tanpa henti!\n\
                            2. Susun hasil akhir secara rapi, padat, dan ramah ponsel (format WhatsApp: *tebal*, bullet points •).\n\
                            3. {}\
                            5. DILARANG KERAS menyertakan laporan status teknis internal seperti 'Status: Terkirim', 'Pesan berhasil dikirim', 'Memproses pengunggahan...', dsb.\n\
                            6. Berikan langsung teks hasil riset atau informasi akhir yang siap dibaca oleh penerima.",
                            task.title,
                            current_time_str,
                            task.target_jid,
                            task.payload,
                            story_delivery_rule,
                        );

                        let pred_id = format!("sched_{}_{}", task.id, now_epoch);
                        let pred = crate::core::domain::NewMetacognitivePrediction {
                            prediction_id: pred_id.clone(),
                            action_audit_id: None,
                            task_description: format!("Scheduled Task #{}: {}", task.id, task.title),
                            domain_type: if is_story {
                                crate::core::domain::TaskDomainType::PersonaStatus
                            } else {
                                crate::core::domain::TaskDomainType::ScheduleTask
                            },
                            predicted_probability: if is_story { 0.88 } else { 0.92 },
                            complexity_tier: if is_story {
                                crate::core::domain::ComplexityTier::Medium
                            } else {
                                crate::core::domain::ComplexityTier::Low
                            },
                            identified_risks: if is_story {
                                vec!["Image generation timeout".to_string(), "Whatsmeow gateway reachability".to_string()]
                            } else {
                                vec!["Search web rate limits".to_string()]
                            },
                            fallback_strategy: Some("Report failure to admin".to_string()),
                        };
                        let _ = self.session_store.record_metacognitive_prediction(&pred).await;

                        match agent.execute(None, &prompt).await {
                            Ok(res) => {
                                let duration = start_instant.elapsed().as_secs_f64();
                                let clean_res = res.response_text.trim();
                                if !clean_res.is_empty() {
                                    // If destination is status story and response contains error, do not post publicly
                                    let is_error_output = clean_res.starts_with("⚠️") || clean_res.contains("503") || clean_res.contains("quota");
                                    let actual_target = if is_story && is_error_output {
                                        // Fallback to admin JID if available to avoid embarrassing public error stories
                                        self.persona_engine.as_ref()
                                            .map(|p| p.admin_jid())
                                            .filter(|j| !j.trim().is_empty())
                                            .unwrap_or(&task.target_jid)
                                    } else {
                                        &task.target_jid
                                    };

                                    let (status, err_msg) = if is_error_output {
                                        ("failed", Some(clean_res))
                                    } else {
                                        ("success", None)
                                    };

                                    let preview = if clean_res.len() > 300 { &clean_res[..300] } else { clean_res };
                                    let _ = self.session_store.update_scheduled_task_result(task.id, status, err_msg, duration).await;
                                    let _ = self.session_store.record_scheduled_task_run(task.id, &task.title, actual_target, status, duration, err_msg, Some(preview)).await;

                                    let actual_outcome = if is_error_output { 0.0 } else { 1.0 };
                                    let _ = self.session_store.resolve_metacognitive_prediction(
                                        &pred_id,
                                        actual_outcome,
                                        duration,
                                        if is_error_output { Some("agent_error_or_quota") } else { None },
                                    ).await;

                                    if is_story {
                                        // WhatsApp Story (status@broadcast) is published directly by wa_tool.py status-send-media / persona_status.py.
                                        // We MUST NEVER send the agent's textual response/summary as an additional text story to status@broadcast,
                                        // as that results in double status stories (one media story + one text story).
                                        info!("AgentAction task #{} for status@broadcast completed successfully. Suppressed sending conversational text to status@broadcast. Preview: {}", task.id, preview);
                                        continue;
                                    }

                                    if let Err(e) = self
                                        .whatsapp
                                        .send_text_with_session(actual_target, clean_res, None, SessionRole::PrimaryBot)
                                        .await
                                    {
                                        error!("Failed to send agent task result to WhatsApp: {}", e);
                                    } else {
                                        info!("Successfully executed and delivered AgentAction task #{} to {}", task.id, actual_target);
                                        let _ = self.session_store.record_message(actual_target, "bot", clean_res, true).await;
                                    }
                                } else {
                                    let _ = self.session_store.update_scheduled_task_result(task.id, "success", None, duration).await;
                                    let _ = self.session_store.record_scheduled_task_run(task.id, &task.title, &task.target_jid, "success", duration, None, None).await;
                                    let _ = self.session_store.resolve_metacognitive_prediction(
                                        &pred_id,
                                        1.0,
                                        duration,
                                        None,
                                    ).await;
                                }
                            }
                            Err(e) => {
                                let duration = start_instant.elapsed().as_secs_f64();
                                let err_str = e.to_string();
                                error!("Agent failed to execute scheduled task #{}: {}", task.id, e);
                                let _ = self.session_store.update_scheduled_task_result(task.id, "failed", Some(&err_str), duration).await;
                                let _ = self.session_store.record_scheduled_task_run(task.id, &task.title, &task.target_jid, "failed", duration, Some(&err_str), None).await;

                                let _ = self.session_store.resolve_metacognitive_prediction(
                                    &pred_id,
                                    0.0,
                                    duration,
                                    Some(&err_str),
                                ).await;

                                let err_msg = format!("⚠️ _Gagal menjalankan tugas terjadwal '{}': {}_", task.title, e);
                                let fallback_target = if is_story {
                                    self.persona_engine.as_ref()
                                        .map(|p| p.admin_jid())
                                        .filter(|j| !j.trim().is_empty())
                                        .unwrap_or(&task.target_jid)
                                } else {
                                    &task.target_jid
                                };
                                let _ = self.whatsapp.send_text_with_session(fallback_target, &err_msg, None, SessionRole::PrimaryBot).await;
                            }
                        }
                    } else {
                        warn!("Cannot execute AgentAction task #{}: AgentEngine is not configured", task.id);
                    }
                }
            }
        }

        // 3. Autonomous Background Task Watcher & Self-Trigger
        // Detects if any active conversation has a background task that finished without a follow-up model response
        if let Some(ref agent) = self.agent_engine {
            if let Err(e) = self.check_and_trigger_completed_background_tasks(agent.as_ref(), now_epoch).await {
                error!("Error in autonomous background task watcher: {}", e);
            }
        }

        Ok(())
    }

    /// Periodically inspects active conversations with running background tasks.
    /// If a background task has finished but Aina previously exited early (turn-ending),
    /// this autonomously wakes up Aina to deliver the final results to WhatsApp.
    async fn check_and_trigger_completed_background_tasks(
        &self,
        agent: &dyn AgentEnginePort,
        now_epoch: i64,
    ) -> anyhow::Result<()> {
        let brain_path = crate::core::domain::AuditEngine::default_brain_path();
        if !brain_path.is_dir() {
            return Ok(());
        }

        // Query recent action audits (within last 3 hours) that responded and have a conversation
        let filter = crate::core::domain::ActionAuditFilter {
            since_epoch: Some(now_epoch.saturating_sub(10800)),
            decision: Some("respond".to_string()),
            status: Some("success".to_string()),
            limit: Some(10),
            ..Default::default()
        };

        let recent_audits = self.session_store.query_action_audits(&filter).await.unwrap_or_default();
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
            let chat_filter = crate::core::domain::ActionAuditFilter {
                chat_jid: Some(audit.chat_jid.clone()),
                since_epoch: Some(now_epoch.saturating_sub(7200)), // check last 2 hours
                limit: Some(10),
                ..Default::default()
            };
            let chat_audits = self.session_store.query_action_audits(&chat_filter).await.unwrap_or_default();

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
            let (steps, _) = crate::core::domain::AuditEngine::load_transcript_for_conversation(&brain_path, conv_id);
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
                let triggered = self.last_self_triggered.read().await;
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
                let mut triggered = self.last_self_triggered.write().await;
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

                        if let Err(e) = self.whatsapp.send_text_with_session(&audit.chat_jid, clean, None, SessionRole::PrimaryBot).await {
                            error!("Autonomous Watcher: Failed to send self-trigger response to WhatsApp: {}", e);
                        } else {
                            let _ = self.session_store.record_message(&audit.chat_jid, "bot", clean, true).await;
                        }

                        let _ = self.whatsapp.send_presence_with_session(&audit.chat_jid, crate::core::domain::PresenceState::Paused, SessionRole::PrimaryBot).await;

                        let audit_entry = crate::core::domain::NewWhatsAppActionAudit {
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
                            tools_invoked: vec!["autonomous_task_watcher".to_string()],
                            created_at_epoch: now_epoch,
                            completed_at_epoch: Some(now_epoch + dur as i64),
                        };
                        let _ = self.session_store.record_action_audit(&audit_entry).await;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::driven::SqliteSessionStore;
    use crate::core::domain::{NewScheduledTask, ScheduledTaskType};
    use crate::core::ports::WhatsAppPort;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct TestWhatsApp {
        pub sent_count: AtomicUsize,
    }

    #[async_trait::async_trait]
    impl WhatsAppPort for TestWhatsApp {
        async fn send_text(&self, _to: &str, _text: &str, _qid: Option<&str>) -> anyhow::Result<()> {
            self.sent_count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
        async fn send_presence(&self, _to: &str, _state: crate::core::domain::PresenceState) -> anyhow::Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_scheduled_tick_executes_due_notification() {
        let dir = std::env::temp_dir().join(format!("aina_test_tick_{}", rand::random::<u32>()));
        let db_file = dir.join("tick_test.db");
        let store = Arc::new(SqliteSessionStore::new(&db_file).unwrap());

        let whatsapp = Arc::new(TestWhatsApp {
            sent_count: AtomicUsize::new(0),
        });

        // Insert a task due in the past
        let new_task = NewScheduledTask {
            title: "Pengingat YouTube Test".to_string(),
            task_type: ScheduledTaskType::DirectNotification,
            target_jid: "6289625345646@s.whatsapp.net".to_string(),
            payload: "Buka YouTube ya!".to_string(),
            schedule_type: "once".to_string(),
            schedule_expr: "22:26".to_string(),
            next_run_epoch: 100, // definitely in the past
        };
        let task_id = store.create_scheduled_task(&new_task).await.unwrap();

        let usecase = ScheduledTickUseCase::new(
            Arc::clone(&store) as _,
            Arc::clone(&whatsapp) as _,
            None,
            None,
            None,
            7,
        );

        // Execute tick
        usecase.execute().await.unwrap();

        // WhatsApp should have received 1 message
        assert_eq!(whatsapp.sent_count.load(Ordering::SeqCst), 1);

        // Task should now be inactive
        let task = store.get_scheduled_task(task_id).await.unwrap().unwrap();
        assert!(!task.is_active);
        assert!(task.last_run_epoch.is_some());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
