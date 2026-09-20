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
