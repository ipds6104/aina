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
}
