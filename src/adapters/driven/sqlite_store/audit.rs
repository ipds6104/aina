use crate::core::domain::{
    ActionAuditFilter, AgentToolExecution, AuditSummaryReport, CountMetric, NewAgentToolExecution,
    NewWhatsAppActionAudit, WhatsAppActionAudit,
};
use rusqlite::{params, Connection, Row};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

pub fn map_action_audit_row(row: &Row) -> rusqlite::Result<WhatsAppActionAudit> {
    let has_media_int: i32 = row.get(11)?;
    let tools_json: String = row.get(16)?;
    let tools_invoked: Vec<String> = serde_json::from_str(&tools_json).unwrap_or_default();
    let usecase: String = row.get(17).unwrap_or_else(|_| "casual_and_consultation".to_string());

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
        usecase,
        created_at_epoch: row.get(18)?,
        completed_at_epoch: row.get(19)?,
    })
}

pub fn record_action_audit(conn: &Connection, audit: &NewWhatsAppActionAudit) -> anyhow::Result<i64> {
    let tools_json = serde_json::to_string(&audit.tools_invoked).unwrap_or_else(|_| "[]".to_string());
    conn.execute(
        "INSERT INTO whatsapp_action_audits (
            message_id, chat_jid, chat_type, sender_jid, sender_name,
            decision, decision_reason, conversation_id, status, input_text,
            has_media, media_path, response_text, error_message, duration_seconds,
            tools_invoked, usecase, created_at_epoch, completed_at_epoch
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)",
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
            audit.usecase,
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
    usecase: Option<&str>,
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
            usecase = COALESCE(?7, usecase),
            completed_at_epoch = ?8
        WHERE id = ?9",
        params![
            conversation_id,
            response_text,
            error_message,
            status,
            duration_seconds,
            tools_json,
            usecase,
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
                tools_invoked, usecase, created_at_epoch, completed_at_epoch
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

    if let Some(ref uc) = filter.usecase {
        conditions.push("usecase = ?");
        query_params.push(Box::new(uc.clone()));
    }

    if let Some(ref tool) = filter.tool {
        let tool_pattern = format!("%\"{}\"%", tool);
        conditions.push("tools_invoked LIKE ?");
        query_params.push(Box::new(tool_pattern));
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
                tools_invoked, usecase, created_at_epoch, completed_at_epoch
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
                tools_invoked, usecase, created_at_epoch, completed_at_epoch
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

    // Usecase breakdown & top usecases (fast indexed SQL aggregation)
    let mut stmt_uc = conn.prepare(
        "SELECT usecase, COUNT(*) as cnt FROM whatsapp_action_audits WHERE usecase IS NOT NULL AND usecase != '' GROUP BY usecase ORDER BY cnt DESC"
    )?;
    let uc_rows = stmt_uc.query_map([], |r| {
        Ok(CountMetric {
            key: r.get(0)?,
            count: r.get(1)?,
        })
    })?;
    let usecase_breakdown: Vec<CountMetric> = uc_rows.filter_map(|r| r.ok()).collect();
    let top_usecases: Vec<CountMetric> = usecase_breakdown.iter().take(5).cloned().collect();

    // Top tools used & total call frequencies (parse tools_invoked)
    let mut stmt_tools = conn.prepare(
        "SELECT tools_invoked FROM whatsapp_action_audits WHERE tools_invoked != '[]' ORDER BY id DESC LIMIT 1000"
    )?;
    let tool_rows = stmt_tools.query_map([], |r| r.get::<_, String>(0))?;
    let mut tool_action_counts: HashMap<String, i64> = HashMap::new();
    let mut tool_freq_counts: HashMap<String, i64> = HashMap::new();

    for t_res in tool_rows.flatten() {
        if let Ok(arr) = serde_json::from_str::<Vec<String>>(&t_res) {
            let mut seen_in_action = std::collections::HashSet::new();
            for tool_name in arr {
                *tool_freq_counts.entry(tool_name.clone()).or_insert(0) += 1;
                if seen_in_action.insert(tool_name.clone()) {
                    *tool_action_counts.entry(tool_name).or_insert(0) += 1;
                }
            }
        }
    }

    let mut top_tools: Vec<CountMetric> = tool_action_counts
        .into_iter()
        .map(|(key, count)| CountMetric { key, count })
        .collect();
    top_tools.sort_by(|a, b| b.count.cmp(&a.count));
    top_tools.truncate(10);

    let mut tool_call_frequency: Vec<CountMetric> = tool_freq_counts
        .into_iter()
        .map(|(key, count)| CountMetric { key, count })
        .collect();
    tool_call_frequency.sort_by(|a, b| b.count.cmp(&a.count));
    tool_call_frequency.truncate(10);

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
        top_usecases,
        usecase_breakdown,
        tool_call_frequency,
    })
}

