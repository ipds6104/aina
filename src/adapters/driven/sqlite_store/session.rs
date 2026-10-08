use crate::core::ports::UserProfile;
use rusqlite::{params, Connection};

pub fn get_conversation_id(conn: &Connection, chat_jid: &str) -> anyhow::Result<Option<String>> {
    let mut stmt = conn.prepare("SELECT conversation_uuid FROM chats WHERE chat_jid = ?1 LIMIT 1")?;
    let mut rows = stmt.query(params![chat_jid])?;
    if let Some(row) = rows.next()? {
        let uuid: String = row.get(0)?;
        Ok(Some(uuid))
    } else {
        Ok(None)
    }
}

pub fn get_active_conversation_id(
    conn: &Connection,
    chat_jid: &str,
    max_inactivity_secs: u64,
) -> anyhow::Result<Option<String>> {
    let mut stmt = conn.prepare(
        "SELECT conversation_uuid,
                CAST((strftime('%s', CURRENT_TIMESTAMP) - strftime('%s', updated_at)) AS INTEGER)
         FROM chats WHERE chat_jid = ?1 LIMIT 1"
    )?;
    let mut rows = stmt.query(params![chat_jid])?;
    if let Some(row) = rows.next()? {
        let uuid: String = row.get(0)?;
        let elapsed: Option<i64> = row.get(1)?;
        if let Some(secs) = elapsed {
            if secs >= 0 && (secs as u64) <= max_inactivity_secs {
                return Ok(Some(uuid));
            } else {
                return Ok(None);
            }
        }
        Ok(Some(uuid))
    } else {
        Ok(None)
    }
}

pub fn touch_conversation_activity(conn: &Connection, chat_jid: &str) -> anyhow::Result<()> {
    conn.execute(
        "UPDATE chats SET updated_at = CURRENT_TIMESTAMP WHERE chat_jid = ?1",
        params![chat_jid],
    )?;
    Ok(())
}


