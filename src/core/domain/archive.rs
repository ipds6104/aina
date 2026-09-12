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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ArchiveSearchFilter {
    pub query: String,
    pub limit: usize,
    pub since: Option<String>,
    pub from_date: Option<String>,
    pub to_date: Option<String>,
}

pub fn parse_since_to_timestamp(since_str: &str) -> Option<String> {
    let s = since_str.trim().to_lowercase();
    let (val_str, unit) = if let Some(stripped) = s.strip_suffix('d') {
        (stripped, 'd')
    } else if let Some(stripped) = s.strip_suffix('h') {
        (stripped, 'h')
    } else if let Some(stripped) = s.strip_suffix('m') {
        (stripped, 'm')
    } else if let Some(stripped) = s.strip_suffix('s') {
        (stripped, 's')
    } else if s.chars().all(|c| c.is_ascii_digit()) {
        (s.as_str(), 'd')
    } else {
        return None;
    };

    let val: u64 = val_str.parse().ok()?;
    let secs = match unit {
        's' => val,
        'm' => val * 60,
        'h' => val * 3600,
        'd' => val * 86400,
        _ => return None,
    };

    let now_epoch = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let cutoff_epoch = now_epoch.saturating_sub(secs) as i64;
    let days = cutoff_epoch.div_euclid(86400);
    let rem = cutoff_epoch.rem_euclid(86400);
    let hh = rem / 3600;
    let mm = (rem % 3600) / 60;
    let ss = rem % 60;

    let z = days + 719468;
    let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let final_y = if m <= 2 { y + 1 } else { y };

    Some(format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}", final_y, m, d, hh, mm, ss))
}

