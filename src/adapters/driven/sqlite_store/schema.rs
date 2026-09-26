use rusqlite::Connection;

pub fn init_schema(conn: &Connection) -> anyhow::Result<()> {
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
            created_at INTEGER NOT NULL,
            last_status TEXT,
            last_error TEXT,
            last_duration_secs REAL
        )",
        [],
    )?;

    // Ensure columns exist on legacy databases
    let _ = conn.execute("ALTER TABLE scheduled_tasks ADD COLUMN last_status TEXT", []);
    let _ = conn.execute("ALTER TABLE scheduled_tasks ADD COLUMN last_error TEXT", []);
    let _ = conn.execute("ALTER TABLE scheduled_tasks ADD COLUMN last_duration_secs REAL", []);

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_scheduled_tasks_next_run 
         ON scheduled_tasks(is_active, next_run_epoch)",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS scheduled_task_runs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            task_id INTEGER NOT NULL,
            task_title TEXT NOT NULL,
            target_jid TEXT NOT NULL,
            status TEXT NOT NULL,
            duration_secs REAL NOT NULL,
            error_message TEXT,
            output_preview TEXT,
            executed_at_epoch INTEGER NOT NULL
        )",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_task_runs_task_id 
         ON scheduled_task_runs(task_id, executed_at_epoch DESC)",
        [],
    )?;

    // WhatsApp Action Audits Table & Indexes
    conn.execute(
        "CREATE TABLE IF NOT EXISTS whatsapp_action_audits (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            message_id TEXT NOT NULL,
            chat_jid TEXT NOT NULL,
            chat_type TEXT NOT NULL,
            sender_jid TEXT NOT NULL,
            sender_name TEXT,
            decision TEXT NOT NULL,
            decision_reason TEXT NOT NULL,
            conversation_id TEXT,
            status TEXT NOT NULL,
            input_text TEXT NOT NULL,
            has_media BOOLEAN NOT NULL DEFAULT 0,
            media_path TEXT,
            response_text TEXT,
            error_message TEXT,
            duration_seconds REAL,
            tools_invoked TEXT NOT NULL DEFAULT '[]',
            usecase TEXT NOT NULL DEFAULT 'casual_and_consultation',
            created_at_epoch INTEGER NOT NULL,
            completed_at_epoch INTEGER
        )",
        [],
    )?;

    // Backward-compatible schema migration for existing databases: ensure 'usecase' exists
    let has_usecase_col: bool = conn
        .prepare("PRAGMA table_info(whatsapp_action_audits)")?
        .query_map([], |row| row.get::<_, String>(1))?
        .filter_map(|r| r.ok())
        .any(|name| name == "usecase");

    if !has_usecase_col {
        let _ = conn.execute(
            "ALTER TABLE whatsapp_action_audits ADD COLUMN usecase TEXT NOT NULL DEFAULT 'casual_and_consultation'",
            [],
        );
    }

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_action_audits_chat 
         ON whatsapp_action_audits(chat_jid, created_at_epoch DESC)",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_action_audits_sender 
         ON whatsapp_action_audits(sender_jid, created_at_epoch DESC)",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_action_audits_status 
         ON whatsapp_action_audits(status, created_at_epoch DESC)",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_action_audits_usecase 
         ON whatsapp_action_audits(usecase, created_at_epoch DESC)",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_action_audits_msg_id 
         ON whatsapp_action_audits(message_id)",
        [],
    )?;

    // Metacognitive Predictions Table & Indexes (Syarat 1 & 2 Definisi D)
    conn.execute(
        "CREATE TABLE IF NOT EXISTS metacognitive_predictions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            prediction_id TEXT NOT NULL UNIQUE,
            action_audit_id INTEGER,
            task_description TEXT NOT NULL,
            domain_type TEXT NOT NULL,
            predicted_probability REAL NOT NULL,
            complexity_tier TEXT NOT NULL,
            identified_risks TEXT NOT NULL DEFAULT '[]',
            fallback_strategy TEXT,
            actual_outcome REAL,
            brier_score REAL,
            execution_duration_secs REAL,
            error_detail TEXT,
            created_at_epoch INTEGER NOT NULL,
            resolved_at_epoch INTEGER
        )",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_metacog_domain 
         ON metacognitive_predictions(domain_type, created_at_epoch DESC)",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_metacog_created 
         ON metacognitive_predictions(created_at_epoch DESC)",
        [],
    )?;

    Ok(())
}
