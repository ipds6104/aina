use crate::core::domain::{KnowledgeEngine, PersonaEngine, ScheduleParser, ScheduledTaskType, SessionRole};
use crate::core::ports::{AgentEnginePort, SessionStorePort, WhatsAppPort};
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

pub struct ScheduledTickUseCase {
    session_store: Arc<dyn SessionStorePort>,
    whatsapp: Arc<dyn WhatsAppPort>,
    agent_engine: Option<Arc<dyn AgentEnginePort>>,
    persona_engine: Option<Arc<PersonaEngine>>,
    workspace_dir: Option<PathBuf>,
    timezone_offset_hours: i32,
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
        Self {
            session_store,
            whatsapp,
            agent_engine,
            persona_engine,
            workspace_dir,
            timezone_offset_hours,
        }
    }

    pub async fn execute(&self) -> anyhow::Result<()> {
        debug!("Running periodic scheduler tick...");

        // 1. In-process deterministic Knowledge Base neatness check & auto-heal
        if let Some(ref ws) = self.workspace_dir {
            if ws.join("knowledge").is_dir() {
                let report = KnowledgeEngine::lint(ws, true);
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

            // 2. Execute task payload
            match task.task_type {
                ScheduledTaskType::DirectNotification => {
                    if let Err(e) = self
                        .whatsapp
                        .send_text_with_session(&task.target_jid, &task.payload, None, SessionRole::PrimaryBot)
                        .await
                    {
                        error!("Failed to deliver direct notification for task #{}: {}", task.id, e);
                    } else {
                        info!("Delivered scheduled notification for task #{} to {}", task.id, task.target_jid);
                        let _ = self.session_store.record_message(&task.target_jid, "bot", &task.payload, true).await;
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

                        let prompt = format!(
                            "🔔 [TUGAS TERJADWAL OTOMATIS - WAKE UP CALL]\n\
                            Judul Tugas: {}\n\
                            Waktu Eksekusi: {}\n\
                            Target Pengiriman: WhatsApp ({})\n\
                            Instruksi Utama:\n{}\n\n\
                            PETUNJUK FORMAT RESPON & EFISIENSI KUOTA UNTUK AINA:\n\
                            1. EFISIENSI KUOTA: Lakukan maksimal 1 hingga 2 kali pencarian web (search_web) yang paling esensial. DILARANG KERAS melakukan pencarian berulang-ulang tanpa henti!\n\
                            2. Susun hasil akhir secara rapi, padat, dan ramah ponsel (format WhatsApp: *tebal*, bullet points •).\n\
                            3. {}
                            4. DILARANG KERAS menyertakan laporan status teknis internal seperti 'Status: Terkirim', 'Pesan berhasil dikirim', dsb.\n\
                            5. Berikan langsung teks hasil riset atau informasi akhir yang siap dibaca oleh penerima.",
                            task.title,
                            current_time_str,
                            task.target_jid,
                            task.payload,
                            if is_story {
                                "Target adalah Status/Story WhatsApp (24 jam). Buat teks ringkas, memikat, dan nyaman dibaca dalam sekali lihat di story (maksimal 3-5 baris padat).\n"
                            } else {
                                "Format ramah obrolan chat.\n"
                            }
                        );

                        match agent.execute(None, &prompt).await {
                            Ok(res) => {
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
                                }
                            }
                            Err(e) => {
                                error!("Agent failed to execute scheduled task #{}: {}", task.id, e);
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
