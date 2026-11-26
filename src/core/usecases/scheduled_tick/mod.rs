//! Periodic scheduler tick use case orchestrator.

mod background_watcher;
pub mod command_runner;
mod knowledge_linter;
mod task_executor;

pub use background_watcher::BackgroundWatcher;
pub use command_runner::{
    run_command_with_pgroup_timeout, CommandEnvConfig, SubprocessOutcome,
};
pub use knowledge_linter::KnowledgeLinter;
pub use task_executor::{TaskExecutionContext, TaskExecutor};

use crate::core::domain::{KnowledgeEngine, PersonaEngine};
use crate::core::ports::{AgentEnginePort, KnowledgePort, SessionStorePort, WhatsAppPort};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, RwLock, Semaphore};
use tracing::{debug, error};

pub struct ScheduledTickUseCase {
    session_store: Arc<dyn SessionStorePort>,
    whatsapp: Arc<dyn WhatsAppPort>,
    agent_engine: Option<Arc<dyn AgentEnginePort>>,
    persona_engine: Option<Arc<PersonaEngine>>,
    knowledge_port: Arc<dyn KnowledgePort>,
    workspace_dir: Option<PathBuf>,
    command_env: Option<CommandEnvConfig>,
    timezone_offset_hours: i32,
    task_timeout: Duration,
    command_timeout: Duration,
    agent_semaphore: Arc<Semaphore>,
    in_flight_tasks: Arc<Mutex<HashSet<i64>>>,
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
            command_env: None,
            timezone_offset_hours,
            task_timeout: Duration::from_secs(600),
            command_timeout: Duration::from_secs(180),
            agent_semaphore: Arc::new(Semaphore::new(3)),
            in_flight_tasks: Arc::new(Mutex::new(HashSet::new())),
            last_self_triggered: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Configures per-task hard timeouts, concurrency ceiling, and command credentials.
    pub fn with_runtime_limits(
        mut self,
        task_timeout_seconds: u64,
        command_timeout_seconds: u64,
        max_concurrent_tasks: usize,
        command_env: Option<CommandEnvConfig>,
    ) -> Self {
        self.task_timeout = Duration::from_secs(task_timeout_seconds.max(1));
        self.command_timeout = Duration::from_secs(command_timeout_seconds.max(1));
        self.agent_semaphore = Arc::new(Semaphore::new(max_concurrent_tasks.clamp(1, 16)));
        self.command_env = command_env;
        self
    }

    /// Returns the number of scheduled tasks currently executing in the background.
    pub async fn in_flight_count(&self) -> usize {
        self.in_flight_tasks.lock().await.len()
    }

    fn build_exec_context(&self) -> TaskExecutionContext {
        TaskExecutionContext {
            session_store: Arc::clone(&self.session_store),
            whatsapp: Arc::clone(&self.whatsapp),
            agent_engine: self.agent_engine.clone(),
            persona_engine: self.persona_engine.clone(),
            workspace_dir: self.workspace_dir.clone(),
            command_env: self.command_env.clone(),
            timezone_offset_hours: self.timezone_offset_hours,
            task_timeout: self.task_timeout,
            command_timeout: self.command_timeout,
            agent_semaphore: Arc::clone(&self.agent_semaphore),
            in_flight_tasks: Arc::clone(&self.in_flight_tasks),
        }
    }

    /// Runs a tick and waits for all due tasks dispatched in this tick to finish
    /// (executing them concurrently). Used by deterministic tests and synchronous triggers.
    #[allow(dead_code)]
    pub async fn execute(&self) -> anyhow::Result<()> {
        self.run_tick(true).await
    }

    /// Non-blocking production daemon tick: dispatches due tasks onto Tokio workers
    /// so the 60s ticker never stalls behind slow agent or script executions.
    pub async fn execute_non_blocking(&self) -> anyhow::Result<()> {
        self.run_tick(false).await
    }