pub fn save_conversation_id(conn: &Connection, chat_jid: &str, conv_uuid: &str) -> anyhow::Result<()> {
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

pub fn delete_conversation_id(conn: &Connection, chat_jid: &str) -> anyhow::Result<()> {
    conn.execute("DELETE FROM chats WHERE chat_jid = ?1", params![chat_jid])?;
    Ok(())
}

pub fn record_message(
    conn: &Connection,
    chat_jid: &str,
    sender_jid: &str,
    text: &str,
    is_from_me: bool,
) -> anyhow::Result<()> {
    conn.execute(
        "INSERT INTO message_history (chat_jid, sender_jid, text, is_from_me)
         VALUES (?1, ?2, ?3, ?4)",
        params![chat_jid, sender_jid, text, is_from_me],
    )?;
    Ok(())
}

fn is_stopword(w: &str) -> bool {
    matches!(
        w,
        "apa"
            | "ya"
            | "yaa"
            | "sih"
            | "soal"
            | "tentang"
            | "yang"
            | "dan"
            | "di"
            | "ke"
            | "dari"
            | "ini"
            | "itu"
            | "ada"
            | "gak"
            | "tidak"
            | "bisa"
            | "tolong"
            | "kah"
            | "dong"
            | "kemarin"
            | "tadi"
            | "kita"
            | "kamu"
            | "saya"
            | "aku"
            | "the"
            | "is"
            | "and"
            | "what"
    )
}

pub fn search_message_history(
    conn: &Connection,
    chat_jid: &str,
    sender_jid: Option<&str>,
    query: &str,
    limit: usize,
) -> anyhow::Result<Vec<crate::core::ports::ChatMessageRecord>> {
    let clean_query = query.trim();
    if clean_query.is_empty() {
        return Ok(Vec::new());
    }

    let keywords: Vec<String> = clean_query
        .split_whitespace()
        .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase())
        .filter(|w| w.len() >= 3 && !is_stopword(w))
        .collect();

    let mut list = Vec::new();

    // Determine scope SQL and initial parameters
    let (scope_sql, mut params_vec): (String, Vec<Box<dyn rusqlite::ToSql>>) = match sender_jid {
        Some(sender) => (
            "(chat_jid = ?1 OR sender_jid = ?2 OR chat_jid IN (SELECT DISTINCT chat_jid FROM message_history WHERE sender_jid = ?2))".to_string(),
            vec![Box::new(chat_jid.to_string()), Box::new(sender.to_string())],
        ),
        None => (
            "chat_jid = ?1".to_string(),
            vec![Box::new(chat_jid.to_string())],
        ),
    };

    let param_offset = params_vec.len() + 1; // 2 or 3

    if !keywords.is_empty() {
        let mut clauses = Vec::new();
        for i in 0..keywords.len() {
            clauses.push(format!("text LIKE ?{}", param_offset + i));
            params_vec.push(Box::new(format!("%{}%", keywords[i])));
        }
        let sql = format!(
            "SELECT id, chat_jid, sender_jid, text, is_from_me, created_at
             FROM message_history
             WHERE {} AND ({})
             ORDER BY id DESC
             LIMIT {}",
            scope_sql,
            clauses.join(" OR "),
            limit
        );

        let mut stmt = conn.prepare(&sql)?;
        let params_slice: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|b| b.as_ref()).collect();
        let rows = stmt.query_map(&params_slice[..], |row| {
            Ok(crate::core::ports::ChatMessageRecord {
                id: row.get(0)?,
                chat_jid: row.get(1)?,
                sender_jid: row.get(2)?,
                text: row.get(3)?,
                is_from_me: row.get(4)?,
                created_at: row.get(5)?,
            })
        })?;

        for r in rows {
            list.push(r?);
        }
    } else {
        let search_pattern = format!("%{}%", clean_query);
        params_vec.push(Box::new(search_pattern));
        let kw_param_idx = params_vec.len();
        params_vec.push(Box::new(limit as i64));
        let limit_param_idx = params_vec.len();

        let sql = format!(
            "SELECT id, chat_jid, sender_jid, text, is_from_me, created_at
             FROM message_history
             WHERE {} AND text LIKE ?{}
             ORDER BY id DESC
             LIMIT ?{}",
            scope_sql,
            kw_param_idx,
            limit_param_idx
        );

        let mut stmt = conn.prepare(&sql)?;
        let params_slice: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|b| b.as_ref()).collect();
        let rows = stmt.query_map(&params_slice[..], |row| {
            Ok(crate::core::ports::ChatMessageRecord {
                id: row.get(0)?,
                chat_jid: row.get(1)?,
                sender_jid: row.get(2)?,
                text: row.get(3)?,
                is_from_me: row.get(4)?,
                created_at: row.get(5)?,
            })
        })?;

        for r in rows {
            list.push(r?);
        }
    }

    Ok(list)
}


pub fn get_recent_messages(
    conn: &Connection,
    chat_jid: &str,
    sender_jid: Option<&str>,
    limit: usize,
) -> anyhow::Result<Vec<crate::core::ports::ChatMessageRecord>> {
    let mut list = Vec::new();
    match sender_jid {
        Some(sender) => {
            let mut stmt = conn.prepare(
                "SELECT id, chat_jid, sender_jid, text, is_from_me, created_at
                 FROM message_history
                 WHERE chat_jid = ?1 OR sender_jid = ?2 OR chat_jid IN (SELECT DISTINCT chat_jid FROM message_history WHERE sender_jid = ?2)
                 ORDER BY id DESC
                 LIMIT ?3",
            )?;
            let rows = stmt.query_map(params![chat_jid, sender, limit as i64], |row| {
                Ok(crate::core::ports::ChatMessageRecord {
                    id: row.get(0)?,
                    chat_jid: row.get(1)?,
                    sender_jid: row.get(2)?,
                    text: row.get(3)?,
                    is_from_me: row.get(4)?,
                    created_at: row.get(5)?,
                })
            })?;
            for r in rows {
                list.push(r?);
            }
        }
        None => {
            let mut stmt = conn.prepare(
                "SELECT id, chat_jid, sender_jid, text, is_from_me, created_at
                 FROM message_history
                 WHERE chat_jid = ?1
                 ORDER BY id DESC
                 LIMIT ?2",
            )?;
            let rows = stmt.query_map(params![chat_jid, limit as i64], |row| {
                Ok(crate::core::ports::ChatMessageRecord {
                    id: row.get(0)?,
                    chat_jid: row.get(1)?,
                    sender_jid: row.get(2)?,
                    text: row.get(3)?,
                    is_from_me: row.get(4)?,
                    created_at: row.get(5)?,
                })
            })?;
            for r in rows {
                list.push(r?);
            }
        }
    }
    // Reverse so returned list is chronological
    list.reverse();
    Ok(list)
}


