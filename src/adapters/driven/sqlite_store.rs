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
}
