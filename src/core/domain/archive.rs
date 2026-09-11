use rusqlite::{params, Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tracing::debug;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveMessage {
    pub id: i64,
    pub timestamp: String,
    pub sender: String,
    pub message: String,
    pub rank: f64,
    pub archive_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveStats {
    pub archive_name: String,
    pub db_path: String,
    pub total_messages: i64,
    pub total_participants: i64,
    pub earliest_date: Option<String>,
    pub latest_date: Option<String>,
    pub file_size_bytes: u64,
}

pub struct ArchiveEngine;

impl ArchiveEngine {
    /// Discover all `.db` archive files inside `data/chats/`, `knowledge/archives/`, or `data/`
    pub fn discover_archives<P: AsRef<Path>>(workspace_dir: P) -> Vec<PathBuf> {
        let ws = workspace_dir.as_ref();
        let mut dbs = Vec::new();

        // 1. Check data/chats/*/messages.db
        let chats_dir = ws.join("data").join("chats");
        if chats_dir.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&chats_dir) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.is_dir() {
                        let msg_db = p.join("messages.db");
                        if msg_db.is_file() {
                            dbs.push(msg_db);
                        }
                    }
                }
            }
        }

        // 2. Check knowledge/archives/*.db
        let archives_dir = ws.join("knowledge").join("archives");
        if archives_dir.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&archives_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("db") {
                        dbs.push(path);
                    }
                }
            }
        }

        // 3. Check data/*.db
        let data_dir = ws.join("data");
        if data_dir.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&data_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("db") {
                        dbs.push(path);
                    }
                }
            }
        }

        dbs.sort();
        dbs.dedup();
        dbs
    }

    /// Search a single archive database using FTS5 (BM25 ranking) with fallback to LIKE
    pub fn search_single_archive<P: AsRef<Path>>(
        db_path: P,
        query: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<ArchiveMessage>> {
        let path = db_path.as_ref();
        let archive_name = if path.file_name().and_then(|s| s.to_str()) == Some("messages.db") {
            path.parent()
                .and_then(|p| p.file_name())
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string()
        } else {
            path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string()
        };

        let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        conn.execute_batch(
            "PRAGMA query_only = ON;
             PRAGMA busy_timeout = 2000;",
        )?;

        // Check if messages_fts exists
        let has_fts: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='messages_fts'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map(|c| c > 0)
            .unwrap_or(false);

        // Check text column name in `messages`: either "text" or "message"
        let text_col = {
            let mut col = "message";
            if let Ok(mut stmt) = conn.prepare("PRAGMA table_info(messages)") {
                if let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(1)) {
                    for name in rows.flatten() {
                        if name.eq_ignore_ascii_case("text") {
                            col = "text";
                            break;
                        }
                    }
                }
            }
            col
        };

        let mut results = Vec::new();

        if has_fts {
            let sanitized_query = query.replace('"', "\"\"");
            let match_expr = format!("\"{}\"", sanitized_query);

            // 1. Try JOIN query with messages table (canonical for FTS5 external content table)
            let sql_join = format!(
                "SELECT m.id, m.timestamp, m.sender, m.{}, bm25(messages_fts) as rank
                 FROM messages_fts
                 JOIN messages m ON messages_fts.rowid = m.id
                 WHERE messages_fts MATCH ?1
                 ORDER BY rank ASC
                 LIMIT ?2",
                text_col
            );

            if let Ok(mut stmt) = conn.prepare(&sql_join) {
                if let Ok(rows) = stmt.query_map(params![match_expr, limit as i64], |row| {
                    Ok(ArchiveMessage {
                        id: row.get(0)?,
                        timestamp: row.get(1)?,
                        sender: row.get(2)?,
                        message: row.get(3)?,
                        rank: row.get(4)?,
                        archive_name: archive_name.clone(),
                    })
                }) {
                    for msg in rows.flatten() {
                        results.push(msg);
                    }
                }
            }

            // 2. If results still empty, try direct SELECT from messages_fts (standalone virtual table)
            if results.is_empty() {
                let sql_direct = format!(
                    "SELECT rowid, timestamp, sender, {}, bm25(messages_fts) as rank
                     FROM messages_fts
                     WHERE messages_fts MATCH ?1
                     ORDER BY rank ASC
                     LIMIT ?2",
                    text_col
                );
                if let Ok(mut stmt) = conn.prepare(&sql_direct) {
                    if let Ok(rows) = stmt.query_map(params![match_expr, limit as i64], |row| {
                        Ok(ArchiveMessage {
                            id: row.get(0)?,
                            timestamp: row.get(1)?,
                            sender: row.get(2)?,
                            message: row.get(3)?,
                            rank: row.get(4)?,
                            archive_name: archive_name.clone(),
                        })
                    }) {
                        for msg in rows.flatten() {
                            results.push(msg);
                        }
                    }
                }
            }
        }

        // If FTS5 gave no results or failed, fallback to LIKE search on `messages` table
        if results.is_empty() {
            let like_expr = format!("%{}%", query);
            let sql = format!(
                "SELECT id, timestamp, sender, {}, 0.0 as rank
                 FROM messages
                 WHERE {} LIKE ?1
                 ORDER BY timestamp DESC
                 LIMIT ?2",
                text_col, text_col
            );

            if let Ok(mut stmt) = conn.prepare(&sql) {
                if let Ok(rows) = stmt.query_map(params![like_expr, limit as i64], |row| {
                    Ok(ArchiveMessage {
                        id: row.get(0)?,
                        timestamp: row.get(1)?,
                        sender: row.get(2)?,
                        message: row.get(3)?,
                        rank: row.get(4)?,
                        archive_name: archive_name.clone(),
                    })
                }) {
                    for msg in rows.flatten() {
                        results.push(msg);
                    }
                }
            }
        }

        debug!("Archive search in {:?} returned {} hits", path, results.len());
        Ok(results)
    }

    /// Search across all archives in the workspace
    pub fn search_all<P: AsRef<Path>>(
        workspace_dir: P,
        query: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<ArchiveMessage>> {
        let archives = Self::discover_archives(workspace_dir);
        let mut all_results = Vec::new();

        for db in archives {
            if let Ok(mut msgs) = Self::search_single_archive(&db, query, limit) {
                all_results.append(&mut msgs);
            }
        }

        // Sort by rank ascending (lower BM25 is better match in SQLite FTS5)
        all_results.sort_by(|a, b| {
            a.rank
                .partial_cmp(&b.rank)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        if all_results.len() > limit {
            all_results.truncate(limit);
        }

        Ok(all_results)
    }

    /// Retrieve statistics for an archive database
    pub fn get_stats<P: AsRef<Path>>(db_path: P) -> anyhow::Result<ArchiveStats> {
        let path = db_path.as_ref();
        let archive_name = if path.file_name().and_then(|s| s.to_str()) == Some("messages.db") {
            path.parent()
                .and_then(|p| p.file_name())
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string()
        } else {
            path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string()
        };

        let file_size_bytes = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;

        let total_messages: i64 = conn
            .query_row("SELECT COUNT(*) FROM messages", [], |row| row.get(0))
            .unwrap_or(0);

        let total_participants: i64 = conn
            .query_row(
                "SELECT COUNT(DISTINCT sender) FROM messages WHERE sender != 'System'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let earliest_date: Option<String> = conn
            .query_row(
                "SELECT MIN(timestamp) FROM messages WHERE timestamp IS NOT NULL AND timestamp != ''",
                [],
                |row| row.get(0),
            )
            .ok();

        let latest_date: Option<String> = conn
            .query_row(
                "SELECT MAX(timestamp) FROM messages WHERE timestamp IS NOT NULL AND timestamp != ''",
                [],
                |row| row.get(0),
            )
            .ok();

        Ok(ArchiveStats {
            archive_name,
            db_path: path.to_string_lossy().to_string(),
            total_messages,
            total_participants,
            earliest_date,
            latest_date,
            file_size_bytes,
        })
    }

    /// Retrieve statistics for all archives in workspace
    pub fn get_all_stats<P: AsRef<Path>>(workspace_dir: P) -> anyhow::Result<Vec<ArchiveStats>> {
        let archives = Self::discover_archives(workspace_dir);
        let mut stats = Vec::new();
        for db in archives {
            if let Ok(st) = Self::get_stats(db) {
                stats.push(st);
            }
        }
        Ok(stats)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static TEST_COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn create_test_archive() -> PathBuf {
        let id = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let temp_dir = std::env::temp_dir().join(format!("aina_test_archive_{}", id));
        let archives_dir = temp_dir.join("knowledge").join("archives");
        std::fs::create_dir_all(&archives_dir).unwrap();
        let db_path = archives_dir.join("chat_test.db");

        let conn = Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp TEXT,
                sender TEXT,
                message TEXT
            );
            CREATE VIRTUAL TABLE messages_fts USING fts5(
                timestamp UNINDEXED,
                sender,
                message,
                content='messages',
                content_rowid='id'
            );
            INSERT INTO messages (id, timestamp, sender, message) VALUES
                (1, '2026-09-01 10:00:00', 'Budi', 'Halo tim, bagaimana persiapan akreditasi prodi?'),
                (2, '2026-09-01 10:05:00', 'Siti', 'Dokumen akreditasi sudah diunggah ke drive.'),
                (3, '2026-09-02 14:00:00', 'Budi', 'Rapat evaluasi kurikulum besok pagi jam 09:00.');
            INSERT INTO messages_fts (rowid, timestamp, sender, message)
                SELECT id, timestamp, sender, message FROM messages;",
        )
        .unwrap();

        temp_dir
    }

    #[test]
    fn test_search_single_archive_fts5() {
        let temp_dir = create_test_archive();
        let db_path = temp_dir.join("knowledge").join("archives").join("chat_test.db");
        let results = ArchiveEngine::search_single_archive(&db_path, "akreditasi", 10).unwrap();
        assert_eq!(results.len(), 2);
        assert!(results[0].message.contains("akreditasi"));
        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_search_all_and_stats() {
        let temp_dir = create_test_archive();
        let results = ArchiveEngine::search_all(&temp_dir, "kurikulum", 5).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].sender, "Budi");

        let stats = ArchiveEngine::get_all_stats(&temp_dir).unwrap();
        assert_eq!(stats.len(), 1);
        assert_eq!(stats[0].total_messages, 3);
        assert_eq!(stats[0].total_participants, 2);
        let _ = std::fs::remove_dir_all(temp_dir);
    }
}