pub fn get_user_profile(conn: &Connection, sender_jid: &str) -> anyhow::Result<Option<UserProfile>> {
    let mut stmt = conn.prepare(
        "SELECT sender_jid, name, role, authority_level, notes FROM user_profiles WHERE sender_jid = ?1 LIMIT 1"
    )?;
    let mut rows = stmt.query(params![sender_jid])?;
    if let Some(row) = rows.next()? {
        Ok(Some(UserProfile {
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

pub fn save_user_profile(conn: &Connection, profile: &UserProfile) -> anyhow::Result<()> {
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

pub fn record_group_membership(
    conn: &Connection,
    group_jid: &str,
    user_jid: &str,
    user_name: Option<&str>,
    role_in_group: Option<&str>,
) -> anyhow::Result<()> {
    conn.execute(
        "INSERT INTO group_memberships (group_jid, user_jid, user_name, role_in_group, last_synced_at)
         VALUES (?1, ?2, ?3, COALESCE(?4, 'member'), CURRENT_TIMESTAMP)
         ON CONFLICT(group_jid, user_jid) DO UPDATE SET
            user_name = COALESCE(excluded.user_name, group_memberships.user_name),
            role_in_group = COALESCE(excluded.role_in_group, group_memberships.role_in_group),
            last_synced_at = CURRENT_TIMESTAMP",
        params![group_jid, user_jid, user_name, role_in_group],
    )?;
    Ok(())
}

pub fn get_user_groups(conn: &Connection, user_jid: &str) -> anyhow::Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT group_jid FROM group_memberships WHERE user_jid = ?1 ORDER BY last_synced_at DESC"
    )?;
    let rows = stmt.query_map(params![user_jid], |row| row.get(0))?;
    let mut groups = Vec::new();
    for r in rows {
        groups.push(r?);
    }
    Ok(groups)
}

pub fn get_group_members(
    conn: &Connection,
    group_jid: &str,
) -> anyhow::Result<Vec<crate::core::ports::GroupMemberRecord>> {
    let mut stmt = conn.prepare(
        "SELECT group_jid, user_jid, user_name, role_in_group, last_synced_at
         FROM group_memberships
         WHERE group_jid = ?1
         ORDER BY user_name ASC, user_jid ASC"
    )?;
    let rows = stmt.query_map(params![group_jid], |row| {
        Ok(crate::core::ports::GroupMemberRecord {
            group_jid: row.get(0)?,
            user_jid: row.get(1)?,
            user_name: row.get(2)?,
            role_in_group: row.get(3)?,
            last_synced_at: row.get(4)?,
        })
    })?;
    let mut members = Vec::new();
    for r in rows {
        members.push(r?);
    }
    Ok(members)
}

pub fn is_user_in_group(conn: &Connection, user_jid: &str, group_jid: &str) -> anyhow::Result<bool> {
    let mut stmt = conn.prepare(
        "SELECT 1 FROM group_memberships WHERE user_jid = ?1 AND group_jid = ?2 LIMIT 1"
    )?;
    let exists = stmt.exists(params![user_jid, group_jid])?;
    Ok(exists)
}
