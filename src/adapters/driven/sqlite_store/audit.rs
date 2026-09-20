use crate::core::domain::{
    ActionAuditFilter, AuditSummaryReport, CountMetric, NewWhatsAppActionAudit, WhatsAppActionAudit,
};
use rusqlite::{params, Connection, Row};
use std::collections::HashMap;

pub fn map_action_audit_row(row: &Row) -> rusqlite::Result<WhatsAppActionAudit> {
    let has_media_int: i32 = row.get(11)?;
    let tools_json: String = row.get(16)?;
    let tools_invoked: Vec<String> = serde_json::from_str(&tools_json).unwrap_or_default();

    Ok(WhatsAppActionAudit {
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

pub fn record_action_audit(conn: &Connection, audit: &NewWhatsAppActionAudit) -> anyhow::Result<i64> {
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

pub fn update_action_audit_result(
    conn: &Connection,
    id: i64,
    conversation_id: Option<&str>,
    response_text: Option<&str>,
    error_message: Option<&str>,
    status: &str,
    duration_seconds: Option<f64>,
    tools_invoked: &[String],
) -> anyhow::Result<()> {
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

pub fn query_action_audits(
    conn: &Connection,
    filter: &ActionAuditFilter,
) -> anyhow::Result<Vec<WhatsAppActionAudit>> {
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

pub fn get_action_audit_by_id(conn: &Connection, id: i64) -> anyhow::Result<Option<WhatsAppActionAudit>> {
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

pub fn get_action_audit_by_message_id(conn: &Connection, message_id: &str) -> anyhow::Result<Option<WhatsAppActionAudit>> {
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

pub fn get_action_audit_summary(conn: &Connection) -> anyhow::Result<AuditSummaryReport> {
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
        Ok(CountMetric {
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
        Ok(CountMetric {
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
        Ok(CountMetric {
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
        Ok(CountMetric {
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
    let mut tool_counts: HashMap<String, i64> = HashMap::new();
    for t_res in tool_rows.flatten() {
        if let Ok(arr) = serde_json::from_str::<Vec<String>>(&t_res) {
            for tool_name in arr {
                *tool_counts.entry(tool_name).or_insert(0) += 1;
            }
        }
    }
    let mut top_tools: Vec<CountMetric> = tool_counts
        .into_iter()
        .map(|(key, count)| CountMetric { key, count })
        .collect();
    top_tools.sort_by(|a, b| b.count.cmp(&a.count));
    top_tools.truncate(10);

    Ok(AuditSummaryReport {
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
