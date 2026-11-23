use crate::core::domain::{
    evaluate_schedule, extract_schedule_creator, parse_interval_seconds, Authority, ScheduleAction,
    TurnCapability, DEFAULT_STAFF_MAX_ACTIVE_SCHEDULES,
};
use crate::core::ports::SessionStorePort;
use serde_json::json;
use std::io::IsTerminal;

pub async fn handle_schedule(args: &[String]) -> anyhow::Result<()> {
    if args.is_empty() || matches!(args[0].as_str(), "help" | "--help" | "-h") {
        println!(
            r#"Manajemen Tugas & Riset Terjadwal (Aina Scheduled Wake-up & Reminders):
    aina schedule list [--all] [--json]
    aina schedule get <id> [--json]
    aina schedule add --title <judul> --type <agent|notify> --target <jid> --when <once|daily|workdays|monthly|last_workday|last_workday_minus_1|interval> --time <waktu> --payload <isi> [--json]
    aina schedule delete|rm|remove <id> [--json]
    aina schedule runs [--limit <n>] [--json]
    aina schedule diag [--json]

Wewenang (deterministik via AINA_TURN_TOKEN; operator lokal gunakan `--operator`):
    • GUEST          : Tidak diizinkan membaca atau mengubah jadwal.
    • STAFF / ATASAN : Diizinkan memasang jadwal ke DM sendiri maupun grup WhatsApp (`@g.us`)
                       mana pun yang Aina tergabung di dalamnya (interval >= 30m,
                       maks 10 jadwal aktif per staf), dan menghapus jadwal buatannya sendiri.
    • ADMIN          : Akses penuh termasuk `status@broadcast`, DM pihak lain, dan jadwal sistem.

Pilihan --when & --time:
    once                 : Satu kali. Contoh --time "22:30", "+15m", "2026-10-30 08:00"
    daily                : Setiap hari. Contoh --time "08:00"
    workdays             : Hari kerja Senin–Jumat (skip weekend). Contoh --time "08:00"
    monthly              : Bulanan. Contoh --time "25 08:00", "last 08:00", "last_workday 08:00"
    last_workday         : Hari kerja terakhir bulan berjalan. Contoh --time "08:00"
    last_workday_minus_1 : H-1 hari kerja terakhir bulan berjalan. Contoh --time "08:00"
    interval             : Berkala. Contoh --time "1h", "30m", "604800"
"#
        );
        return Ok(());
    }

    let config_path =
        std::env::var("AINA_CONFIG").unwrap_or_else(|_| "config/config.yaml".to_string());
    let config = crate::config::AppConfig::load_from_file_or_default(&config_path);
    let db_path = std::env::var("DATABASE_PATH").unwrap_or(config.database.path);
    let store = crate::adapters::driven::SqliteSessionStore::new(&db_path)?;

    let subcmd = args[0].as_str();
    let as_json = args.iter().any(|a| a == "--json" || a == "-j");
    let operator_flag = args.iter().any(|a| a == "--operator" || a == "--local");
    let is_read_only = matches!(subcmd, "list" | "ls" | "get" | "runs" | "history" | "diag" | "diagnostics");

    // Fail-closed capability resolution:
    // - Agent subprocesses MUST present a valid AINA_TURN_TOKEN (prevents `env -u AINA_TURN_TOKEN` bypass).
    // - Human operators on an interactive TTY may use `--operator` (or read-only commands directly).
    let capability = match resolve_schedule_capability(&store, operator_flag, is_read_only).await {
        Ok(Some(c)) => c,
        Ok(None) => {
            emit_denied(
                as_json,
                subcmd,
                "missing_turn_capability",
                "Operasi scheduler hanya dapat dijalankan dari percakapan WhatsApp terverifikasi (minimal STAFF) atau terminal interaktif dengan `--operator`.",
            );
            return Ok(());
        }
        Err(e) => {
            emit_denied(as_json, subcmd, "capability_resolution_failed", &e.to_string());
            return Ok(());
        }
    };

    let now_epoch = chrono::Utc::now().timestamp();

    match subcmd {
        "list" | "ls" => {
            let decision = evaluate_schedule(&capability, &ScheduleAction::Read, now_epoch);
            if !decision.allowed {
                emit_denied(as_json, subcmd, &decision.reason, &human_denial_hint(&decision.reason));
                return Ok(());
            }

            let show_all = args.iter().any(|a| a == "--all");
            let tasks = store.list_scheduled_tasks(!show_all).await?;

            if as_json {
                println!("{}", serde_json::to_string_pretty(&tasks)?);
                return Ok(());
            }

            println!("⏰ Daftar Tugas & Riset Terjadwal (Aina Scheduler)");
            println!("================================================================================");
            if tasks.is_empty() {
                println!("(Tidak ada tugas terjadwal aktif)");
            } else {
                for t in &tasks {
                    let status_str = if t.is_active { "AKTIF" } else { "SELESAI / NON-AKTIF" };
                    let type_str = match t.task_type {
                        crate::core::domain::ScheduledTaskType::AgentAction => "Riset / Tindakan Agentik",
                        crate::core::domain::ScheduledTaskType::DirectNotification => "Pesan Pengingat Langsung",
                    };
                    println!("• [#ID: {}] {} ({})", t.id, t.title, status_str);
                    println!("  Tipe      : {}", type_str);
                    println!("  Jadwal    : {} ({})", t.schedule_type, t.schedule_expr);
                    println!("  Penerima  : {}", t.target_jid);
                    if let Some(creator) = extract_schedule_creator(t.metadata.as_deref()) {
                        println!("  Pembuat   : {}", creator);
                    }
                    println!("  Epoch Run : {}", t.next_run_epoch);
                    if let Some(ref st) = t.last_status {
                        let dur_str = t
                            .last_duration_secs
                            .map(|d| format!("{:.2}s", d))
                            .unwrap_or_else(|| "-".to_string());
                        println!("  Last Run  : Status: {} | Durasi: {}", st, dur_str);
                        if let Some(ref err) = t.last_error {
                            println!("  Last Err  : {}", err);
                        }
                    }
                    println!("  Isi/Tugas : {}", t.payload);
                    println!("--------------------------------------------------------------------------------");
                }
            }
            Ok(())
        }
        "get" => {
            let decision = evaluate_schedule(&capability, &ScheduleAction::Read, now_epoch);
            if !decision.allowed {
                emit_denied(as_json, subcmd, &decision.reason, &human_denial_hint(&decision.reason));
                return Ok(());
            }

            let Some(id_arg) = args.iter().skip(1).find(|a| !a.starts_with('-')) else {
                eprintln!("Format salah. Contoh: aina schedule get 1");
                std::process::exit(1);
            };
            let id: i64 = id_arg.parse()?;
            let task = store.get_scheduled_task(id).await?;

            match task {
                Some(t) => {
                    if as_json {
                        println!("{}", serde_json::to_string_pretty(&t)?);
                    } else {
                        println!("ID: {}", t.id);
                        println!("Title: {}", t.title);
                        println!("Type: {:?}", t.task_type);
                        println!("Target: {}", t.target_jid);
                        println!("Schedule: {} ({})", t.schedule_type, t.schedule_expr);
                        if let Some(creator) = extract_schedule_creator(t.metadata.as_deref()) {
                            println!("Created By: {}", creator);
                        }
                        println!("Next Run Epoch: {}", t.next_run_epoch);
                        println!("Payload: {}", t.payload);
                        println!("Active: {}", t.is_active);
                    }
                }
                None => {
                    eprintln!("Tugas terjadwal dengan ID #{} tidak ditemukan.", id);
                }
            }
            Ok(())
        }
        "add" => {
            let mut title = String::from("Pengingat");
            let mut task_type = crate::core::domain::ScheduledTaskType::DirectNotification;
            let mut target_jid = String::new();
            let mut payload = String::new();
            let mut schedule_type = String::from("once");
            let mut schedule_expr = String::new();

            let mut i = 1;
            while i < args.len() {
                match args[i].as_str() {
                    "--title" if i + 1 < args.len() => {
                        title = args[i + 1].clone();
                        i += 2;
                    }
                    "--type" if i + 1 < args.len() => {
                        task_type = crate::core::domain::ScheduledTaskType::from_str(&args[i + 1]);
                        i += 2;
                    }
                    "--target" if i + 1 < args.len() => {
                        target_jid = args[i + 1].clone();
                        i += 2;
                    }
                    "--when" if i + 1 < args.len() => {
                        schedule_type = args[i + 1].clone();
                        i += 2;
                    }
                    "--time" if i + 1 < args.len() => {
                        schedule_expr = args[i + 1].clone();
                        i += 2;
                    }
                    "--payload" if i + 1 < args.len() => {
                        payload = args[i + 1].clone();
                        i += 2;
                    }
                    _ => i += 1,
                }
            }

            if target_jid.trim().is_empty() && !capability.chat_jid.trim().is_empty() {
                target_jid = capability.chat_jid.clone();
            }

            if schedule_expr.is_empty() || payload.is_empty() {
                eprintln!(
                    "Parameter --time dan --payload wajib diisi. Ketik `aina schedule` untuk panduan."
                );
                std::process::exit(1);
            }

            let active_tasks_by_sender = if capability.authority != Authority::Admin {
                let active_tasks = store.list_scheduled_tasks(true).await.unwrap_or_default();
                active_tasks
                    .iter()
                    .filter(|t| {
                        extract_schedule_creator(t.metadata.as_deref())
                            .map(|c| {
                                crate::core::domain::capability::is_admin_identity(
                                    &c,
                                    &capability.principal_jid,
                                )
                            })
                            .unwrap_or(false)
                    })
                    .count()
            } else {
                0
            };

            let max_tasks_for_staff = std::env::var("AINA_STAFF_MAX_SCHEDULES")
                .ok()
                .and_then(|v| v.trim().parse::<usize>().ok())
                .filter(|&n| n >= 1)
                .unwrap_or(DEFAULT_STAFF_MAX_ACTIVE_SCHEDULES);

            let interval_secs = if schedule_type.eq_ignore_ascii_case("interval") {
                parse_interval_seconds(&schedule_expr)
            } else {
                None
            };

            let decision = evaluate_schedule(
                &capability,
                &ScheduleAction::Add {
                    target_jid: target_jid.clone(),
                    schedule_type: schedule_type.clone(),
                    interval_secs,
                    active_tasks_by_sender,
                    max_tasks_for_staff,
                },
                now_epoch,
            );

            if !decision.allowed {
                emit_denied(as_json, "add", &decision.reason, &human_denial_hint(&decision.reason));
                return Ok(());
            }

            let next_epoch = crate::core::domain::ScheduleParser::compute_next_run(
                &schedule_type,
                &schedule_expr,
                config.app.timezone_offset_hours,
                now_epoch,
            )?;

            let provenance_meta = json!({
                "created_by": capability.principal_jid,
                "created_by_authority": capability.authority.as_str(),
                "created_in_chat": capability.chat_jid,
            })
            .to_string();

            let new_task = crate::core::domain::NewScheduledTask {
                title: title.clone(),
                task_type,
                target_jid: target_jid.clone(),
                payload,
                schedule_type: schedule_type.clone(),
                schedule_expr: schedule_expr.clone(),
                next_run_epoch: next_epoch,
                slug: None,
                category: Some("custom".to_string()),
                source: Some("custom".to_string()),
                metadata: Some(provenance_meta),
            };

            let id = store.create_scheduled_task(&new_task).await?;
            if as_json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "ok": true,
                        "action": "add",
                        "id": id,
                        "title": title,
                        "task_type": task_type.as_str(),
                        "schedule_type": schedule_type,
                        "schedule_expr": schedule_expr,
                        "target_jid": target_jid,
                        "next_run_epoch": next_epoch,
                        "created_by": capability.principal_jid,
                        "authority": capability.authority.as_str(),
                    }))?
                );
            } else {
                println!("✅ Berhasil menambahkan tugas terjadwal #{} ('{}')", id, title);
                println!("   Tipe      : {:?}", task_type);
                println!("   Jadwal    : {} ({})", schedule_type, schedule_expr);
                println!("   Target WA : {}", target_jid);
                println!("   Pembuat   : {} ({})", capability.principal_jid, capability.authority.as_str());
                println!("   Next Epoch: {}", next_epoch);
            }
            Ok(())
        }
        "delete" | "rm" | "remove" => {
            let Some(id_arg) = args.iter().skip(1).find(|a| !a.starts_with('-')) else {
                eprintln!("Format salah. Contoh: aina schedule delete 1");
                std::process::exit(1);
            };
            let id: i64 = id_arg.parse()?;

            let Some(existing) = store.get_scheduled_task(id).await? else {
                if as_json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&json!({
                            "ok": false,
                            "action": "delete",
                            "id": id,
                            "reason": "not_found"
                        }))?
                    );
                } else {
                    println!("⚠️ Tugas terjadwal #{} tidak ditemukan.", id);
                }
                return Ok(());
            };

            let decision = evaluate_schedule(
                &capability,
                &ScheduleAction::Delete {
                    task_target_jid: existing.target_jid.clone(),
                    task_source: existing.source.clone(),
                    task_created_by: extract_schedule_creator(existing.metadata.as_deref()),
                },
                now_epoch,
            );
            if !decision.allowed {
                emit_denied(as_json, "delete", &decision.reason, &human_denial_hint(&decision.reason));
                return Ok(());
            }

            let deleted = store.delete_scheduled_task(id).await?;
            if as_json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "ok": deleted,
                        "action": "delete",
                        "id": id,
                        "deleted_by": capability.principal_jid,
                    }))?
                );
            } else if deleted {
                println!("✅ Tugas terjadwal #{} berhasil dihapus.", id);
            } else {
                println!("⚠️ Tugas terjadwal #{} tidak ditemukan.", id);
            }
            Ok(())
        }
        "runs" | "history" => {
            let decision = evaluate_schedule(&capability, &ScheduleAction::Read, now_epoch);
            if !decision.allowed {
                emit_denied(as_json, subcmd, &decision.reason, &human_denial_hint(&decision.reason));
                return Ok(());
            }

            let mut limit = 20;
            if let Some(pos) = args.iter().position(|a| a == "--limit") {
                if pos + 1 < args.len() {
                    limit = args[pos + 1].parse().unwrap_or(20);
                }
            }
            let runs = store.list_scheduled_task_runs(limit).await?;
            if as_json {
                println!("{}", serde_json::to_string_pretty(&runs)?);
                return Ok(());
            }

            println!("📜 Riwayat Eksekusi Scheduler (Aina Task Run History)");
            println!("================================================================================");
            if runs.is_empty() {
                println!("(Belum ada catatan eksekusi tugas terjadwal)");
            } else {
                for r in &runs {
                    let diff = now_epoch - r.executed_at_epoch;
                    let time_desc = if diff < 60 {
                        format!("{}s lalu", diff)
                    } else if diff < 3600 {
                        format!("{}m lalu", diff / 60)
                    } else {
                        format!("{}h {}m lalu", diff / 3600, (diff % 3600) / 60)
                    };
                    let status_badge = if r.status == "success" { "✅ SUKSES" } else { "❌ GAGAL" };
                    println!(
                        "• [#Run: {} | Task: #{}] {} — {}",
                        r.id, r.task_id, r.task_title, status_badge
                    );
                    println!(
                        "  Waktu     : {} (Epoch: {}) | Durasi: {:.2}s",
                        time_desc, r.executed_at_epoch, r.duration_secs
                    );
                    println!("  Target WA : {}", r.target_jid);
                    if let Some(ref err) = r.error_message {
                        println!("  Error/RCA : {}", err);
                    }
                    if let Some(ref prev) = r.output_preview {
                        let clean_prev = prev.trim().replace('\n', " ");
                        let truncated_prev = if clean_prev.len() > 100 {
                            format!("{}...", &clean_prev[..100])
                        } else {
                            clean_prev
                        };
                        println!("  Preview   : {}", truncated_prev);
                    }
                    println!("--------------------------------------------------------------------------------");
                }
            }
            Ok(())
        }
        "diag" | "diagnostics" => {
            let decision = evaluate_schedule(&capability, &ScheduleAction::Read, now_epoch);
            if !decision.allowed {
                emit_denied(as_json, subcmd, &decision.reason, &human_denial_hint(&decision.reason));
                return Ok(());
            }

            let diag = store.get_scheduler_diagnostics().await?;
            if as_json {
                println!("{}", serde_json::to_string_pretty(&diag)?);
                return Ok(());
            }

            println!("🩺 Diagnostik & Observabilitas Scheduler Aina");
            println!("================================================================================");
            println!("  Total Tugas Terdaftar   : {}", diag.total_tasks);
            println!("  Tugas Aktif             : {}", diag.active_tasks);
            println!("  Total Eksekusi (Runs)   : {}", diag.total_runs);
            let pct = if diag.total_runs > 0 {
                (diag.successful_runs as f64 / diag.total_runs as f64) * 100.0
            } else {
                0.0
            };
            println!("  Eksekusi Sukses         : {} ({:.1}%)", diag.successful_runs, pct);
            println!("  Eksekusi Gagal          : {}", diag.failed_runs);
            println!("--------------------------------------------------------------------------------");
            if let Some(ref last) = diag.last_run {
                let st = if last.status == "success" { "✅ Sukses" } else { "❌ Gagal" };
                println!("  Eksekusi Terakhir       : Task #{} '{}' -> {}", last.task_id, last.task_title, st);
                println!("    Durasi: {:.2}s | Epoch: {}", last.duration_secs, last.executed_at_epoch);
                if let Some(ref err) = last.error_message {
                    println!("    Detail Error: {}", err);
                }
            } else {
                println!("  Eksekusi Terakhir       : (Belum pernah dijalankan)");
            }
            if let Some(ref fail) = diag.last_failure {
                println!("  Kegagalan Terakhir      : Task #{} '{}'", fail.task_id, fail.task_title);
                if let Some(ref err) = fail.error_message {
                    println!("    RCA / Root Cause      : {}", err);
                }
            }
            if let Some(ref next) = diag.next_task {
                let diff = next.next_run_epoch - now_epoch;
                let countdown = if diff > 0 {
                    format!("dalam {}m {}s", diff / 60, diff % 60)
                } else {
                    format!("terlewat {}s", -diff)
                };
                println!("  Tugas Berikutnya        : Task #{} '{}'", next.id, next.title);
                println!(
                    "    Jadwal: {} ({}) | Eksekusi: {} (Epoch: {})",
                    next.schedule_type, next.schedule_expr, countdown, next.next_run_epoch
                );
            }
            println!("================================================================================");
            Ok(())
        }
        _ => {
            eprintln!(
                "Subcommand schedule tidak dikenal: `{}`. Pilihan: list, get, add, delete, remove, runs, diag.",
                subcmd
            );
            Ok(())
        }
    }
}