fn to_hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        use std::fmt::Write;
        let _ = write!(s, "{:02x}", b);
    }
    s
}

pub fn record_tool_execution(
    conn: &Connection,
    exec: &NewAgentToolExecution,
) -> anyhow::Result<AgentToolExecution> {
    // 1. Get the latest record_hash (genesis is 64 zeros)
    let prev_hash: String = conn
        .query_row(
            "SELECT record_hash FROM agent_tool_executions ORDER BY id DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .unwrap_or_else(|_| "0000000000000000000000000000000000000000000000000000000000000000".to_string());

    // 2. Compute current record hash: sha256(prev_hash + sender_jid + tool_name + command_payload + created_at_epoch)
    let mut hasher = Sha256::new();
    hasher.update(prev_hash.as_bytes());
    hasher.update(exec.turn_token_prefix.as_bytes());
    hasher.update(exec.sender_jid.as_bytes());
    hasher.update(exec.sender_authority.as_bytes());
    hasher.update(exec.tool_name.as_bytes());
    hasher.update(exec.command_payload.as_bytes());
    hasher.update(exec.execution_status.as_bytes());
    hasher.update(exec.duration_ms.to_string().as_bytes());
    hasher.update(exec.created_at_epoch.to_string().as_bytes());
    let record_hash = to_hex(&hasher.finalize());

    conn.execute(
        "INSERT INTO agent_tool_executions (
            action_audit_id, turn_token_prefix, sender_jid, sender_authority,
            tool_name, command_payload, execution_status, exit_code, duration_ms,
            prev_hash, record_hash, created_at_epoch
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            exec.action_audit_id,
            exec.turn_token_prefix,
            exec.sender_jid,
            exec.sender_authority,
            exec.tool_name,
            exec.command_payload,
            exec.execution_status,
            exec.exit_code,
            exec.duration_ms,
            prev_hash,
            record_hash,
            exec.created_at_epoch,
        ],
    )?;

    let id = conn.last_insert_rowid();

    Ok(AgentToolExecution {
        id,
        action_audit_id: exec.action_audit_id,
        turn_token_prefix: exec.turn_token_prefix.clone(),
        sender_jid: exec.sender_jid.clone(),
        sender_authority: exec.sender_authority.clone(),
        tool_name: exec.tool_name.clone(),
        command_payload: exec.command_payload.clone(),
        execution_status: exec.execution_status.clone(),
        exit_code: exec.exit_code,
        duration_ms: exec.duration_ms,
        prev_hash,
        record_hash,
        created_at_epoch: exec.created_at_epoch,
    })
}

