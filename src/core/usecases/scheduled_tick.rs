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

                        let prompt = format!(
                            "🔔 [TUGAS TERJADWAL OTOMATIS - WAKE UP CALL]\n\
                            Judul Tugas: {}\n\
                            Waktu Eksekusi: {}\n\
                            Target Pengiriman: WhatsApp ({})\n\
                            Instruksi Utama:\n{}\n\n\
                            PETUNJUK FORMAT RESPON UNTUK AINA:\n\
                            1. Langsung jalankan instruksi di atas sekarang juga (gunakan search_web, read_url_content, atau alat bantu lain bila diperlukan riset internet).\n\
                            2. Susun hasil akhir secara rapi, padat, dan ramah ponsel (format WhatsApp: *tebal*, bullet points •, sertakan link sumber asli bila riset berita).\n\
                            3. DILARANG KERAS menyertakan laporan status teknis internal seperti 'Status: Terkirim', 'Pesan berhasil dikirim', dsb.\n\
                            4. Berikan langsung teks hasil riset atau informasi akhir yang siap dibaca oleh penerima.",
                            task.title,
                            current_time_str,
                            task.target_jid,
                            task.payload
                        );

                        match agent.execute(None, &prompt).await {
                            Ok(res) => {
                                let clean_res = res.response_text.trim();
                                if !clean_res.is_empty() {
                                    if let Err(e) = self
                                        .whatsapp
                                        .send_text_with_session(&task.target_jid, clean_res, None, SessionRole::PrimaryBot)
                                        .await
                                    {
                                        error!("Failed to send agent task result to WhatsApp: {}", e);
                                    } else {
                                        info!("Successfully executed and delivered AgentAction task #{} to {}", task.id, task.target_jid);
                                        let _ = self.session_store.record_message(&task.target_jid, "bot", clean_res, true).await;
                                    }
                                }
                            }
                            Err(e) => {
                                error!("Agent failed to execute scheduled task #{}: {}", task.id, e);
                                let err_msg = format!("⚠️ _Gagal menjalankan tugas terjadwal '{}': {}_", task.title, e);
                                let _ = self.whatsapp.send_text_with_session(&task.target_jid, &err_msg, None, SessionRole::PrimaryBot).await;
                            }
                        }
                    } else {
                        warn!("Cannot execute AgentAction task #{}: AgentEngine is not configured", task.id);
                    }
                }
            }

            // 3. Compute and update next run time or mark as completed
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
                error!("Failed to update scheduled task #{} run state: {}", task.id, e);
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