/// Resolves the caller's verified [`TurnCapability`] fail-closed:
/// 1. Interactive local human TTY with `--operator` (or read-only TTY inspection).
/// 2. Scoped `AINA_TURN_TOKEN` from SQLite `turn_capabilities`.
/// Non-interactive subprocesses without a valid token are ALWAYS rejected.
async fn resolve_schedule_capability(
    store: &crate::adapters::driven::SqliteSessionStore,
    operator_flag: bool,
    is_read_only: bool,
) -> anyhow::Result<Option<TurnCapability>> {
    let is_interactive_tty = (std::io::stdin().is_terminal() && std::io::stdout().is_terminal())
        && std::env::var("AINA_FORCE_NON_TERMINAL").is_err();

    if operator_flag {
        if !is_interactive_tty {
            anyhow::bail!(
                "mode --operator hanya tersedia pada sesi terminal interaktif (bukan proses non-interaktif/agen)"
            );
        }
        let now = chrono::Utc::now().timestamp();
        let mut cap = TurnCapability::mint(
            "local:operator",
            Authority::Admin,
            "",
            "direct",
            "operator_schedule",
            now,
        );
        cap.expires_at = now + 60;
        return Ok(Some(cap));
    }

    let token = std::env::var("AINA_TURN_TOKEN").unwrap_or_default();
    if !token.trim().is_empty() {
        return store.get_turn_capability(token.trim()).await;
    }

    // Convenience for local human operator checking `aina schedule list/diag` on a real TTY
    if is_read_only && is_interactive_tty {
        let now = chrono::Utc::now().timestamp();
        let mut cap = TurnCapability::mint(
            "local:operator",
            Authority::Admin,
            "",
            "direct",
            "operator_schedule_ro",
            now,
        );
        cap.expires_at = now + 60;
        return Ok(Some(cap));
    }

    Ok(None)
}

