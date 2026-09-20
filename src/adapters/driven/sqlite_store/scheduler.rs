use crate::core::domain::{
    NewScheduledTask, ScheduledTask, ScheduledTaskRun, ScheduledTaskType, SchedulerDiagnostics,
};
use rusqlite::{params, Connection};

pub fn create_scheduled_task(conn: &Connection, task: &NewScheduledTask) -> anyhow::Result<i64> {
    let now_epoch = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    // Idempotency check: check if an active task already exists for this target around the same time (+/- 120s)
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

pub fn list_scheduled_tasks(conn: &Connection, active_only: bool) -> anyhow::Result<Vec<ScheduledTask>> {
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
        Ok(ScheduledTask {
            id: row.get(0)?,
            title: row.get(1)?,
            task_type: ScheduledTaskType::from_str(&task_type_str),
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

pub fn get_due_scheduled_tasks(conn: &Connection, current_epoch: i64) -> anyhow::Result<Vec<ScheduledTask>> {
    let mut stmt = conn.prepare(
        "SELECT id, title, task_type, target_jid, payload, schedule_type, schedule_expr, next_run_epoch, last_run_epoch, is_active, created_at, last_status, last_error, last_duration_secs
         FROM scheduled_tasks 
         WHERE is_active = 1 AND next_run_epoch <= ?1
         ORDER BY next_run_epoch ASC"
    )?;

    let rows = stmt.query_map(params![current_epoch], |row| {
        let task_type_str: String = row.get(2)?;
        let active_int: i32 = row.get(9)?;
        Ok(ScheduledTask {
            id: row.get(0)?,
            title: row.get(1)?,
            task_type: ScheduledTaskType::from_str(&task_type_str),
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

pub fn update_scheduled_task_run(
    conn: &Connection,
    id: i64,
    last_run: i64,
    next_run: Option<i64>,
    is_active: bool,
) -> anyhow::Result<()> {
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

pub fn delete_scheduled_task(conn: &Connection, id: i64) -> anyhow::Result<bool> {
    let affected = conn.execute("DELETE FROM scheduled_tasks WHERE id = ?1", params![id])?;
    Ok(affected > 0)
}

pub fn get_scheduled_task(conn: &Connection, id: i64) -> anyhow::Result<Option<ScheduledTask>> {
    let mut stmt = conn.prepare(
        "SELECT id, title, task_type, target_jid, payload, schedule_type, schedule_expr, next_run_epoch, last_run_epoch, is_active, created_at, last_status, last_error, last_duration_secs
         FROM scheduled_tasks WHERE id = ?1 LIMIT 1"
    )?;

    let mut rows = stmt.query(params![id])?;
    if let Some(row) = rows.next()? {
        let task_type_str: String = row.get(2)?;
        let active_int: i32 = row.get(9)?;
        Ok(Some(ScheduledTask {
            id: row.get(0)?,
            title: row.get(1)?,
            task_type: ScheduledTaskType::from_str(&task_type_str),
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

pub fn record_scheduled_task_run(
    conn: &Connection,
    task_id: i64,
    task_title: &str,
    target_jid: &str,
    status: &str,
    duration_secs: f64,
    error_message: Option<&str>,
    output_preview: Option<&str>,
) -> anyhow::Result<i64> {
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

pub fn list_scheduled_task_runs(conn: &Connection, limit: usize) -> anyhow::Result<Vec<ScheduledTaskRun>> {
    let mut stmt = conn.prepare(
        "SELECT id, task_id, task_title, target_jid, status, duration_secs, error_message, output_preview, executed_at_epoch
         FROM scheduled_task_runs
         ORDER BY executed_at_epoch DESC, id DESC
         LIMIT ?1"
    )?;

    let rows = stmt.query_map(params![limit as i64], |row| {
        Ok(ScheduledTaskRun {
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

pub fn update_scheduled_task_result(
    conn: &Connection,
    id: i64,
    status: &str,
    error_message: Option<&str>,
    duration_secs: f64,
) -> anyhow::Result<()> {
    conn.execute(
        "UPDATE scheduled_tasks 
         SET last_status = ?1, last_error = ?2, last_duration_secs = ?3 
         WHERE id = ?4",
        params![status, error_message, duration_secs, id],
    )?;
    Ok(())
}

pub fn get_scheduler_diagnostics(conn: &Connection) -> anyhow::Result<SchedulerDiagnostics> {
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

    let last_run: Option<ScheduledTaskRun> = conn.query_row(
        "SELECT id, task_id, task_title, target_jid, status, duration_secs, error_message, output_preview, executed_at_epoch
         FROM scheduled_task_runs
         ORDER BY executed_at_epoch DESC, id DESC LIMIT 1",
        [],
        |row| Ok(ScheduledTaskRun {
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

    let last_failure: Option<ScheduledTaskRun> = conn.query_row(
        "SELECT id, task_id, task_title, target_jid, status, duration_secs, error_message, output_preview, executed_at_epoch
         FROM scheduled_task_runs
         WHERE status = 'failed'
         ORDER BY executed_at_epoch DESC, id DESC LIMIT 1",
        [],
        |row| Ok(ScheduledTaskRun {
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

    let next_task: Option<ScheduledTask> = conn.query_row(
        "SELECT id, title, task_type, target_jid, payload, schedule_type, schedule_expr, next_run_epoch, last_run_epoch, is_active, created_at, last_status, last_error, last_duration_secs
         FROM scheduled_tasks
         WHERE is_active = 1
         ORDER BY next_run_epoch ASC LIMIT 1",
        [],
        |row| {
            let task_type_str: String = row.get(2)?;
            let active_int: i32 = row.get(9)?;
            Ok(ScheduledTask {
                id: row.get(0)?,
                title: row.get(1)?,
                task_type: ScheduledTaskType::from_str(&task_type_str),
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

    Ok(SchedulerDiagnostics {
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
