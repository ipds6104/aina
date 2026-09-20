use crate::core::ports::SessionStorePort;
use async_trait::async_trait;
use rusqlite::{params, Connection};
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
        
        // Resilience against power cuts & high concurrency: Enable Write-Ahead Logging (WAL)
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA busy_timeout = 5000;"
        )?;
        
        // Initialize tables
        conn.execute(
            "CREATE TABLE IF NOT EXISTS chats (
                chat_jid TEXT PRIMARY KEY,
                conversation_uuid TEXT NOT NULL,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS message_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                chat_jid TEXT NOT NULL,
                sender_jid TEXT NOT NULL,
                text TEXT NOT NULL,
                is_from_me BOOLEAN NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS user_profiles (
                sender_jid TEXT PRIMARY KEY,
                name TEXT,
                role TEXT,
                authority_level TEXT NOT NULL DEFAULT 'staff',
                notes TEXT,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS scheduled_tasks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                title TEXT NOT NULL,
                task_type TEXT NOT NULL,
                target_jid TEXT NOT NULL,
                payload TEXT NOT NULL,
                schedule_type TEXT NOT NULL,
                schedule_expr TEXT NOT NULL,
                next_run_epoch INTEGER NOT NULL,
                last_run_epoch INTEGER,
                is_active BOOLEAN NOT NULL DEFAULT 1,
                created_at INTEGER NOT NULL,
                last_status TEXT,
                last_error TEXT,
                last_duration_secs REAL
            )",
            [],
        )?;

        // Ensure columns exist on legacy databases
        let _ = conn.execute("ALTER TABLE scheduled_tasks ADD COLUMN last_status TEXT", []);
        let _ = conn.execute("ALTER TABLE scheduled_tasks ADD COLUMN last_error TEXT", []);
        let _ = conn.execute("ALTER TABLE scheduled_tasks ADD COLUMN last_duration_secs REAL", []);

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_scheduled_tasks_next_run 
             ON scheduled_tasks(is_active, next_run_epoch)",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS scheduled_task_runs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                task_id INTEGER NOT NULL,
                task_title TEXT NOT NULL,
                target_jid TEXT NOT NULL,
                status TEXT NOT NULL,
                duration_secs REAL NOT NULL,
                error_message TEXT,
                output_preview TEXT,
                executed_at_epoch INTEGER NOT NULL
            )",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_task_runs_task_id 
             ON scheduled_task_runs(task_id, executed_at_epoch DESC)",
            [],
        )?;

        // WhatsApp Action Audits Table & Indexes
        conn.execute(
            "CREATE TABLE IF NOT EXISTS whatsapp_action_audits (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                message_id TEXT NOT NULL,
                chat_jid TEXT NOT NULL,
                chat_type TEXT NOT NULL,
                sender_jid TEXT NOT NULL,
                sender_name TEXT,
                decision TEXT NOT NULL,
                decision_reason TEXT NOT NULL,
                conversation_id TEXT,
                status TEXT NOT NULL,
                input_text TEXT NOT NULL,
                has_media BOOLEAN NOT NULL DEFAULT 0,
                media_path TEXT,
                response_text TEXT,
                error_message TEXT,
                duration_seconds REAL,
                tools_invoked TEXT NOT NULL DEFAULT '[]',
                created_at_epoch INTEGER NOT NULL,
                completed_at_epoch INTEGER
            )",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_action_audits_chat 
             ON whatsapp_action_audits(chat_jid, created_at_epoch DESC)",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_action_audits_sender 
             ON whatsapp_action_audits(sender_jid, created_at_epoch DESC)",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_action_audits_status 
             ON whatsapp_action_audits(status, created_at_epoch DESC)",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_action_audits_msg_id 
             ON whatsapp_action_audits(message_id)",
            [],
        )?;

        // Metacognitive Predictions Table & Indexes (Syarat 1 & 2 Definisi D)
        conn.execute(
            "CREATE TABLE IF NOT EXISTS metacognitive_predictions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                prediction_id TEXT NOT NULL UNIQUE,
                action_audit_id INTEGER,
                task_description TEXT NOT NULL,
                domain_type TEXT NOT NULL,
                predicted_probability REAL NOT NULL,
                complexity_tier TEXT NOT NULL,
                identified_risks TEXT NOT NULL DEFAULT '[]',
                fallback_strategy TEXT,
                actual_outcome REAL,
                brier_score REAL,
                execution_duration_secs REAL,
                error_detail TEXT,
                created_at_epoch INTEGER NOT NULL,
                resolved_at_epoch INTEGER
            )",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_metacog_domain 
             ON metacognitive_predictions(domain_type, created_at_epoch DESC)",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_metacog_created 
             ON metacognitive_predictions(created_at_epoch DESC)",
            [],
        )?;

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
        let mut stmt = conn.prepare(
            "SELECT conversation_uuid FROM chats WHERE chat_jid = ?1 LIMIT 1"
        )?;
        
        let mut rows = stmt.query(params![chat_jid])?;
        if let Some(row) = rows.next()? {
            let uuid: String = row.get(0)?;
            Ok(Some(uuid))
        } else {
            Ok(None)
        }
    }

    async fn save_conversation_id(&self, chat_jid: &str, conv_uuid: &str) -> anyhow::Result<()> {
        let conn = self.conn.lock().await;
        conn.execute(
            "INSERT INTO chats (chat_jid, conversation_uuid, updated_at)
             VALUES (?1, ?2, CURRENT_TIMESTAMP)
             ON CONFLICT(chat_jid) DO UPDATE SET
                conversation_uuid = excluded.conversation_uuid,
                updated_at = CURRENT_TIMESTAMP",
            params![chat_jid, conv_uuid],
        )?;
        Ok(())
    }

    async fn delete_conversation_id(&self, chat_jid: &str) -> anyhow::Result<()> {
        let conn = self.conn.lock().await;
        conn.execute(
            "DELETE FROM chats WHERE chat_jid = ?1",
            params![chat_jid],
        )?;
        Ok(())
    }

    async fn record_message(
        &self,
        chat_jid: &str,
        sender_jid: &str,
        text: &str,
        is_from_me: bool,
    ) -> anyhow::Result<()> {
        let conn = self.conn.lock().await;
        conn.execute(
            "INSERT INTO message_history (chat_jid, sender_jid, text, is_from_me)
             VALUES (?1, ?2, ?3, ?4)",
            params![chat_jid, sender_jid, text, is_from_me],
        )?;
        Ok(())
    }

    async fn get_user_profile(&self, sender_jid: &str) -> anyhow::Result<Option<crate::core::ports::UserProfile>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(
            "SELECT sender_jid, name, role, authority_level, notes FROM user_profiles WHERE sender_jid = ?1 LIMIT 1"
        )?;
        
        let mut rows = stmt.query(params![sender_jid])?;
        if let Some(row) = rows.next()? {
            Ok(Some(crate::core::ports::UserProfile {
                sender_jid: row.get(0)?,
                name: row.get(1)?,
                role: row.get(2)?,
                authority_level: row.get(3)?,
                notes: row.get(4)?,
            }))
        } else {
            Ok(None)
        }
    }

    async fn save_user_profile(&self, profile: &crate::core::ports::UserProfile) -> anyhow::Result<()> {
        let conn = self.conn.lock().await;
        conn.execute(
            "INSERT INTO user_profiles (sender_jid, name, role, authority_level, notes, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, CURRENT_TIMESTAMP)
             ON CONFLICT(sender_jid) DO UPDATE SET
                name = COALESCE(excluded.name, user_profiles.name),
                role = COALESCE(excluded.role, user_profiles.role),
                authority_level = excluded.authority_level,
                notes = COALESCE(excluded.notes, user_profiles.notes),
                updated_at = CURRENT_TIMESTAMP",
            params![
                profile.sender_jid,
                profile.name,
                profile.role,
                profile.authority_level,
                profile.notes
            ],
        )?;
        Ok(())
    }

    async fn create_scheduled_task(&self, task: &crate::core::domain::NewScheduledTask) -> anyhow::Result<i64> {
        let conn = self.conn.lock().await;
        let now_epoch = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        // Idempotency check: check if an active task already exists for this target around the same time (+/- 120s)
        // to prevent double execution if LLM retries or calls schedule add multiple times
        let mut check_stmt = conn.prepare(
            "SELECT id FROM scheduled_tasks 
             WHERE is_active = 1 
               AND target_jid = ?1 
               AND schedule_type = ?2 
               AND ABS(next_run_epoch - ?3) <= 120
             LIMIT 1",
        )?;
        let mut existing_id: Option<i64> = None;
        let mut rows = check_stmt.query(params![task.target_jid, task.schedule_type, task.next_run_epoch])?;
        if let Some(row) = rows.next()? {
            existing_id = Some(row.get(0)?);
        }
        drop(rows);
        drop(check_stmt);

        if let Some(eid) = existing_id {
            conn.execute(
                "UPDATE scheduled_tasks 
                 SET title = ?1, task_type = ?2, payload = ?3, schedule_expr = ?4, next_run_epoch = ?5 
                 WHERE id = ?6",
                params![
                    task.title,
                    task.task_type.as_str(),
                    task.payload,
                    task.schedule_expr,
                    task.next_run_epoch,
                    eid
                ],
            )?;
            return Ok(eid);
        }

        conn.execute(
            "INSERT INTO scheduled_tasks 
             (title, task_type, target_jid, payload, schedule_type, schedule_expr, next_run_epoch, is_active, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1, ?8)",
            params![
                task.title,
                task.task_type.as_str(),
                task.target_jid,
                task.payload,
                task.schedule_type,
                task.schedule_expr,
                task.next_run_epoch,
                now_epoch
            ],
        )?;

        let id = conn.last_insert_rowid();
        Ok(id)
    }

    async fn list_scheduled_tasks(&self, active_only: bool) -> anyhow::Result<Vec<crate::core::domain::ScheduledTask>> {
        let conn = self.conn.lock().await;
        let query = if active_only {
            "SELECT id, title, task_type, target_jid, payload, schedule_type, schedule_expr, next_run_epoch, last_run_epoch, is_active, created_at, last_status, last_error, last_duration_secs
             FROM scheduled_tasks WHERE is_active = 1 ORDER BY next_run_epoch ASC"
        } else {
            "SELECT id, title, task_type, target_jid, payload, schedule_type, schedule_expr, next_run_epoch, last_run_epoch, is_active, created_at, last_status, last_error, last_duration_secs
             FROM scheduled_tasks ORDER BY is_active DESC, next_run_epoch ASC"
        };

        let mut stmt = conn.prepare(query)?;
        let rows = stmt.query_map([], |row| {
            let task_type_str: String = row.get(2)?;
            let active_int: i32 = row.get(9)?;
            Ok(crate::core::domain::ScheduledTask {
                id: row.get(0)?,
                title: row.get(1)?,
                task_type: crate::core::domain::ScheduledTaskType::from_str(&task_type_str),
                target_jid: row.get(3)?,
                payload: row.get(4)?,
                schedule_type: row.get(5)?,
                schedule_expr: row.get(6)?,
                next_run_epoch: row.get(7)?,
                last_run_epoch: row.get(8)?,
                is_active: active_int != 0,
                created_at: row.get(10)?,
                last_status: row.get(11)?,
                last_error: row.get(12)?,
                last_duration_secs: row.get(13)?,
            })
        })?;

        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    async fn get_due_scheduled_tasks(&self, current_epoch: i64) -> anyhow::Result<Vec<crate::core::domain::ScheduledTask>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(
            "SELECT id, title, task_type, target_jid, payload, schedule_type, schedule_expr, next_run_epoch, last_run_epoch, is_active, created_at, last_status, last_error, last_duration_secs
             FROM scheduled_tasks 
             WHERE is_active = 1 AND next_run_epoch <= ?1
             ORDER BY next_run_epoch ASC"
        )?;

        let rows = stmt.query_map(params![current_epoch], |row| {
            let task_type_str: String = row.get(2)?;
            let active_int: i32 = row.get(9)?;
            Ok(crate::core::domain::ScheduledTask {
                id: row.get(0)?,
                title: row.get(1)?,
                task_type: crate::core::domain::ScheduledTaskType::from_str(&task_type_str),
                target_jid: row.get(3)?,
                payload: row.get(4)?,
                schedule_type: row.get(5)?,
                schedule_expr: row.get(6)?,
                next_run_epoch: row.get(7)?,
                last_run_epoch: row.get(8)?,
                is_active: active_int != 0,
                created_at: row.get(10)?,
                last_status: row.get(11)?,
                last_error: row.get(12)?,
                last_duration_secs: row.get(13)?,
            })
        })?;

        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    async fn update_scheduled_task_run(
        &self,
        id: i64,
        last_run: i64,
        next_run: Option<i64>,
        is_active: bool,
    ) -> anyhow::Result<()> {
        let conn = self.conn.lock().await;
        let next_epoch = next_run.unwrap_or(0);
        let active_int = if is_active { 1 } else { 0 };
        conn.execute(
            "UPDATE scheduled_tasks 
             SET last_run_epoch = ?1, next_run_epoch = ?2, is_active = ?3 
             WHERE id = ?4",
            params![last_run, next_epoch, active_int, id],
        )?;
        Ok(())
    }

    async fn delete_scheduled_task(&self, id: i64) -> anyhow::Result<bool> {
        let conn = self.conn.lock().await;
        let affected = conn.execute("DELETE FROM scheduled_tasks WHERE id = ?1", params![id])?;
        Ok(affected > 0)
    }

    async fn get_scheduled_task(&self, id: i64) -> anyhow::Result<Option<crate::core::domain::ScheduledTask>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(
            "SELECT id, title, task_type, target_jid, payload, schedule_type, schedule_expr, next_run_epoch, last_run_epoch, is_active, created_at, last_status, last_error, last_duration_secs
             FROM scheduled_tasks WHERE id = ?1 LIMIT 1"
        )?;

        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            let task_type_str: String = row.get(2)?;
            let active_int: i32 = row.get(9)?;
            Ok(Some(crate::core::domain::ScheduledTask {
                id: row.get(0)?,
                title: row.get(1)?,
                task_type: crate::core::domain::ScheduledTaskType::from_str(&task_type_str),
                target_jid: row.get(3)?,
                payload: row.get(4)?,
                schedule_type: row.get(5)?,
                schedule_expr: row.get(6)?,
                next_run_epoch: row.get(7)?,
                last_run_epoch: row.get(8)?,
                is_active: active_int != 0,
                created_at: row.get(10)?,
                last_status: row.get(11)?,
                last_error: row.get(12)?,
                last_duration_secs: row.get(13)?,
            }))
        } else {
            Ok(None)
        }
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
        let now_epoch = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        conn.execute(
            "INSERT INTO scheduled_task_runs 
             (task_id, task_title, target_jid, status, duration_secs, error_message, output_preview, executed_at_epoch)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                task_id,
                task_title,
                target_jid,
                status,
                duration_secs,
                error_message,
                output_preview,
                now_epoch
            ],
        )?;

        let id = conn.last_insert_rowid();
        Ok(id)
    }

    async fn list_scheduled_task_runs(&self, limit: usize) -> anyhow::Result<Vec<crate::core::domain::ScheduledTaskRun>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(
            "SELECT id, task_id, task_title, target_jid, status, duration_secs, error_message, output_preview, executed_at_epoch
             FROM scheduled_task_runs
             ORDER BY executed_at_epoch DESC, id DESC
             LIMIT ?1"
        )?;

        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok(crate::core::domain::ScheduledTaskRun {
                id: row.get(0)?,
                task_id: row.get(1)?,
                task_title: row.get(2)?,
                target_jid: row.get(3)?,
                status: row.get(4)?,
                duration_secs: row.get(5)?,
                error_message: row.get(6)?,
                output_preview: row.get(7)?,
                executed_at_epoch: row.get(8)?,
            })
        })?;

        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    async fn update_scheduled_task_result(
        &self,
        id: i64,
        status: &str,
        error_message: Option<&str>,
        duration_secs: f64,
    ) -> anyhow::Result<()> {
        let conn = self.conn.lock().await;
        conn.execute(
            "UPDATE scheduled_tasks 
             SET last_status = ?1, last_error = ?2, last_duration_secs = ?3 
             WHERE id = ?4",
            params![status, error_message, duration_secs, id],
        )?;
        Ok(())
    }

    async fn get_scheduler_diagnostics(&self) -> anyhow::Result<crate::core::domain::SchedulerDiagnostics> {
        let conn = self.conn.lock().await;

        let total_tasks: usize = conn.query_row(
            "SELECT COUNT(*) FROM scheduled_tasks",
            [],
            |r| r.get::<_, i64>(0).map(|v| v as usize),
        ).unwrap_or(0);

        let active_tasks: usize = conn.query_row(
            "SELECT COUNT(*) FROM scheduled_tasks WHERE is_active = 1",
            [],
            |r| r.get::<_, i64>(0).map(|v| v as usize),
        ).unwrap_or(0);

        let total_runs: usize = conn.query_row(
            "SELECT COUNT(*) FROM scheduled_task_runs",
            [],
            |r| r.get::<_, i64>(0).map(|v| v as usize),
        ).unwrap_or(0);

        let successful_runs: usize = conn.query_row(
            "SELECT COUNT(*) FROM scheduled_task_runs WHERE status = 'success'",
            [],
            |r| r.get::<_, i64>(0).map(|v| v as usize),
        ).unwrap_or(0);

        let failed_runs: usize = conn.query_row(
            "SELECT COUNT(*) FROM scheduled_task_runs WHERE status = 'failed'",
            [],
            |r| r.get::<_, i64>(0).map(|v| v as usize),
        ).unwrap_or(0);

        let last_run: Option<crate::core::domain::ScheduledTaskRun> = conn.query_row(
            "SELECT id, task_id, task_title, target_jid, status, duration_secs, error_message, output_preview, executed_at_epoch
             FROM scheduled_task_runs
             ORDER BY executed_at_epoch DESC, id DESC LIMIT 1",
            [],
            |row| Ok(crate::core::domain::ScheduledTaskRun {
                id: row.get(0)?,
                task_id: row.get(1)?,
                task_title: row.get(2)?,
                target_jid: row.get(3)?,
                status: row.get(4)?,
                duration_secs: row.get(5)?,
                error_message: row.get(6)?,
                output_preview: row.get(7)?,
                executed_at_epoch: row.get(8)?,
            })
        ).ok();

        let last_failure: Option<crate::core::domain::ScheduledTaskRun> = conn.query_row(
            "SELECT id, task_id, task_title, target_jid, status, duration_secs, error_message, output_preview, executed_at_epoch
             FROM scheduled_task_runs
             WHERE status = 'failed'
             ORDER BY executed_at_epoch DESC, id DESC LIMIT 1",
            [],
            |row| Ok(crate::core::domain::ScheduledTaskRun {
                id: row.get(0)?,
                task_id: row.get(1)?,
                task_title: row.get(2)?,
                target_jid: row.get(3)?,
                status: row.get(4)?,
                duration_secs: row.get(5)?,
                error_message: row.get(6)?,
                output_preview: row.get(7)?,
                executed_at_epoch: row.get(8)?,
            })
        ).ok();

        let next_task: Option<crate::core::domain::ScheduledTask> = conn.query_row(
            "SELECT id, title, task_type, target_jid, payload, schedule_type, schedule_expr, next_run_epoch, last_run_epoch, is_active, created_at, last_status, last_error, last_duration_secs
             FROM scheduled_tasks
             WHERE is_active = 1
             ORDER BY next_run_epoch ASC LIMIT 1",
            [],
            |row| {
                let task_type_str: String = row.get(2)?;
                let active_int: i32 = row.get(9)?;
                Ok(crate::core::domain::ScheduledTask {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    task_type: crate::core::domain::ScheduledTaskType::from_str(&task_type_str),
                    target_jid: row.get(3)?,
                    payload: row.get(4)?,
                    schedule_type: row.get(5)?,
                    schedule_expr: row.get(6)?,
                    next_run_epoch: row.get(7)?,
                    last_run_epoch: row.get(8)?,
                    is_active: active_int != 0,
                    created_at: row.get(10)?,
                    last_status: row.get(11)?,
                    last_error: row.get(12)?,
                    last_duration_secs: row.get(13)?,
                })
            }
        ).ok();

        Ok(crate::core::domain::SchedulerDiagnostics {
            total_tasks,
            active_tasks,
            total_runs,
            successful_runs,
            failed_runs,
            last_run,
            last_failure,
            next_task,
        })
    }

    async fn record_action_audit(&self, audit: &crate::core::domain::NewWhatsAppActionAudit) -> anyhow::Result<i64> {
        let conn = self.conn.lock().await;
        let tools_json = serde_json::to_string(&audit.tools_invoked).unwrap_or_else(|_| "[]".to_string());
        conn.execute(
            "INSERT INTO whatsapp_action_audits (
                message_id, chat_jid, chat_type, sender_jid, sender_name,
                decision, decision_reason, conversation_id, status, input_text,
                has_media, media_path, response_text, error_message, duration_seconds,
                tools_invoked, created_at_epoch, completed_at_epoch
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)",
            params![
                audit.message_id,
                audit.chat_jid,
                audit.chat_type,
                audit.sender_jid,
                audit.sender_name,
                audit.decision,
                audit.decision_reason,
                audit.conversation_id,
                audit.status,
                audit.input_text,
                if audit.has_media { 1 } else { 0 },
                audit.media_path,
                audit.response_text,
                audit.error_message,
                audit.duration_seconds,
                tools_json,
                audit.created_at_epoch,
                audit.completed_at_epoch,
            ],
        )?;
        Ok(conn.last_insert_rowid())
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
    ) -> anyhow::Result<()> {
        let conn = self.conn.lock().await;
        let completed_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let tools_json = serde_json::to_string(tools_invoked).unwrap_or_else(|_| "[]".to_string());

        conn.execute(
            "UPDATE whatsapp_action_audits SET
                conversation_id = COALESCE(?1, conversation_id),
                response_text = COALESCE(?2, response_text),
                error_message = ?3,
                status = ?4,
                duration_seconds = COALESCE(?5, duration_seconds),
                tools_invoked = ?6,
                completed_at_epoch = ?7
            WHERE id = ?8",
            params![
                conversation_id,
                response_text,
                error_message,
                status,
                duration_seconds,
                tools_json,
                completed_at,
                id,
            ],
        )?;
        Ok(())
    }

    async fn query_action_audits(
        &self,
        filter: &crate::core::domain::ActionAuditFilter,
    ) -> anyhow::Result<Vec<crate::core::domain::WhatsAppActionAudit>> {
        let conn = self.conn.lock().await;
        let mut sql = String::from(
            "SELECT id, message_id, chat_jid, chat_type, sender_jid, sender_name,
                    decision, decision_reason, conversation_id, status, input_text,
                    has_media, media_path, response_text, error_message, duration_seconds,
                    tools_invoked, created_at_epoch, completed_at_epoch
             FROM whatsapp_action_audits WHERE 1=1"
        );

        let mut conditions = Vec::new();
        let mut query_params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(ref chat) = filter.chat_jid {
            conditions.push("chat_jid = ?");
            query_params.push(Box::new(chat.clone()));
        }

        if let Some(ref sender) = filter.sender_jid {
            conditions.push("sender_jid = ?");
            query_params.push(Box::new(sender.clone()));
        }

        if let Some(ref dec) = filter.decision {
            conditions.push("decision = ?");
            query_params.push(Box::new(dec.clone()));
        }

        if let Some(ref st) = filter.status {
            conditions.push("status = ?");
            query_params.push(Box::new(st.clone()));
        }

        if let Some(since) = filter.since_epoch {
            conditions.push("created_at_epoch >= ?");
            query_params.push(Box::new(since));
        }

        if let Some(until) = filter.until_epoch {
            conditions.push("created_at_epoch <= ?");
            query_params.push(Box::new(until));
        }

        if let Some(ref q) = filter.query {
            let pattern = format!("%{}%", q);
            conditions.push("(input_text LIKE ? OR response_text LIKE ? OR decision_reason LIKE ?)");
            query_params.push(Box::new(pattern.clone()));
            query_params.push(Box::new(pattern.clone()));
            query_params.push(Box::new(pattern));
        }

        for cond in conditions {
            sql.push_str(" AND ");
            sql.push_str(cond);
        }

        sql.push_str(" ORDER BY created_at_epoch DESC, id DESC");

        let limit = filter.limit.unwrap_or(50).min(500);
        sql.push_str(&format!(" LIMIT {}", limit));

        if let Some(offset) = filter.offset {
            sql.push_str(&format!(" OFFSET {}", offset));
        }

        let mut stmt = conn.prepare(&sql)?;
        let param_refs: Vec<&dyn rusqlite::ToSql> = query_params.iter().map(|b| b.as_ref()).collect();
        let rows = stmt.query_map(param_refs.as_slice(), map_action_audit_row)?;

        let mut results = Vec::new();
        for r in rows {
            results.push(r?);
        }

        Ok(results)
    }

    async fn get_action_audit_by_id(&self, id: i64) -> anyhow::Result<Option<crate::core::domain::WhatsAppActionAudit>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(
            "SELECT id, message_id, chat_jid, chat_type, sender_jid, sender_name,
                    decision, decision_reason, conversation_id, status, input_text,
                    has_media, media_path, response_text, error_message, duration_seconds,
                    tools_invoked, created_at_epoch, completed_at_epoch
             FROM whatsapp_action_audits WHERE id = ?1 LIMIT 1"
        )?;
        let mut rows = stmt.query_map(params![id], map_action_audit_row)?;
        if let Some(row) = rows.next() {
            Ok(Some(row?))
        } else {
            Ok(None)
        }
    }

    async fn get_action_audit_by_message_id(&self, message_id: &str) -> anyhow::Result<Option<crate::core::domain::WhatsAppActionAudit>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(
            "SELECT id, message_id, chat_jid, chat_type, sender_jid, sender_name,
                    decision, decision_reason, conversation_id, status, input_text,
                    has_media, media_path, response_text, error_message, duration_seconds,
                    tools_invoked, created_at_epoch, completed_at_epoch
             FROM whatsapp_action_audits WHERE message_id = ?1 ORDER BY id DESC LIMIT 1"
        )?;
        let mut rows = stmt.query_map(params![message_id], map_action_audit_row)?;
        if let Some(row) = rows.next() {
            Ok(Some(row?))
        } else {
            Ok(None)
        }
    }

    async fn get_action_audit_summary(&self) -> anyhow::Result<crate::core::domain::AuditSummaryReport> {
        let conn = self.conn.lock().await;

        let total_actions: i64 = conn.query_row(
            "SELECT COUNT(*) FROM whatsapp_action_audits",
            [],
            |r| r.get(0),
        ).unwrap_or(0);

        let total_responses: i64 = conn.query_row(
            "SELECT COUNT(*) FROM whatsapp_action_audits WHERE decision = 'respond'",
            [],
            |r| r.get(0),
        ).unwrap_or(0);

        let total_recorded_only: i64 = conn.query_row(
            "SELECT COUNT(*) FROM whatsapp_action_audits WHERE decision = 'record_only'",
            [],
            |r| r.get(0),
        ).unwrap_or(0);

        let total_ignored: i64 = conn.query_row(
            "SELECT COUNT(*) FROM whatsapp_action_audits WHERE decision = 'ignore'",
            [],
            |r| r.get(0),
        ).unwrap_or(0);

        let total_errors: i64 = conn.query_row(
            "SELECT COUNT(*) FROM whatsapp_action_audits WHERE status = 'failed'",
            [],
            |r| r.get(0),
        ).unwrap_or(0);

        let avg_duration_seconds: f64 = conn.query_row(
            "SELECT COALESCE(AVG(duration_seconds), 0.0) FROM whatsapp_action_audits WHERE duration_seconds IS NOT NULL",
            [],
            |r| r.get(0),
        ).unwrap_or(0.0);

        // Most active chats
        let mut stmt_chats = conn.prepare(
            "SELECT chat_jid, COUNT(*) as cnt FROM whatsapp_action_audits GROUP BY chat_jid ORDER BY cnt DESC LIMIT 5"
        )?;
        let chat_rows = stmt_chats.query_map([], |r| {
            Ok(crate::core::domain::CountMetric {
                key: r.get(0)?,
                count: r.get(1)?,
            })
        })?;
        let most_active_chats = chat_rows.filter_map(|r| r.ok()).collect();

        // Most active senders
        let mut stmt_senders = conn.prepare(
            "SELECT sender_jid, COUNT(*) as cnt FROM whatsapp_action_audits GROUP BY sender_jid ORDER BY cnt DESC LIMIT 5"
        )?;
        let sender_rows = stmt_senders.query_map([], |r| {
            Ok(crate::core::domain::CountMetric {
                key: r.get(0)?,
                count: r.get(1)?,
            })
        })?;
        let most_active_senders = sender_rows.filter_map(|r| r.ok()).collect();

        // Decision breakdown
        let mut stmt_dec = conn.prepare(
            "SELECT decision, COUNT(*) as cnt FROM whatsapp_action_audits GROUP BY decision ORDER BY cnt DESC"
        )?;
        let dec_rows = stmt_dec.query_map([], |r| {
            Ok(crate::core::domain::CountMetric {
                key: r.get(0)?,
                count: r.get(1)?,
            })
        })?;
        let decision_breakdown = dec_rows.filter_map(|r| r.ok()).collect();

        // Status breakdown
        let mut stmt_st = conn.prepare(
            "SELECT status, COUNT(*) as cnt FROM whatsapp_action_audits GROUP BY status ORDER BY cnt DESC"
        )?;
        let st_rows = stmt_st.query_map([], |r| {
            Ok(crate::core::domain::CountMetric {
                key: r.get(0)?,
                count: r.get(1)?,
            })
        })?;
        let status_breakdown = st_rows.filter_map(|r| r.ok()).collect();

        // Top tools used (parse recent tools_invoked)
        let mut stmt_tools = conn.prepare(
            "SELECT tools_invoked FROM whatsapp_action_audits WHERE tools_invoked != '[]' ORDER BY id DESC LIMIT 500"
        )?;
        let tool_rows = stmt_tools.query_map([], |r| r.get::<_, String>(0))?;
        let mut tool_counts: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
        for t_res in tool_rows.flatten() {
            if let Ok(arr) = serde_json::from_str::<Vec<String>>(&t_res) {
                for tool_name in arr {
                    *tool_counts.entry(tool_name).or_insert(0) += 1;
                }
            }
        }
        let mut top_tools: Vec<crate::core::domain::CountMetric> = tool_counts
            .into_iter()
            .map(|(key, count)| crate::core::domain::CountMetric { key, count })
            .collect();
        top_tools.sort_by(|a, b| b.count.cmp(&a.count));
        top_tools.truncate(10);

        Ok(crate::core::domain::AuditSummaryReport {
            total_actions,
            total_responses,
            total_recorded_only,
            total_ignored,
            total_errors,
            avg_duration_seconds,
            most_active_chats,
            most_active_senders,
            decision_breakdown,
            status_breakdown,
            top_tools_used: top_tools,
        })
    }

    async fn record_metacognitive_prediction(
        &self,
        pred: &crate::core::domain::NewMetacognitivePrediction,
    ) -> anyhow::Result<i64> {
        let conn = self.conn.lock().await;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let risks_json = serde_json::to_string(&pred.identified_risks).unwrap_or_else(|_| "[]".to_string());

        conn.execute(
            "INSERT INTO metacognitive_predictions (
                prediction_id, action_audit_id, task_description, domain_type,
                predicted_probability, complexity_tier, identified_risks,
                fallback_strategy, created_at_epoch
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                pred.prediction_id,
                pred.action_audit_id,
                pred.task_description,
                pred.domain_type.as_str(),
                pred.predicted_probability,
                pred.complexity_tier.as_str(),
                risks_json,
                pred.fallback_strategy,
                now,
            ],
        )?;

        Ok(conn.last_insert_rowid())
    }

    async fn resolve_metacognitive_prediction(
        &self,
        prediction_id: &str,
        actual_outcome: f64,
        duration_secs: f64,
        error_detail: Option<&str>,
    ) -> anyhow::Result<()> {
        let conn = self.conn.lock().await;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        let mut stmt = conn.prepare("SELECT predicted_probability FROM metacognitive_predictions WHERE prediction_id = ?1")?;
        let predicted_prob: Option<f64> = stmt.query_row(params![prediction_id], |r| r.get(0)).ok();

        if let Some(p) = predicted_prob {
            let brier = crate::core::domain::calculate_brier_score(p, actual_outcome);
            conn.execute(
                "UPDATE metacognitive_predictions SET
                    actual_outcome = ?1,
                    brier_score = ?2,
                    execution_duration_secs = ?3,
                    error_detail = ?4,
                    resolved_at_epoch = ?5
                WHERE prediction_id = ?6",
                params![actual_outcome, brier, duration_secs, error_detail, now, prediction_id],
            )?;
        }

        Ok(())
    }

    async fn list_metacognitive_predictions(
        &self,
        limit: usize,
        domain_filter: Option<&str>,
    ) -> anyhow::Result<Vec<crate::core::domain::MetacognitivePrediction>> {
        let conn = self.conn.lock().await;
        let query = if let Some(dom) = domain_filter {
            format!(
                "SELECT id, prediction_id, action_audit_id, task_description, domain_type,
                        predicted_probability, complexity_tier, identified_risks, fallback_strategy,
                        actual_outcome, brier_score, execution_duration_secs, error_detail,
                        created_at_epoch, resolved_at_epoch
                 FROM metacognitive_predictions
                 WHERE domain_type = '{}'
                 ORDER BY created_at_epoch DESC LIMIT {}",
                dom.replace('\'', "''"),
                limit
            )
        } else {
            format!(
                "SELECT id, prediction_id, action_audit_id, task_description, domain_type,
                        predicted_probability, complexity_tier, identified_risks, fallback_strategy,
                        actual_outcome, brier_score, execution_duration_secs, error_detail,
                        created_at_epoch, resolved_at_epoch
                 FROM metacognitive_predictions
                 ORDER BY created_at_epoch DESC LIMIT {}",
                limit
            )
        };

        let mut stmt = conn.prepare(&query)?;
        let rows = stmt.query_map([], |row| {
            let risks_json: String = row.get(7)?;
            let risks: Vec<String> = serde_json::from_str(&risks_json).unwrap_or_default();
            let domain_str: String = row.get(4)?;
            let complexity_str: String = row.get(6)?;

            Ok(crate::core::domain::MetacognitivePrediction {
                id: row.get(0)?,
                prediction_id: row.get(1)?,
                action_audit_id: row.get(2)?,
                task_description: row.get(3)?,
                domain_type: crate::core::domain::TaskDomainType::from_str(&domain_str),
                predicted_probability: row.get(5)?,
                complexity_tier: crate::core::domain::ComplexityTier::from_str(&complexity_str),
                identified_risks: risks,
                fallback_strategy: row.get(8)?,
                actual_outcome: row.get(9)?,
                brier_score: row.get(10)?,
                execution_duration_secs: row.get(11)?,
                error_detail: row.get(12)?,
                created_at_epoch: row.get(13)?,
                resolved_at_epoch: row.get(14)?,
            })
        })?;

        let mut results = Vec::new();
        for r in rows {
            results.push(r?);
        }
        Ok(results)
    }

    async fn get_metacognitive_calibration_stats(&self) -> anyhow::Result<crate::core::domain::MetacognitiveCalibrationStats> {
        let predictions = self.list_metacognitive_predictions(500, None).await?;
        Ok(crate::core::domain::calculate_calibration_stats(&predictions))
    }
}