fn emit_denied(as_json: bool, action: &str, reason: &str, hint: &str) {
    if as_json {
        let payload = json!({
            "ok": false,
            "action": action,
            "reason": reason,
            "hint": hint,
        });
        println!("{}", serde_json::to_string_pretty(&payload).unwrap_or_else(|_| payload.to_string()));
    } else {
        println!("⛔ Aksi scheduler `{}` ditolak ({}).\n{}", action, reason, hint);
    }
}

fn human_denial_hint(reason: &str) -> String {
    match reason {
        "guest_may_not_use_scheduler" => {
            "Penjadwalan otomatis minimal memerlukan wewenang STAFF terverifikasi. Sampaikan dengan santun bahwa kamu perlu izin/verifikasi dari Admin terlebih dahulu.".to_string()
        }
        "only_admin_may_schedule_status_or_system" => {
            "Penjadwalan Status WhatsApp (`status@broadcast`) atau tugas internal sistem dikhususkan untuk Admin.".to_string()
        }
        "staff_may_not_schedule_other_user_dm" => {
            "Staf hanya dapat memasang jadwal DM untuk nomornya sendiri (bukan nomor pribadi orang lain).".to_string()
        }
        "invalid_group_jid" => {
            "Format JID grup tujuan tidak valid (contoh yang benar: `120363xxxx@g.us`).".to_string()
        }
        "interval_too_frequent_for_staff" => {
            "Untuk staf, interval pengulangan minimal 30 menit (atau gunakan `daily`/`workdays`/`monthly`) agar tidak memicu spam.".to_string()
        }
        "staff_schedule_quota_exceeded" => {
            "Kuota jadwal aktif untuk pengirim ini sudah mencapai batas maksimal. Hapus jadwal lama yang sudah tidak terpakai terlebih dahulu.".to_string()
        }
        "only_admin_may_delete_system_schedule" => {
            "Jadwal bawaan sistem (`baseline`) atau Status WhatsApp hanya boleh dihapus oleh Admin.".to_string()
        }
        "staff_may_only_delete_own_schedule" => {
            "Staf hanya dapat menghapus jadwal yang dibuat oleh dirinya sendiri. Untuk menghapus jadwal milik rekan lain atau Admin, koordinasikan dengan pembuatnya/Admin.".to_string()
        }
        "capability_expired" => {
            "Token tur percakapan sudah kedaluwarsa; minta pengguna mengirim pesan baru untuk memperbarui sesi.".to_string()
        }
        _ => format!("Permintaan tidak memenuhi kebijakan scheduler ({}).", reason),
    }
}