pub fn verify_audit_hash_chain(conn: &Connection) -> anyhow::Result<bool> {
    let mut stmt = conn.prepare(
        "SELECT id, turn_token_prefix, sender_jid, sender_authority, tool_name,
                command_payload, execution_status, duration_ms, prev_hash, record_hash, created_at_epoch
         FROM agent_tool_executions ORDER BY id ASC"
    )?;

    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
            row.get::<_, String>(5)?,
            row.get::<_, String>(6)?,
            row.get::<_, i64>(7)?,
            row.get::<_, String>(8)?,
            row.get::<_, String>(9)?,
            row.get::<_, i64>(10)?,
        ))
    })?;

    let mut expected_prev_hash = "0000000000000000000000000000000000000000000000000000000000000000".to_string();

    for r in rows {
        let (id, turn_token_prefix, sender_jid, sender_authority, tool_name, command_payload, execution_status, duration_ms, prev_hash, record_hash, created_at_epoch) = r?;

        if prev_hash != expected_prev_hash {
            anyhow::bail!(
                "Rantai hash putus pada audit ID {}: prev_hash '{}' tidak cocok dengan hash sebelumnya '{}'",
                id, prev_hash, expected_prev_hash
            );
        }

        let mut hasher = Sha256::new();
        hasher.update(prev_hash.as_bytes());
        hasher.update(turn_token_prefix.as_bytes());
        hasher.update(sender_jid.as_bytes());
        hasher.update(sender_authority.as_bytes());
        hasher.update(tool_name.as_bytes());
        hasher.update(command_payload.as_bytes());
        hasher.update(execution_status.as_bytes());
        hasher.update(duration_ms.to_string().as_bytes());
        hasher.update(created_at_epoch.to_string().as_bytes());
        let computed_hash = to_hex(&hasher.finalize());

        if record_hash != computed_hash {
            anyhow::bail!(
                "Integritas data termanipulasi pada audit ID {}: record_hash '{}' tidak cocok dengan kalkulasi SHA-256 '{}'",
                id, record_hash, computed_hash
            );
        }

        expected_prev_hash = record_hash;
    }

    Ok(true)
}

pub fn get_tool_executions_by_sender(
    conn: &Connection,
    sender_jid: &str,
    limit: usize,
) -> anyhow::Result<Vec<AgentToolExecution>> {
    let mut stmt = conn.prepare(
        "SELECT id, action_audit_id, turn_token_prefix, sender_jid, sender_authority,
                tool_name, command_payload, execution_status, exit_code, duration_ms,
                prev_hash, record_hash, created_at_epoch
         FROM agent_tool_executions
         WHERE sender_jid = ?1
         ORDER BY id DESC LIMIT ?2"
    )?;

    let rows = stmt.query_map(params![sender_jid, limit as i64], |row| {
        Ok(AgentToolExecution {
            id: row.get(0)?,
            action_audit_id: row.get(1)?,
            turn_token_prefix: row.get(2)?,
            sender_jid: row.get(3)?,
            sender_authority: row.get(4)?,
            tool_name: row.get(5)?,
            command_payload: row.get(6)?,
            execution_status: row.get(7)?,
            exit_code: row.get(8)?,
            duration_ms: row.get(9)?,
            prev_hash: row.get(10)?,
            record_hash: row.get(11)?,
            created_at_epoch: row.get(12)?,
        })
    })?;

    let mut list = Vec::new();
    for r in rows {
        list.push(r?);
    }
    Ok(list)
}

pub fn get_tool_executions_by_tool(
    conn: &Connection,
    tool_name: &str,
    limit: usize,
) -> anyhow::Result<Vec<AgentToolExecution>> {
    let mut stmt = conn.prepare(
        "SELECT id, action_audit_id, turn_token_prefix, sender_jid, sender_authority,
                tool_name, command_payload, execution_status, exit_code, duration_ms,
                prev_hash, record_hash, created_at_epoch
         FROM agent_tool_executions
         WHERE tool_name = ?1
         ORDER BY id DESC LIMIT ?2"
    )?;

    let rows = stmt.query_map(params![tool_name, limit as i64], |row| {
        Ok(AgentToolExecution {
            id: row.get(0)?,
            action_audit_id: row.get(1)?,
            turn_token_prefix: row.get(2)?,
            sender_jid: row.get(3)?,
            sender_authority: row.get(4)?,
            tool_name: row.get(5)?,
            command_payload: row.get(6)?,
            execution_status: row.get(7)?,
            exit_code: row.get(8)?,
            duration_ms: row.get(9)?,
            prev_hash: row.get(10)?,
            record_hash: row.get(11)?,
            created_at_epoch: row.get(12)?,
        })
    })?;

    let mut list = Vec::new();
    for r in rows {
        list.push(r?);
    }
    Ok(list)
}
