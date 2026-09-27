//! Periodic scheduler tick use case orchestrator.

mod background_watcher;
mod knowledge_linter;
mod task_executor;

pub use background_watcher::BackgroundWatcher;
pub use knowledge_linter::KnowledgeLinter;
pub use task_executor::TaskExecutor;

use crate::core::domain::{KnowledgeEngine, PersonaEngine};
use crate::core::ports::{AgentEnginePort, KnowledgePort, SessionStorePort, WhatsAppPort};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error};

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
        KnowledgeLinter::lint_workspace(self.knowledge_port.as_ref(), self.workspace_dir.as_deref());

        // 2. Query and process due scheduled tasks (Alarms, Reminders, and Autonomous Web Research)
        let now_epoch = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        TaskExecutor::execute_due_tasks(
            &self.session_store,
            &self.whatsapp,
            self.agent_engine.as_ref(),
            self.persona_engine.as_ref(),
            self.timezone_offset_hours,
            now_epoch,
        )
        .await;

        // 3. Autonomous Background Task Watcher & Self-Trigger
        if let Some(ref agent) = self.agent_engine {
            if let Err(e) = BackgroundWatcher::check_and_trigger(
                agent.as_ref(),
                &self.session_store,
                &self.whatsapp,
                &self.last_self_triggered,
                now_epoch,
            )
            .await
            {
                error!("Error in autonomous background task watcher: {}", e);
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

    struct MockFailingAgent;

    #[async_trait::async_trait]
    impl AgentEnginePort for MockFailingAgent {
        async fn execute_with_model(
            &self,
            _conversation_id: Option<&str>,
            _prompt: &str,
            _model_override: Option<&str>,
        ) -> anyhow::Result<crate::core::ports::AgentResponse> {
            anyhow::bail!("Antigravity CLI failed: Eligibility check failed: Your current account is not eligible for Antigravity.")
        }
        async fn get_model(&self) -> String { "gemini-3.8-flash".to_string() }
        async fn set_model(&self, _model: &str) -> anyhow::Result<()> { Ok(()) }
        async fn is_authenticated(&self) -> bool { true }
        async fn save_auth_token(&self, _token_content: &str) -> anyhow::Result<()> { Ok(()) }
    }

    struct TestRecordingWhatsApp {
        pub sent_messages: Arc<tokio::sync::Mutex<Vec<(String, String)>>>,
    }

    #[async_trait::async_trait]
    impl WhatsAppPort for TestRecordingWhatsApp {
        async fn send_text(&self, to: &str, text: &str, _qid: Option<&str>) -> anyhow::Result<()> {
            self.sent_messages.lock().await.push((to.to_string(), text.to_string()));
            Ok(())
        }
        async fn send_presence(&self, _to: &str, _state: crate::core::domain::PresenceState) -> anyhow::Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_scheduled_tick_agent_action_failure_routes_only_to_admin_never_to_group_or_status() {
        let dir = std::env::temp_dir().join(format!("aina_test_fail_route_{}", rand::random::<u32>()));
        let db_file = dir.join("fail_route_test.db");
        let store = Arc::new(SqliteSessionStore::new(&db_file).unwrap());

        let sent = Arc::new(tokio::sync::Mutex::new(Vec::new()));
        let whatsapp = Arc::new(TestRecordingWhatsApp {
            sent_messages: Arc::clone(&sent),
        });

        // 1. Group task that will fail
        let group_task = NewScheduledTask {
            title: "Cek Deadline Harian IPDS 6104".to_string(),
            task_type: ScheduledTaskType::AgentAction,
            target_jid: "120363377989532476@g.us".to_string(),
            payload: "Periksa deadline hari ini".to_string(),
            schedule_type: "once".to_string(),
            schedule_expr: "22:00".to_string(),
            next_run_epoch: 100,
        };
        store.create_scheduled_task(&group_task).await.unwrap();

        // 2. Status broadcast task that will fail
        let status_task = NewScheduledTask {
            title: "Status WhatsApp Malam".to_string(),
            task_type: ScheduledTaskType::AgentAction,
            target_jid: "status@broadcast".to_string(),
            payload: "Buat status senja".to_string(),
            schedule_type: "once".to_string(),
            schedule_expr: "22:00".to_string(),
            next_run_epoch: 100,
        };
        store.create_scheduled_task(&status_task).await.unwrap();

        let persona = Arc::new(PersonaEngine::new(
            "Persona text".to_string(),
            "Org text".to_string(),
            "6289625345646@s.whatsapp.net".to_string(),
            "Asia/Jakarta".to_string(),
            7,
            "id-ID".to_string(),
            "https://aina-wa.dvlpid.my.id".to_string(),
            "628982157341@s.whatsapp.net".to_string(),
            None,
        ));

        let usecase = ScheduledTickUseCase::new(
            Arc::clone(&store) as _,
            Arc::clone(&whatsapp) as _,
            Some(Arc::new(MockFailingAgent)),
            Some(persona),
            None,
            7,
        );

        // Execute tick
        usecase.execute().await.unwrap();

        let msgs = sent.lock().await.clone();
        // Should have received 2 failure reports, BOTH delivered to admin (6289625345646@s.whatsapp.net)!
        assert_eq!(msgs.len(), 2);

        for (target, text) in &msgs {
            // NEVER sent to the group or status!
            assert_ne!(target, "120363377989532476@g.us");
            assert_ne!(target, "status@broadcast");
            // Exclusively sent to companion/admin
            assert_eq!(target, "6289625345646@s.whatsapp.net");
            assert!(text.contains("🚨 *Laporan Kegagalan Tugas Terjadwal Aina*"));
            assert!(text.contains("Eligibility check failed"));
            assert!(text.contains("Rincian Lengkap Masalah"));
        }

        let _ = std::fs::remove_dir_all(&dir);
    }
}