pub fn normalize_datetime(dt_str: &str, is_end_of_day: bool) -> String {
    let trimmed = dt_str.trim();
    if trimmed.len() == 10 && trimmed.chars().nth(4) == Some('-') && trimmed.chars().nth(7) == Some('-') {
        if is_end_of_day {
            format!("{} 23:59:59", trimmed)
        } else {
            format!("{} 00:00:00", trimmed)
        }
    } else {
        trimmed.to_string()
    }
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

        // 4. Check DATABASE_PATH env if specified explicitly
        if let Ok(db_env) = std::env::var("DATABASE_PATH") {
            let p = PathBuf::from(db_env);
            if p.is_file() {
                dbs.push(p);
            }
        }

        dbs.sort();
        dbs.dedup();
        dbs
    }

    /// Search a single archive database using full filter (FTS5 BM25, temporal range, or LIKE fallback)
    pub fn search_single_archive_with_filter<P: AsRef<Path>>(
        db_path: P,
        filter: &ArchiveSearchFilter,
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

        // Determine table name: "messages" (archive) or "message_history" (live database)
        let (table_name, text_col, sender_col, time_col) = {
            let has_messages: bool = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='messages'",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .map(|c| c > 0)
                .unwrap_or(false);

            let has_message_history: bool = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='message_history'",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .map(|c| c > 0)
                .unwrap_or(false);

            if has_messages {
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
                ("messages", col, "sender", "timestamp")
            } else if has_message_history {
                ("message_history", "text", "sender_jid", "created_at")
            } else {
                return Ok(Vec::new());
            }
        };

        let cutoff_from: Option<String> = filter
            .from_date
            .as_deref()
            .map(|d| normalize_datetime(d, false))
            .or_else(|| filter.since.as_deref().and_then(parse_since_to_timestamp));

        let cutoff_to: Option<String> = filter
            .to_date
            .as_deref()
            .map(|d| normalize_datetime(d, true));

        let query_trimmed = filter.query.trim();
        let mut results = Vec::new();

        if !query_trimmed.is_empty() {
            // Check if messages_fts exists
            let has_fts: bool = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='messages_fts'",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .map(|c| c > 0)
                .unwrap_or(false);

            if has_fts {
                let sanitized_query = query_trimmed.replace('"', "\"\"");
                let match_expr = format!("\"{}\"", sanitized_query);

                // Try JOIN query with external content table
                let sql_join = format!(
                    "SELECT m.id, m.{}, m.{}, m.{}, bm25(messages_fts) as rank
                     FROM messages_fts
                     JOIN {} m ON messages_fts.rowid = m.id
                     WHERE messages_fts MATCH ?1
                       AND (?2 IS NULL OR m.{} >= ?2)
                       AND (?3 IS NULL OR m.{} <= ?3)
                     ORDER BY rank ASC
                     LIMIT ?4",
                    time_col, sender_col, text_col, table_name, time_col, time_col
                );

                if let Ok(mut stmt) = conn.prepare(&sql_join) {
                    if let Ok(rows) = stmt.query_map(
                        params![match_expr, cutoff_from, cutoff_to, filter.limit as i64],
                        |row| {
                            Ok(ArchiveMessage {
                                id: row.get(0)?,
                                timestamp: row.get(1)?,
                                sender: row.get(2)?,
                                message: row.get(3)?,
                                rank: row.get(4)?,
                                archive_name: archive_name.clone(),
                            })
                        },
                    ) {
                        for msg in rows.flatten() {
                            results.push(msg);
                        }
                    }
                }
            }

            // Fallback to LIKE search on detected table
            if results.is_empty() {
                let like_expr = format!("%{}%", query_trimmed);
                let sql = format!(
                    "SELECT id, {}, {}, {}, 0.0 as rank
                     FROM {}
                     WHERE {} LIKE ?1
                       AND (?2 IS NULL OR {} >= ?2)
                       AND (?3 IS NULL OR {} <= ?3)
                     ORDER BY {} DESC
                     LIMIT ?4",
                    time_col, sender_col, text_col, table_name, text_col, time_col, time_col, time_col
                );

                if let Ok(mut stmt) = conn.prepare(&sql) {
                    if let Ok(rows) = stmt.query_map(
                        params![like_expr, cutoff_from, cutoff_to, filter.limit as i64],
                        |row| {
                            Ok(ArchiveMessage {
                                id: row.get(0)?,
                                timestamp: row.get(1)?,
                                sender: row.get(2)?,
                                message: row.get(3)?,
                                rank: row.get(4)?,
                                archive_name: archive_name.clone(),
                            })
                        },
                    ) {
                        for msg in rows.flatten() {
                            results.push(msg);
                        }
                    }
                }
            }
        } else {
            // No keyword query: retrieve recent messages within temporal bounds
            let sql = format!(
                "SELECT id, {}, {}, {}, 0.0 as rank
                 FROM {}
                 WHERE (?1 IS NULL OR {} >= ?1)
                   AND (?2 IS NULL OR {} <= ?2)
                 ORDER BY {} DESC
                 LIMIT ?3",
                time_col, sender_col, text_col, table_name, time_col, time_col, time_col
            );

            if let Ok(mut stmt) = conn.prepare(&sql) {
                if let Ok(rows) = stmt.query_map(
                    params![cutoff_from, cutoff_to, filter.limit as i64],
                    |row| {
                        Ok(ArchiveMessage {
                            id: row.get(0)?,
                            timestamp: row.get(1)?,
                            sender: row.get(2)?,
                            message: row.get(3)?,
                            rank: row.get(4)?,
                            archive_name: archive_name.clone(),
                        })
                    },
                ) {
                    for msg in rows.flatten() {
                        results.push(msg);
                    }
                }
            }
        }

        debug!("Archive search in {:?} returned {} hits", path, results.len());
        Ok(results)
    }

    /// Search a single archive database using FTS5 (BM25 ranking) with fallback to LIKE (convenience)
    #[allow(dead_code)]
    pub fn search_single_archive<P: AsRef<Path>>(
        db_path: P,
        query: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<ArchiveMessage>> {
        Self::search_single_archive_with_filter(
            db_path,
            &ArchiveSearchFilter {
                query: query.to_string(),
                limit,
                ..Default::default()
            },
        )
    }

    /// Search across all archives in the workspace with temporal filters
    pub fn search_all_with_filter<P: AsRef<Path>>(
        workspace_dir: P,
        filter: &ArchiveSearchFilter,
    ) -> anyhow::Result<Vec<ArchiveMessage>> {
        let archives = Self::discover_archives(workspace_dir);
        let mut all_results = Vec::new();

        for db in archives {
            if let Ok(mut msgs) = Self::search_single_archive_with_filter(&db, filter) {
                all_results.append(&mut msgs);
            }
        }

        // Sort by rank ascending, then timestamp descending
        all_results.sort_by(|a, b| {
            match a.rank.partial_cmp(&b.rank).unwrap_or(std::cmp::Ordering::Equal) {
                std::cmp::Ordering::Equal => b.timestamp.cmp(&a.timestamp),
                other => other,
            }
        });

        if all_results.len() > filter.limit {
            all_results.truncate(filter.limit);
        }

        Ok(all_results)
    }

    /// Search across all archives in the workspace (convenience)
    #[allow(dead_code)]
    pub fn search_all<P: AsRef<Path>>(
        workspace_dir: P,
        query: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<ArchiveMessage>> {
        Self::search_all_with_filter(
            workspace_dir,
            &ArchiveSearchFilter {
                query: query.to_string(),
                limit,
                ..Default::default()
            },
        )
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

        let has_messages: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='messages'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map(|c| c > 0)
            .unwrap_or(false);

        let (table_name, sender_col, time_col) = if has_messages {
            ("messages", "sender", "timestamp")
        } else {
            ("message_history", "sender_jid", "created_at")
        };

        let total_messages: i64 = conn
            .query_row(&format!("SELECT COUNT(*) FROM {}", table_name), [], |row| row.get(0))
            .unwrap_or(0);

        let total_participants: i64 = conn
            .query_row(
                &format!("SELECT COUNT(DISTINCT {}) FROM {} WHERE {} != 'System'", sender_col, table_name, sender_col),
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let earliest_date: Option<String> = conn
            .query_row(
                &format!("SELECT MIN({}) FROM {} WHERE {} IS NOT NULL AND {} != ''", time_col, table_name, time_col, time_col),
                [],
                |row| row.get(0),
            )
            .ok();

        let latest_date: Option<String> = conn
            .query_row(
                &format!("SELECT MAX({}) FROM {} WHERE {} IS NOT NULL AND {} != ''", time_col, table_name, time_col, time_col),
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

        let db_path = temp_dir.join("knowledge").join("archives").join("chat_test.db");
        let stats = ArchiveEngine::get_stats(&db_path).unwrap();
        assert_eq!(stats.total_messages, 3);
        assert_eq!(stats.total_participants, 2);

        let all_stats = ArchiveEngine::get_all_stats(&temp_dir).unwrap();
        assert_eq!(all_stats.len(), 1);

        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_search_live_message_history() {
        let id = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let temp_dir = std::env::temp_dir().join(format!("aina_test_live_db_{}", id));
        let data_dir = temp_dir.join("data");
        std::fs::create_dir_all(&data_dir).unwrap();
        let db_path = data_dir.join("aina.db");

        let conn = Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE message_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                chat_jid TEXT NOT NULL,
                sender_jid TEXT NOT NULL,
                text TEXT NOT NULL,
                is_from_me BOOLEAN NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );
            INSERT INTO message_history (id, chat_jid, sender_jid, text, is_from_me, created_at) VALUES
                (1, 'group1@g.us', '628123456@s.whatsapp.net', 'Apakah server redis sudah di-deploy?', 0, '2026-09-10 08:00:00'),
                (2, 'group1@g.us', 'bot@s.whatsapp.net', 'Sudah aktif di port 6379 mas.', 1, '2026-09-10 08:01:00');",
        )
        .unwrap();

        let results = ArchiveEngine::search_single_archive(&db_path, "redis", 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].sender, "628123456@s.whatsapp.net");
        assert!(results[0].message.contains("redis"));

        let stats = ArchiveEngine::get_stats(&db_path).unwrap();
        assert_eq!(stats.total_messages, 2);
        assert_eq!(stats.total_participants, 2);

        let _ = std::fs::remove_dir_all(temp_dir);
    }
}