    async fn run_tick(&self, wait_for_completion: bool) -> anyhow::Result<()> {
        debug!("Running periodic scheduler tick...");

        // 1. In-process deterministic Knowledge Base neatness check & auto-heal
        KnowledgeLinter::lint_workspace(self.knowledge_port.as_ref(), self.workspace_dir.as_deref());

        // 2. Query and dispatch due scheduled tasks concurrently (Alarms, DirectCommands, AgentActions)
        let now_epoch = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        TaskExecutor::execute_due_tasks(
            self.build_exec_context(),
            now_epoch,
            wait_for_completion,
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
            slug: None,
            category: None,
            source: None,
            metadata: None,
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
            slug: None,
            category: None,
            source: None,
            metadata: None,
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
            slug: None,
            category: None,
            source: None,
            metadata: None,
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
            assert!(text.contains("*Laporan Kegagalan Tugas Terjadwal Aina*"));
            assert!(text.contains("Eligibility check failed"));
            assert!(text.contains("Rincian Lengkap Masalah"));
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    struct MockSilentAgent;

    #[async_trait::async_trait]
    impl AgentEnginePort for MockSilentAgent {
        async fn execute_with_model(
            &self,
            _conversation_id: Option<&str>,
            _prompt: &str,
            _model_override: Option<&str>,
        ) -> anyhow::Result<crate::core::ports::AgentResponse> {
            Ok(crate::core::ports::AgentResponse {
                response_text: "I have launched the monitoring rendering and WhatsApp dispatch task in the background. I will wait for it to complete.\nMenunggu proses rendering dan pengiriman selesai...\n[NO_SEND]".to_string(),
                conversation_id: "conv_test_silent".to_string(),
                duration_seconds: 0.1,
            })
        }
        async fn get_model(&self) -> String { "gemini-3.8-flash".to_string() }
        async fn set_model(&self, _model: &str) -> anyhow::Result<()> { Ok(()) }
        async fn is_authenticated(&self) -> bool { true }
        async fn save_auth_token(&self, _token_content: &str) -> anyhow::Result<()> { Ok(()) }
    }

    #[tokio::test]
    async fn test_scheduled_tick_agent_action_silent_skip_and_internal_silent_never_sends_to_group() {
        let dir = std::env::temp_dir().join(format!("aina_test_silent_{}", rand::random::<u32>()));
        let db_file = dir.join("silent_test.db");
        let store = Arc::new(SqliteSessionStore::new(&db_file).unwrap());

        let sent = Arc::new(tokio::sync::Mutex::new(Vec::new()));
        let whatsapp = Arc::new(TestRecordingWhatsApp {
            sent_messages: Arc::clone(&sent),
        });

        // Group task where payload contains [NO_SEND]
        let group_task = NewScheduledTask {
            title: "Monitoring Harian Matching SBR SE2026".to_string(),
            task_type: ScheduledTaskType::AgentAction,
            target_jid: "120363433281094659@g.us".to_string(),
            payload: "Jalankan render rekap lalu akhiri dengan [NO_SEND]".to_string(),
            schedule_type: "once".to_string(),
            schedule_expr: "07:00".to_string(),
            next_run_epoch: 100,
            slug: None,
            category: None,
            source: None,
            metadata: None,
        };
        let _task_id = store.create_scheduled_task(&group_task).await.unwrap();

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
            Some(Arc::new(MockSilentAgent)),
            Some(persona),
            None,
            7,
        );

        // Execute tick
        usecase.execute().await.unwrap();

        let msgs = sent.lock().await.clone();
        // ZERO messages sent because it requested silent skip and was internal silent!
        assert_eq!(msgs.len(), 0);

        // Task should be marked as success in runs history
        let runs = store.list_scheduled_task_runs(1).await.unwrap();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].status, "success");

