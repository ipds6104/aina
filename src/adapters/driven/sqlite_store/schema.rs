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
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            turn_count INTEGER NOT NULL DEFAULT 1
        )",
        [],
    )?;

    // Backward-compatible schema migration: ensure 'turn_count' exists on chats
    let has_turn_count_col: bool = conn
        .prepare("PRAGMA table_info(chats)")?
        .query_map([], |row| row.get::<_, String>(1))?
        .filter_map(|r| r.ok())
        .any(|name| name == "turn_count");

    if !has_turn_count_col {
        let _ = conn.execute(
            "ALTER TABLE chats ADD COLUMN turn_count INTEGER NOT NULL DEFAULT 1",
            [],
        );
    }

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
        "CREATE INDEX IF NOT EXISTS idx_msg_history_chat 
         ON message_history(chat_jid, id DESC)",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_msg_history_sender 
         ON message_history(sender_jid, id DESC)",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS user_profiles (
            sender_jid TEXT PRIMARY KEY,
            name TEXT,
            role TEXT,
            authority_level TEXT NOT NULL DEFAULT 'guest',
            notes TEXT,
            granted_by TEXT,
            granted_at INTEGER,
            role_hierarchy TEXT DEFAULT 'guest',
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
        )",
        [],
    )?;

    // Ensure columns exist on legacy databases for progressive trust & delegation
    let _ = conn.execute("ALTER TABLE user_profiles ADD COLUMN granted_by TEXT", []);
    let _ = conn.execute("ALTER TABLE user_profiles ADD COLUMN granted_at INTEGER", []);
    let _ = conn.execute("ALTER TABLE user_profiles ADD COLUMN role_hierarchy TEXT DEFAULT 'guest'", []);

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
            last_duration_secs REAL,
            slug TEXT,
            category TEXT DEFAULT 'custom',
            source TEXT DEFAULT 'custom',
            metadata TEXT
        )",
        [],
    )?;

    // Ensure columns exist on legacy databases
    let _ = conn.execute("ALTER TABLE scheduled_tasks ADD COLUMN last_status TEXT", []);
    let _ = conn.execute("ALTER TABLE scheduled_tasks ADD COLUMN last_error TEXT", []);
    let _ = conn.execute("ALTER TABLE scheduled_tasks ADD COLUMN last_duration_secs REAL", []);
    let _ = conn.execute("ALTER TABLE scheduled_tasks ADD COLUMN slug TEXT", []);
    let _ = conn.execute("ALTER TABLE scheduled_tasks ADD COLUMN category TEXT DEFAULT 'custom'", []);
    let _ = conn.execute("ALTER TABLE scheduled_tasks ADD COLUMN source TEXT DEFAULT 'custom'", []);
    let _ = conn.execute("ALTER TABLE scheduled_tasks ADD COLUMN metadata TEXT", []);

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_scheduled_tasks_next_run 
         ON scheduled_tasks(is_active, next_run_epoch)",
        [],
    )?;

    conn.execute(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_scheduled_tasks_slug 
         ON scheduled_tasks(slug) WHERE slug IS NOT NULL",
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

    // Group Memberships Table (Cross-channel Group to DM Identity Bridge)
    conn.execute(
        "CREATE TABLE IF NOT EXISTS group_memberships (
            group_jid TEXT NOT NULL,
            user_jid TEXT NOT NULL,
            user_name TEXT,
            role_in_group TEXT NOT NULL DEFAULT 'member',
            is_group_trusted BOOLEAN NOT NULL DEFAULT 0,
            authorized_by TEXT,
            authorized_at INTEGER,
            last_synced_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (group_jid, user_jid)
        )",
        [],
    )?;

    // Ensure columns exist on legacy databases for group delegation
    let _ = conn.execute("ALTER TABLE group_memberships ADD COLUMN is_group_trusted BOOLEAN NOT NULL DEFAULT 0", []);
    let _ = conn.execute("ALTER TABLE group_memberships ADD COLUMN authorized_by TEXT", []);
    let _ = conn.execute("ALTER TABLE group_memberships ADD COLUMN authorized_at INTEGER", []);

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_group_memberships_user 
         ON group_memberships(user_jid)",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_group_memberships_group 
         ON group_memberships(group_jid)",
        [],
    )?;

    // Short-lived turn capabilities: the only proof the agent may present for a
    // privileged action. Minted by the core for the verified sender of a turn.
    conn.execute(
        "CREATE TABLE IF NOT EXISTS turn_capabilities (
            token TEXT PRIMARY KEY,
            principal_jid TEXT NOT NULL,
            authority TEXT NOT NULL,
            chat_jid TEXT NOT NULL,
            chat_type TEXT NOT NULL,
            message_id TEXT NOT NULL,
            issued_at INTEGER NOT NULL,
            expires_at INTEGER NOT NULL
        )",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_turn_capabilities_expiry
         ON turn_capabilities(expires_at)",
        [],
    )?;

    // Experience Records Table (Harness² Experience Reflection format: cue, lesson, evidence)
    conn.execute(
        "CREATE TABLE IF NOT EXISTS experience_records (
            id TEXT PRIMARY KEY,
            domain TEXT NOT NULL,
            cue TEXT NOT NULL,
            lesson TEXT NOT NULL,
            evidence TEXT NOT NULL,
            confidence REAL NOT NULL DEFAULT 1.0,
            task_id TEXT,
            created_at_epoch INTEGER NOT NULL
        )",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_experience_domain 
         ON experience_records(domain, created_at_epoch DESC)",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_experience_created 
         ON experience_records(created_at_epoch DESC)",
        [],
    )?;

    Ok(())
}
