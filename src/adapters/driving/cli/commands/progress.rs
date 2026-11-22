//! `aina progress` — sanctioned, rate-limited, human-like in-flight progress channel
//! and elapsed-time awareness utility for the active agent turn.
//!
//! Security & Architecture:
//! - Authenticates strictly via the per-turn unforgeable `AINA_TURN_TOKEN` environment
//!   variable bound to `turn_progress_sessions` in SQLite.
//! - Requires ZERO WhatsApp API secrets in the agent subprocess (works even on
//!   least-privilege `GUEST` turns without violating Phase 20 env isolation).
//! - Atomically enforces `ProgressPolicy` (initial delay, DM/group cooldowns, update caps,
//!   length limit, deduplication) inside a SQLite `BEGIN IMMEDIATE` transaction and writes
//!   approved messages to `turn_progress_outbox` for delivery by the daemon heartbeat loop.

use crate::core::domain::{format_elapsed_indonesian, ProgressDecision, ProgressPolicy};
use rusqlite::Connection;
use serde_json::json;

pub fn handle_progress(args: &[String]) -> anyhow::Result<()> {
    if args.is_empty() || matches!(args[0].as_str(), "help" | "--help" | "-h") {
        print_help();
        return Ok(());
    }

    let flags = ProgressFlags::parse(args);
    let as_json = flags.json;

    let conn = match open_db() {
        Ok(c) => c,
        Err(e) => {
            emit(
                as_json,
                json!({ "ok": false, "reason": "database_unavailable", "detail": e.to_string() }),
                format!("❌ Tidak dapat membuka database: {}", e),
            );
            return Ok(());
        }
    };

    let token = std::env::var("AINA_TURN_TOKEN").unwrap_or_default();
    if token.trim().is_empty() {
        emit(
            as_json,
            json!({
                "ok": false,
                "reason": "missing_turn_token",
                "hint": "`aina progress` hanya dapat dipakai dari dalam tur percakapan WhatsApp yang aktif."
            }),
            "⛔ Tidak ada `AINA_TURN_TOKEN` aktif pada konteks ini.".to_string(),
        );
        return Ok(());
    }

    let now = chrono::Utc::now().timestamp();
    let policy = ProgressPolicy::from_env();

    let is_status_query = flags.elapsed_only
        || flags.positional.first().map(|s| s.as_str()) == Some("status")
        || flags.positional.first().map(|s| s.as_str()) == Some("elapsed");

    if is_status_query {
        return cmd_status(&conn, &token, &policy, now, as_json);
    }

    // Extract progress message: support both `aina progress "pesan"` and `aina progress send "pesan"`
    let message = if flags.positional.first().map(|s| s.as_str()) == Some("send") {
        flags.positional[1..].join(" ")
    } else {
        flags.positional.join(" ")
    };

    cmd_send(&conn, &token, &message, &policy, now, as_json)
}

fn cmd_status(
    conn: &Connection,
    token: &str,
    policy: &ProgressPolicy,
    now: i64,
    as_json: bool,
) -> anyhow::Result<()> {
    let Some(session) =
        crate::adapters::driven::sqlite_store::session::get_turn_progress_session(conn, token)?
    else {
        emit(
            as_json,
            json!({
                "ok": false,
                "reason": "turn_session_not_found",
                "hint": "Sesi progres untuk tur ini tidak ditemukan."
            }),
            "⚠️ Sesi progres untuk tur ini tidak ditemukan.".to_string(),
        );
        return Ok(());
    };

    let elapsed_secs = (now - session.started_at_epoch).max(0);
    let elapsed_human = format_elapsed_indonesian(elapsed_secs);
    let max_updates = policy.max_updates_for(&session.chat_type);
    let cooldown_remaining = policy.cooldown_remaining_secs(&session, now);
    let since_last = session.last_progress_at_epoch.map(|t| (now - t).max(0));
    let can_send_now =
        session.is_active && session.progress_count < max_updates && cooldown_remaining == 0;

    let payload = json!({
        "ok": true,
        "active": session.is_active,
        "chat_type": session.chat_type,
        "started_at_epoch": session.started_at_epoch,
        "elapsed_seconds": elapsed_secs,
        "elapsed_human": elapsed_human,
        "updates_sent": session.progress_count,
        "max_updates": max_updates,
        "seconds_since_last_update": since_last,
        "cooldown_remaining_seconds": cooldown_remaining,
        "can_send_now": can_send_now,
        "last_progress": session.last_progress_text,
    });

    let human = format!(
        "⏱️ Durasi berjalan: {} ({} detik) | Update terkirim: {}/{} | Bisa kirim sekarang: {} (sisa cooldown: {} detik)",
        elapsed_human,
        elapsed_secs,
        session.progress_count,
        max_updates,
        if can_send_now { "Ya" } else { "Belum" },
        cooldown_remaining
    );

    emit(as_json, payload, human);
    Ok(())
}

