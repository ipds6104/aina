pub mod audit;
pub mod metacog;
pub mod scheduler;
pub mod schema;
pub mod session;

use crate::core::domain::{
    ActionAuditFilter, AuditSummaryReport, MetacognitiveCalibrationStats, MetacognitivePrediction,
    NewMetacognitivePrediction, NewScheduledTask, NewWhatsAppActionAudit, ScheduledTask,
    ScheduledTaskRun, SchedulerDiagnostics, WhatsAppActionAudit,
};
use crate::core::ports::{SessionStorePort, UserProfile};
use async_trait::async_trait;
use rusqlite::Connection;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::info;

pub struct SqliteSessionStore {
    conn: Arc<Mutex<Connection>>,
}

impl SqliteSessionStore {
    pub fn new<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        let path_ref = path.as_ref();
        if let Some(parent) = path_ref.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(path_ref)?;
        schema::init_schema(&conn)?;

        info!("SQLite database initialized at {:?}", path_ref);
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }
}

#[async_trait]
impl SessionStorePort for SqliteSessionStore {
    async fn get_conversation_id(&self, chat_jid: &str) -> anyhow::Result<Option<String>> {
        let conn = self.conn.lock().await;
        session::get_conversation_id(&conn, chat_jid)
    }

    async fn get_active_conversation_id(
        &self,
        chat_jid: &str,
        max_inactivity_secs: u64,
    ) -> anyhow::Result<Option<String>> {
        let conn = self.conn.lock().await;
        session::get_active_conversation_id(&conn, chat_jid, max_inactivity_secs)
    }

    async fn touch_conversation_activity(&self, chat_jid: &str) -> anyhow::Result<()> {
        let conn = self.conn.lock().await;
        session::touch_conversation_activity(&conn, chat_jid)
    }