fn map_action_audit_row(row: &rusqlite::Row) -> rusqlite::Result<crate::core::domain::WhatsAppActionAudit> {
    let has_media_int: i32 = row.get(11)?;
    let tools_json: String = row.get(16)?;
    let tools_invoked: Vec<String> = serde_json::from_str(&tools_json).unwrap_or_default();

    Ok(crate::core::domain::WhatsAppActionAudit {
        id: row.get(0)?,
        message_id: row.get(1)?,
        chat_jid: row.get(2)?,
        chat_type: row.get(3)?,
        sender_jid: row.get(4)?,
        sender_name: row.get(5)?,
        decision: row.get(6)?,
        decision_reason: row.get(7)?,
        conversation_id: row.get(8)?,
        status: row.get(9)?,
        input_text: row.get(10)?,
        has_media: has_media_int != 0,
        media_path: row.get(12)?,
        response_text: row.get(13)?,
        error_message: row.get(14)?,
        duration_seconds: row.get(15)?,
        tools_invoked,
        created_at_epoch: row.get(17)?,
        completed_at_epoch: row.get(18)?,
    })
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
            created_at_epoch: 1774000000,
            completed_at_epoch: None,
        };

        let audit_id = store.record_action_audit(&new_audit).await.unwrap();
        assert!(audit_id > 0);

        // Fetch by id
        let fetched = store.get_action_audit_by_id(audit_id).await.unwrap().unwrap();
        assert_eq!(fetched.message_id, "MSG_AUDIT_101");
        assert_eq!(fetched.status, "in_progress");

        // Fetch by message_id
        let fetched_by_msg = store.get_action_audit_by_message_id("MSG_AUDIT_101").await.unwrap().unwrap();
        assert_eq!(fetched_by_msg.id, audit_id);

        // Update result
        store.update_action_audit_result(
            audit_id,
            Some("conv-uuid-12345"),
            Some("Ini hasil ekstraksi data Anda"),
            None,
            "success",
            Some(2.45),
            &["run_command".to_string(), "view_file".to_string()],
        ).await.unwrap();

        let updated = store.get_action_audit_by_id(audit_id).await.unwrap().unwrap();
        assert_eq!(updated.status, "success");
        assert_eq!(updated.conversation_id.as_deref(), Some("conv-uuid-12345"));
        assert_eq!(updated.duration_seconds, Some(2.45));
        assert_eq!(updated.tools_invoked, vec!["run_command".to_string(), "view_file".to_string()]);

        // Query filter by status
        let filter = crate::core::domain::ActionAuditFilter {
            status: Some("success".to_string()),
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

        // Clean up
        let _ = std::fs::remove_dir_all(&dir);
    }
}