fn cmd_send(
    conn: &Connection,
    token: &str,
    raw_message: &str,
    policy: &ProgressPolicy,
    now: i64,
    as_json: bool,
) -> anyhow::Result<()> {
    let outcome = crate::adapters::driven::sqlite_store::session::try_enqueue_turn_progress_update(
        conn,
        token,
        raw_message,
        policy,
        now,
    )?;

    let Some((decision, session)) = outcome else {
        emit(
            as_json,
            json!({
                "ok": false,
                "reason": "turn_session_not_found",
                "hint": "Sesi progres untuk tur ini tidak ditemukan."
            }),
            "⚠️ Sesi progres untuk tur ini tidak ditemukan.".to_string(),
        );
        return Ok(());
    };

    let max_updates = policy.max_updates_for(&session.chat_type);
    match decision {
        ProgressDecision::Allow {
            sanitized_text,
            elapsed_secs,
        } => {
            let elapsed_human = format_elapsed_indonesian(elapsed_secs);
            let cooldown_secs = policy.cooldown_for(&session.chat_type);
            let payload = json!({
                "ok": true,
                "queued": true,
                "message": sanitized_text,
                "elapsed_seconds": elapsed_secs,
                "elapsed_human": elapsed_human,
                "updates_sent": session.progress_count,
                "max_updates": max_updates,
                "next_cooldown_seconds": cooldown_secs,
            });
            let human = format!(
                "✅ Update progres dijadwalkan untuk segera dikirim ({} berjalan, update {}/{}). Jawaban final tetap ditulis di respons akhir.",
                elapsed_human, session.progress_count, max_updates
            );
            emit(as_json, payload, human);
        }
        ProgressDecision::Deny {
            code,
            reason,
            retry_after_secs,
        } => {
            let elapsed_secs = (now - session.started_at_epoch).max(0);
            let payload = json!({
                "ok": false,
                "queued": false,
                "reason": code,
                "detail": reason,
                "retry_after_seconds": retry_after_secs,
                "elapsed_seconds": elapsed_secs,
                "elapsed_human": format_elapsed_indonesian(elapsed_secs),
                "updates_sent": session.progress_count,
                "max_updates": max_updates,
            });
            let human = format!("⏳ Update progres ditahan ({}): {}", code, reason);
            emit(as_json, payload, human);
        }
    }

    Ok(())
}

fn open_db() -> anyhow::Result<Connection> {
    let config_path =
        std::env::var("AINA_CONFIG").unwrap_or_else(|_| "config/config.yaml".to_string());
    let config = crate::config::AppConfig::load_from_file_or_default(&config_path);
    let db_path = std::env::var("DATABASE_PATH").unwrap_or(config.database.path);

    let conn = Connection::open(&db_path)
        .map_err(|e| anyhow::anyhow!("gagal membuka {}: {}", db_path, e))?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    crate::adapters::driven::sqlite_store::schema::init_schema(&conn)?;
    Ok(conn)
}

fn emit(as_json: bool, payload: serde_json::Value, human: String) {
    if as_json {
        match serde_json::to_string_pretty(&payload) {
            Ok(s) => println!("{}", s),
            Err(_) => println!("{}", payload),
        }
    } else {
        println!("{}", human);
    }
}

fn print_help() {
    println!(
        r#"Penggunaan `aina progress` (kabar progres manusiawi & kesadaran durasi tur aktif):

    aina progress status [--json]        Cek durasi berjalan sejak permintaan user & sisa cooldown
    aina progress --elapsed [--json]     Alias cepat untuk `aina progress status`
    aina progress "<pesan>" [--json]     Kirim update progres singkat (1-2 kalimat) ke chat aktif

Aturan bawaan (anti-spam deterministik):
    • Wajib berjalan dari dalam tur WhatsApp aktif (`AINA_TURN_TOKEN`).
    • Cooldown otomatis: 90 detik (DM) / 180 detik (Grup); maks 6 update (DM) / 2 update (Grup).
    • Panjang pesan dibatasi <= 450 karakter agar jawaban lengkap tetap di respons akhir.
    • Setiap update yang dikirim otomatis mereset timer fallback heartbeat.
"#
    );
}

#[derive(Default)]
struct ProgressFlags {
    json: bool,
    elapsed_only: bool,
    positional: Vec<String>,
}

impl ProgressFlags {
    fn parse(args: &[String]) -> Self {
        let mut f = Self::default();
        for a in args {
            match a.as_str() {
                "--json" | "-j" => f.json = true,
                "--elapsed" | "-e" => f.elapsed_only = true,
                other => f.positional.push(other.to_string()),
            }
        }
        f
    }
}
