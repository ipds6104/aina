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
                created_at INTEGER NOT NULL
            )",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_scheduled_tasks_next_run 
             ON scheduled_tasks(is_active, next_run_epoch)",
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
            "SELECT id, title, task_type, target_jid, payload, schedule_type, schedule_expr, next_run_epoch, last_run_epoch, is_active, created_at
             FROM scheduled_tasks WHERE is_active = 1 ORDER BY next_run_epoch ASC"
        } else {
            "SELECT id, title, task_type, target_jid, payload, schedule_type, schedule_expr, next_run_epoch, last_run_epoch, is_active, created_at
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
            "SELECT id, title, task_type, target_jid, payload, schedule_type, schedule_expr, next_run_epoch, last_run_epoch, is_active, created_at
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
            "SELECT id, title, task_type, target_jid, payload, schedule_type, schedule_expr, next_run_epoch, last_run_epoch, is_active, created_at
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
            }))
        } else {
            Ok(None)
        }
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

        let active_tasks = store.list_scheduled_tasks(true).await.unwrap();
        assert_eq!(active_tasks.len(), 1);
        assert_eq!(active_tasks[0].title, "Pengingat YouTube");

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
}