        let _ = std::fs::remove_dir_all(&dir);
    }

    struct SlowRecordingAgent {
        pub delay: Duration,
        pub observed_timeout: Arc<Mutex<Option<Duration>>>,
    }

    #[async_trait::async_trait]
    impl AgentEnginePort for SlowRecordingAgent {
        async fn execute_with_model(
            &self,
            _conversation_id: Option<&str>,
            _prompt: &str,
            _model_override: Option<&str>,
        ) -> anyhow::Result<crate::core::ports::AgentResponse> {
            tokio::time::sleep(self.delay).await;
            Ok(crate::core::ports::AgentResponse {
                conversation_id: "conv_slow".to_string(),
                response_text: "Slow done".to_string(),
                duration_seconds: self.delay.as_secs_f64(),
            })
        }

        async fn execute_with_capability_and_timeout(
            &self,
            conversation_id: Option<&str>,
            prompt: &str,
            _capability: Option<&crate::core::domain::capability::TurnCapability>,
            timeout_override: Option<Duration>,
        ) -> anyhow::Result<crate::core::ports::AgentResponse> {
            *self.observed_timeout.lock().await = timeout_override;
            self.execute_with_model(conversation_id, prompt, None).await
        }

        async fn get_model(&self) -> String { "gemini-3.8-flash".to_string() }
        async fn set_model(&self, _model: &str) -> anyhow::Result<()> { Ok(()) }
        async fn is_authenticated(&self) -> bool { true }
        async fn save_auth_token(&self, _token_content: &str) -> anyhow::Result<()> { Ok(()) }
    }

    #[tokio::test]
    async fn test_non_blocking_tick_delivers_notification_while_slow_agent_runs_and_passes_timeout() {
        let dir = std::env::temp_dir().join(format!("aina_test_nonblock_{}", rand::random::<u32>()));
        let db_file = dir.join("nonblock.db");
        let store = Arc::new(SqliteSessionStore::new(&db_file).unwrap());

        let sent = Arc::new(tokio::sync::Mutex::new(Vec::new()));
        let whatsapp = Arc::new(TestRecordingWhatsApp {
            sent_messages: Arc::clone(&sent),
        });
        let observed_timeout = Arc::new(Mutex::new(None));
        let slow_agent = Arc::new(SlowRecordingAgent {
            delay: Duration::from_millis(600),
            observed_timeout: Arc::clone(&observed_timeout),
        });

        // 1. Slow AgentAction scheduled BEFORE fast DirectNotification
        store.create_scheduled_task(&NewScheduledTask {
            title: "Slow Agent Monitoring".to_string(),
            task_type: ScheduledTaskType::AgentAction,
            target_jid: "1203630001@g.us".to_string(),
            payload: "Run slow check".to_string(),
            schedule_type: "once".to_string(),
            schedule_expr: "07:30".to_string(),
            next_run_epoch: 100,
            slug: None,
            category: None,
            source: None,
            metadata: None,
        }).await.unwrap();

        // 2. Fast DirectNotification due right after
        store.create_scheduled_task(&NewScheduledTask {
            title: "Fast Reminder".to_string(),
            task_type: ScheduledTaskType::DirectNotification,
            target_jid: "6289625345646@s.whatsapp.net".to_string(),
            payload: "Jangan lupa rapat!".to_string(),
            schedule_type: "once".to_string(),
            schedule_expr: "08:00".to_string(),
            next_run_epoch: 110,
            slug: None,
            category: None,
            source: None,
            metadata: None,
        }).await.unwrap();

        let usecase = ScheduledTickUseCase::new(
            Arc::clone(&store) as _,
            Arc::clone(&whatsapp) as _,
            Some(slow_agent),
            None,
            None,
            7,
        )
        .with_runtime_limits(420, 90, 3, None);

        let tick_start = std::time::Instant::now();
        usecase.execute_non_blocking().await.unwrap();
        // Tick MUST return immediately without waiting for the 600ms slow agent!
        assert!(tick_start.elapsed() < Duration::from_millis(250));

        // Wait briefly: DirectNotification MUST already have arrived while slow agent is still running!
        tokio::time::sleep(Duration::from_millis(150)).await;
        {
            let msgs = sent.lock().await;
            assert_eq!(msgs.len(), 1);
            assert_eq!(msgs[0].1, "Jangan lupa rapat!");
        }
        assert_eq!(usecase.in_flight_count().await, 1);

        // Wait for slow agent to finish
        tokio::time::sleep(Duration::from_millis(700)).await;
        assert_eq!(usecase.in_flight_count().await, 0);
        assert_eq!(*observed_timeout.lock().await, Some(Duration::from_secs(420)));
        assert_eq!(sent.lock().await.len(), 2);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_direct_command_succeeds_and_failure_routes_exclusively_to_admin() {
        let dir = std::env::temp_dir().join(format!("aina_test_dcmd_{}", rand::random::<u32>()));
        let db_file = dir.join("dcmd.db");
        let store = Arc::new(SqliteSessionStore::new(&db_file).unwrap());

        let sent = Arc::new(tokio::sync::Mutex::new(Vec::new()));
        let whatsapp = Arc::new(TestRecordingWhatsApp {
            sent_messages: Arc::clone(&sent),
        });
        let persona = Arc::new(PersonaEngine::new(
            "Persona".to_string(),
            "Org".to_string(),
            "6289625345646@s.whatsapp.net".to_string(),
            "Asia/Jakarta".to_string(),
            7,
            "id-ID".to_string(),
            "https://aina-wa.dvlpid.my.id".to_string(),
            "628982157341@s.whatsapp.net".to_string(),
            None,
        ));

        // 1. Succeeding direct command targeting a group (must NOT spam stdout to group)
        let ok_id = store.create_scheduled_task(&NewScheduledTask {
            title: "Deterministic Sync".to_string(),
            task_type: ScheduledTaskType::DirectCommand,
            target_jid: "120363432058158028@g.us".to_string(),
            payload: "echo sync_ok".to_string(),
            schedule_type: "once".to_string(),
            schedule_expr: "07:30".to_string(),
            next_run_epoch: 100,
            slug: None,
            category: None,
            source: None,
            metadata: None,
        }).await.unwrap();

        // 2. Failing direct command targeting a group (error MUST go ONLY to admin DM;
        //    next_run_epoch spaced >120s apart to avoid idempotency upsert with task #1)
        let fail_id = store.create_scheduled_task(&NewScheduledTask {
            title: "Broken Screenshot Script".to_string(),
            task_type: ScheduledTaskType::DirectCommand,
            target_jid: "120363432058158028@g.us".to_string(),
            payload: "echo chromium_crash >&2; exit 2".to_string(),
            schedule_type: "once".to_string(),
            schedule_expr: "07:35".to_string(),
            next_run_epoch: 300,
            slug: None,
            category: None,
            source: None,
            metadata: None,
        }).await.unwrap();
        assert_ne!(ok_id, fail_id);

        let usecase = ScheduledTickUseCase::new(
            Arc::clone(&store) as _,
            Arc::clone(&whatsapp) as _,
            None,
            Some(persona),
            None,
            7,
        )
        .with_runtime_limits(60, 10, 3, None);

        usecase.execute().await.unwrap();

        let ok_task = store.get_scheduled_task(ok_id).await.unwrap().unwrap();
        assert_eq!(ok_task.last_status.as_deref(), Some("success"));

        let fail_task = store.get_scheduled_task(fail_id).await.unwrap().unwrap();
        assert_eq!(fail_task.last_status.as_deref(), Some("failed"));

        let msgs = sent.lock().await;
        // Only the failure report should have been sent, exclusively to Admin DM
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].0, "6289625345646@s.whatsapp.net");
        assert!(msgs[0].1.contains("chromium_crash"));
        assert!(msgs[0].1.contains("direct_command"));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