    async fn search_message_history(
        &self,
        chat_jid: &str,
        sender_jid: Option<&str>,
        query: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<crate::core::ports::ChatMessageRecord>> {
        let conn = self.conn.lock().await;
        session::search_message_history(&conn, chat_jid, sender_jid, query, limit)
    }

    async fn get_recent_messages(
        &self,
        chat_jid: &str,
        sender_jid: Option<&str>,
        limit: usize,
    ) -> anyhow::Result<Vec<crate::core::ports::ChatMessageRecord>> {
        let conn = self.conn.lock().await;
        session::get_recent_messages(&conn, chat_jid, sender_jid, limit)
    }


    async fn save_conversation_id(&self, chat_jid: &str, conv_uuid: &str) -> anyhow::Result<()> {
        let conn = self.conn.lock().await;
        session::save_conversation_id(&conn, chat_jid, conv_uuid)
    }

    async fn delete_conversation_id(&self, chat_jid: &str) -> anyhow::Result<()> {
        let conn = self.conn.lock().await;
        session::delete_conversation_id(&conn, chat_jid)
    }

    async fn record_message(
        &self,
        chat_jid: &str,
        sender_jid: &str,
        text: &str,
        is_from_me: bool,
    ) -> anyhow::Result<()> {
        let conn = self.conn.lock().await;
        session::record_message(&conn, chat_jid, sender_jid, text, is_from_me)
    }

    async fn get_user_profile(&self, sender_jid: &str) -> anyhow::Result<Option<UserProfile>> {
        let conn = self.conn.lock().await;
        session::get_user_profile(&conn, sender_jid)
    }

    async fn save_user_profile(&self, profile: &UserProfile) -> anyhow::Result<()> {
        let conn = self.conn.lock().await;
        session::save_user_profile(&conn, profile)
    }

    async fn create_scheduled_task(&self, task: &NewScheduledTask) -> anyhow::Result<i64> {
        let conn = self.conn.lock().await;
        scheduler::create_scheduled_task(&conn, task)
    }

    async fn list_scheduled_tasks(&self, active_only: bool) -> anyhow::Result<Vec<ScheduledTask>> {
        let conn = self.conn.lock().await;
        scheduler::list_scheduled_tasks(&conn, active_only)
    }

    async fn get_due_scheduled_tasks(&self, current_epoch: i64) -> anyhow::Result<Vec<ScheduledTask>> {
        let conn = self.conn.lock().await;
        scheduler::get_due_scheduled_tasks(&conn, current_epoch)
    }

    async fn update_scheduled_task_run(
        &self,
        id: i64,
        last_run: i64,
        next_run: Option<i64>,
        is_active: bool,
    ) -> anyhow::Result<()> {
        let conn = self.conn.lock().await;
        scheduler::update_scheduled_task_run(&conn, id, last_run, next_run, is_active)
    }

    async fn delete_scheduled_task(&self, id: i64) -> anyhow::Result<bool> {
        let conn = self.conn.lock().await;
        scheduler::delete_scheduled_task(&conn, id)
    }

    async fn get_scheduled_task(&self, id: i64) -> anyhow::Result<Option<ScheduledTask>> {
        let conn = self.conn.lock().await;
        scheduler::get_scheduled_task(&conn, id)
    }

    async fn record_scheduled_task_run(
        &self,
        task_id: i64,
        task_title: &str,
        target_jid: &str,
        status: &str,
        duration_secs: f64,
        error_message: Option<&str>,
        output_preview: Option<&str>,
    ) -> anyhow::Result<i64> {
        let conn = self.conn.lock().await;
        scheduler::record_scheduled_task_run(
            &conn,
            task_id,
            task_title,
            target_jid,
            status,
            duration_secs,
            error_message,
            output_preview,
        )
    }

    async fn list_scheduled_task_runs(&self, limit: usize) -> anyhow::Result<Vec<ScheduledTaskRun>> {
        let conn = self.conn.lock().await;
        scheduler::list_scheduled_task_runs(&conn, limit)
    }

    async fn update_scheduled_task_result(
        &self,
        id: i64,
        status: &str,
        error_message: Option<&str>,
        duration_secs: f64,
    ) -> anyhow::Result<()> {
        let conn = self.conn.lock().await;
        scheduler::update_scheduled_task_result(&conn, id, status, error_message, duration_secs)
    }

    async fn get_scheduler_diagnostics(&self) -> anyhow::Result<SchedulerDiagnostics> {
        let conn = self.conn.lock().await;
        scheduler::get_scheduler_diagnostics(&conn)
    }

    async fn record_action_audit(&self, audit: &NewWhatsAppActionAudit) -> anyhow::Result<i64> {
        let conn = self.conn.lock().await;
        audit::record_action_audit(&conn, audit)
    }

    async fn update_action_audit_result(
        &self,
        id: i64,
        conversation_id: Option<&str>,
        response_text: Option<&str>,
        error_message: Option<&str>,
        status: &str,
        duration_seconds: Option<f64>,
        tools_invoked: &[String],
        usecase: Option<&str>,
    ) -> anyhow::Result<()> {
        let conn = self.conn.lock().await;
        audit::update_action_audit_result(
            &conn,
            id,
            conversation_id,
            response_text,
            error_message,
            status,
            duration_seconds,
            tools_invoked,
            usecase,
        )
    }

    async fn query_action_audits(
        &self,
        filter: &ActionAuditFilter,
    ) -> anyhow::Result<Vec<WhatsAppActionAudit>> {
        let conn = self.conn.lock().await;
        audit::query_action_audits(&conn, filter)
    }

    async fn get_action_audit_by_id(&self, id: i64) -> anyhow::Result<Option<WhatsAppActionAudit>> {
        let conn = self.conn.lock().await;
        audit::get_action_audit_by_id(&conn, id)
    }

    async fn get_action_audit_by_message_id(&self, message_id: &str) -> anyhow::Result<Option<WhatsAppActionAudit>> {
        let conn = self.conn.lock().await;
        audit::get_action_audit_by_message_id(&conn, message_id)
    }

    async fn get_action_audit_summary(&self) -> anyhow::Result<AuditSummaryReport> {
        let conn = self.conn.lock().await;
        audit::get_action_audit_summary(&conn)
    }

    async fn record_metacognitive_prediction(
        &self,
        pred: &NewMetacognitivePrediction,
    ) -> anyhow::Result<i64> {
        let conn = self.conn.lock().await;
        metacog::record_metacognitive_prediction(&conn, pred)
    }

    async fn resolve_metacognitive_prediction(
        &self,
        prediction_id: &str,
        actual_outcome: f64,
        duration_secs: f64,
        error_detail: Option<&str>,
    ) -> anyhow::Result<()> {
        let conn = self.conn.lock().await;
        metacog::resolve_metacognitive_prediction(&conn, prediction_id, actual_outcome, duration_secs, error_detail)
    }

    async fn list_metacognitive_predictions(
        &self,
        limit: usize,
        domain_filter: Option<&str>,
    ) -> anyhow::Result<Vec<MetacognitivePrediction>> {
        let conn = self.conn.lock().await;
        metacog::list_metacognitive_predictions(&conn, limit, domain_filter)
    }

    async fn get_metacognitive_calibration_stats(&self) -> anyhow::Result<MetacognitiveCalibrationStats> {
        let conn = self.conn.lock().await;
        metacog::get_metacognitive_calibration_stats(&conn)
    }

    async fn record_group_membership(
        &self,
        group_jid: &str,
        user_jid: &str,
        user_name: Option<&str>,
        role_in_group: Option<&str>,
    ) -> anyhow::Result<()> {
        let conn = self.conn.lock().await;
        session::record_group_membership(&conn, group_jid, user_jid, user_name, role_in_group)
    }

    async fn get_user_groups(&self, user_jid: &str) -> anyhow::Result<Vec<String>> {
        let conn = self.conn.lock().await;
        session::get_user_groups(&conn, user_jid)
    }

    async fn get_group_members(&self, group_jid: &str) -> anyhow::Result<Vec<crate::core::ports::GroupMemberRecord>> {
        let conn = self.conn.lock().await;
        session::get_group_members(&conn, group_jid)
    }

    async fn is_user_in_group(&self, user_jid: &str, group_jid: &str) -> anyhow::Result<bool> {
        let conn = self.conn.lock().await;
        session::is_user_in_group(&conn, user_jid, group_jid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fts5_support() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute("CREATE VIRTUAL TABLE test_fts USING fts5(content);", []).unwrap();
        conn.execute("INSERT INTO test_fts (content) VALUES ('halo dunia');", []).unwrap();
        let mut stmt = conn.prepare("SELECT content FROM test_fts WHERE test_fts MATCH 'dunia'").unwrap();
        let res: String = stmt.query_row([], |r| r.get(0)).unwrap();
        assert_eq!(res, "halo dunia");
    }

    #[tokio::test]
    async fn test_scheduled_tasks_crud_and_due_query() {
        let dir = std::env::temp_dir().join(format!("aina_test_sched_{}", rand::random::<u32>()));
        let db_file = dir.join("test.db");
        let store = SqliteSessionStore::new(&db_file).unwrap();

        let new_task = crate::core::domain::NewScheduledTask {
            title: "Pengingat YouTube".to_string(),
            task_type: crate::core::domain::ScheduledTaskType::DirectNotification,
            target_jid: "6289625345646@s.whatsapp.net".to_string(),
            payload: "Buka YouTube ya!".to_string(),
            schedule_type: "once".to_string(),
            schedule_expr: "22:26".to_string(),
            next_run_epoch: 1000,
        };

        let task_id = store.create_scheduled_task(&new_task).await.unwrap();
        assert!(task_id > 0);

        // Deduplication test: adding task with same target & close timestamp should return same task_id
        let duplicate_task = crate::core::domain::NewScheduledTask {
            title: "Pengingat YouTube (Updated)".to_string(),
            task_type: crate::core::domain::ScheduledTaskType::DirectNotification,
            target_jid: "6289625345646@s.whatsapp.net".to_string(),
            payload: "Buka YouTube ya, jangan lupa!".to_string(),
            schedule_type: "once".to_string(),
            schedule_expr: "22:26".to_string(),
            next_run_epoch: 1000,
        };
        let dup_id = store.create_scheduled_task(&duplicate_task).await.unwrap();
        assert_eq!(task_id, dup_id);

        let active_tasks = store.list_scheduled_tasks(true).await.unwrap();
        assert_eq!(active_tasks.len(), 1);
        assert_eq!(active_tasks[0].title, "Pengingat YouTube (Updated)");

        // Query when now < 1000 -> not due yet
        let due_before = store.get_due_scheduled_tasks(999).await.unwrap();
        assert_eq!(due_before.len(), 0);

        // Query when now >= 1000 -> due!
        let due_after = store.get_due_scheduled_tasks(1000).await.unwrap();
        assert_eq!(due_after.len(), 1);
        assert_eq!(due_after[0].id, task_id);

        // Update run
        store.update_scheduled_task_run(task_id, 1000, None, false).await.unwrap();
        let active_after = store.list_scheduled_tasks(true).await.unwrap();
        assert_eq!(active_after.len(), 0);

        // Clean up
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_action_audits_crud_filter_and_summary() {
        let dir = std::env::temp_dir().join(format!("aina_test_audit_db_{}", rand::random::<u32>()));
        let db_file = dir.join("test_audit.db");
        let store = SqliteSessionStore::new(&db_file).unwrap();

        let new_audit = crate::core::domain::NewWhatsAppActionAudit {
            message_id: "MSG_AUDIT_101".to_string(),
            chat_jid: "628123456789@s.whatsapp.net".to_string(),
            chat_type: "direct".to_string(),
            sender_jid: "628123456789@s.whatsapp.net".to_string(),
            sender_name: Some("Budi".to_string()),
            decision: "respond".to_string(),
            decision_reason: "Direct message always responds".to_string(),
            conversation_id: None,
            status: "in_progress".to_string(),
            input_text: "Halo tolong bantu ekstrak data".to_string(),
            has_media: false,
            media_path: None,
            response_text: None,
            error_message: None,
            duration_seconds: None,
            tools_invoked: vec![],
            usecase: "data_analysis".to_string(),
            created_at_epoch: 1774000000,
            completed_at_epoch: None,
        };

        let audit_id = store.record_action_audit(&new_audit).await.unwrap();
        assert!(audit_id > 0);

        // Fetch by id
        let fetched = store.get_action_audit_by_id(audit_id).await.unwrap().unwrap();
        assert_eq!(fetched.message_id, "MSG_AUDIT_101");
        assert_eq!(fetched.status, "in_progress");
        assert_eq!(fetched.usecase, "data_analysis");

        // Fetch by message_id
        let fetched_by_msg = store.get_action_audit_by_message_id("MSG_AUDIT_101").await.unwrap().unwrap();
        assert_eq!(fetched_by_msg.id, audit_id);
        assert_eq!(fetched_by_msg.usecase, "data_analysis");

        // Update result
        store.update_action_audit_result(
            audit_id,
            Some("conv-uuid-12345"),
            Some("Ini hasil ekstraksi data Anda"),
            None,
            "success",
            Some(2.45),
            &["run_command".to_string(), "view_file".to_string()],
            Some("data_analysis"),
        ).await.unwrap();

        let updated = store.get_action_audit_by_id(audit_id).await.unwrap().unwrap();
        assert_eq!(updated.status, "success");
        assert_eq!(updated.conversation_id.as_deref(), Some("conv-uuid-12345"));
        assert_eq!(updated.duration_seconds, Some(2.45));
        assert_eq!(updated.tools_invoked, vec!["run_command".to_string(), "view_file".to_string()]);
        assert_eq!(updated.usecase, "data_analysis");

        // Query filter by status, usecase, and tool
        let filter = crate::core::domain::ActionAuditFilter {
            status: Some("success".to_string()),
            usecase: Some("data_analysis".to_string()),
            tool: Some("run_command".to_string()),
            ..Default::default()
        };
        let query_res = store.query_action_audits(&filter).await.unwrap();
        assert_eq!(query_res.len(), 1);

        // Summary report
        let summary = store.get_action_audit_summary().await.unwrap();
        assert_eq!(summary.total_actions, 1);
        assert_eq!(summary.total_responses, 1);
        assert_eq!(summary.total_errors, 0);
        assert!((summary.avg_duration_seconds - 2.45).abs() < 0.01);
        assert_eq!(summary.most_active_chats.len(), 1);
        assert_eq!(summary.top_tools_used.len(), 2);
        assert_eq!(summary.top_usecases.len(), 1);
        assert_eq!(summary.top_usecases[0].key, "data_analysis");
        assert_eq!(summary.top_usecases[0].count, 1);
        assert_eq!(summary.tool_call_frequency.len(), 2);

        // Clean up
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_active_session_lifecycle_and_message_retrieval() {
        let dir = std::env::temp_dir().join(format!("aina_test_active_sess_{}", rand::random::<u32>()));
        let db_file = dir.join("test.db");
        let store = SqliteSessionStore::new(&db_file).unwrap();

        let chat = "6289999999@s.whatsapp.net";

        // Initially no conversation
        assert_eq!(store.get_active_conversation_id(chat, 7200).await.unwrap(), None);

        // Save conversation
        store.save_conversation_id(chat, "conv-active-uuid-1").await.unwrap();

        // Immediately active within 7200s window
        let active = store.get_active_conversation_id(chat, 7200).await.unwrap();
        assert_eq!(active.as_deref(), Some("conv-active-uuid-1"));

        // Touch activity
        store.touch_conversation_activity(chat).await.unwrap();
        let active2 = store.get_active_conversation_id(chat, 7200).await.unwrap();
        assert_eq!(active2.as_deref(), Some("conv-active-uuid-1"));

        // Test with 0 seconds window (or simulate expiration)
        // With 0 max_inactivity_secs, elapsed is >= 0 so immediately expires if elapsed > 0
        // Record message history
        store.record_message(chat, chat, "Halo Aina, tolong bantu setting coolify", false).await.unwrap();
        store.record_message(chat, "bot@s.whatsapp.net", "Halo! Tentu, coolify sudah siap", true).await.unwrap();
        store.record_message(chat, chat, "Berapa port default coolify?", false).await.unwrap();

        // Search message history
        let search_results = store.search_message_history(chat, None, "coolify", 5).await.unwrap();
        assert_eq!(search_results.len(), 3);
        assert!(search_results.iter().any(|m| m.text.contains("setting coolify")));

        assert!(search_results.iter().any(|m| m.text.contains("default coolify")));

        // Search message history empty query
        let empty_search = store.search_message_history(chat, None, "", 5).await.unwrap();
        assert!(empty_search.is_empty());

        // Get recent messages
        let recent = store.get_recent_messages(chat, None, 2).await.unwrap();
        assert_eq!(recent.len(), 2);
        // Chronological order: first is bot, then user
        assert_eq!(recent[0].text, "Halo! Tentu, coolify sudah siap");
        assert_eq!(recent[1].text, "Berapa port default coolify?");

        // Clean up
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_group_memberships_crud_and_cross_channel_resolution() {
        let dir = std::env::temp_dir().join(format!("aina_test_group_mem_{}", rand::random::<u32>()));
        let db_file = dir.join("test.db");
        let store = SqliteSessionStore::new(&db_file).unwrap();

        let group_jid = "120363253842861469@g.us";
        let user_jid = "628111222333@s.whatsapp.net";

        // Initially no group
        let groups_init = store.get_user_groups(user_jid).await.unwrap();
        assert!(groups_init.is_empty());
        assert!(!store.is_user_in_group(user_jid, group_jid).await.unwrap());

        // Record membership
        store.record_group_membership(group_jid, user_jid, Some("Adwin Haithay"), Some("member")).await.unwrap();

        // Check user groups
        let groups = store.get_user_groups(user_jid).await.unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0], group_jid);
        assert!(store.is_user_in_group(user_jid, group_jid).await.unwrap());

        // Check group members
        let members = store.get_group_members(group_jid).await.unwrap();
        assert_eq!(members.len(), 1);
        assert_eq!(members[0].user_jid, user_jid);
        assert_eq!(members[0].user_name.as_deref(), Some("Adwin Haithay"));
        assert_eq!(members[0].role_in_group, "member");

        let _ = std::fs::remove_dir_all(&dir);
    }
}

