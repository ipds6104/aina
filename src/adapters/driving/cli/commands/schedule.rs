use crate::core::ports::SessionStorePort;

pub async fn handle_schedule(args: &[String]) -> anyhow::Result<()> {
    if args.is_empty() {
        println!(
            r#"Manajemen Tugas & Riset Terjadwal (Aina Scheduled Wake-up & Reminders):
    aina schedule list [--all] [--json]
    aina schedule get <id> [--json]
    aina schedule add --title <judul> --type <agent|notify> --target <jid> --when <once|daily|interval> --time <waktu> --payload <isi>
    aina schedule delete <id>
    aina schedule runs [--limit <n>] [--json]
    aina schedule diag [--json]
"#
        );
        return Ok(());
    }

    let config_path = std::env::var("AINA_CONFIG").unwrap_or_else(|_| "config/config.yaml".to_string());
    let config = crate::config::AppConfig::load_from_file_or_default(&config_path);
    let db_path = std::env::var("DATABASE_PATH").unwrap_or(config.database.path);
    let store = crate::adapters::driven::SqliteSessionStore::new(&db_path)?;

    let subcmd = args[0].as_str();
    match subcmd {
        "list" | "ls" => {
            let as_json = args.iter().any(|a| a == "--json");
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
                    println!("  Epoch Run : {}", t.next_run_epoch);
                    if let Some(ref st) = t.last_status {
                        let dur_str = t.last_duration_secs.map(|d| format!("{:.2}s", d)).unwrap_or_else(|| "-".to_string());
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
            if args.len() < 2 {
                eprintln!("Format salah. Contoh: aina schedule get 1");
                std::process::exit(1);
            }
            let id: i64 = args[1].parse()?;
            let as_json = args.iter().any(|a| a == "--json");
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

            if schedule_expr.is_empty() || payload.is_empty() {
                eprintln!("Parameter --time dan --payload wajib diisi. Ketik `aina schedule` untuk panduan.");
                std::process::exit(1);
            }

            let now_epoch = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);

            let next_epoch = crate::core::domain::ScheduleParser::compute_next_run(
                &schedule_type,
                &schedule_expr,
                config.app.timezone_offset_hours,
                now_epoch,
            )?;

            let new_task = crate::core::domain::NewScheduledTask {
                title: title.clone(),
                task_type,
                target_jid: target_jid.clone(),
                payload,
                schedule_type: schedule_type.clone(),
                schedule_expr: schedule_expr.clone(),
                next_run_epoch: next_epoch,
            };

            let id = store.create_scheduled_task(&new_task).await?;
            println!("✅ Berhasil menambahkan tugas terjadwal #{} ('{}')", id, title);
            println!("   Tipe      : {:?}", task_type);
            println!("   Jadwal    : {} ({})", schedule_type, schedule_expr);
            println!("   Target WA : {}", target_jid);
            println!("   Next Epoch: {}", next_epoch);
            Ok(())
        }
        "delete" | "rm" => {
            if args.len() < 2 {
                eprintln!("Format salah. Contoh: aina schedule delete 1");
                std::process::exit(1);
            }
            let id: i64 = args[1].parse()?;
            let deleted = store.delete_scheduled_task(id).await?;
            if deleted {
                println!("✅ Tugas terjadwal #{} berhasil dihapus.", id);
            } else {
                println!("⚠️ Tugas terjadwal #{} tidak ditemukan.", id);
            }
            Ok(())
        }
        "runs" | "history" => {
            let as_json = args.iter().any(|a| a == "--json");
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
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0);
                for r in &runs {
                    let diff = now - r.executed_at_epoch;
                    let time_desc = if diff < 60 {
                        format!("{}s lalu", diff)
                    } else if diff < 3600 {
                        format!("{}m lalu", diff / 60)
                    } else {
                        format!("{}h {}m lalu", diff / 3600, (diff % 3600) / 60)
                    };
                    let status_badge = if r.status == "success" { "✅ SUKSES" } else { "❌ GAGAL" };
                    println!("• [#Run: {} | Task: #{}] {} — {}", r.id, r.task_id, r.task_title, status_badge);
                    println!("  Waktu     : {} (Epoch: {}) | Durasi: {:.2}s", time_desc, r.executed_at_epoch, r.duration_secs);
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
            let as_json = args.iter().any(|a| a == "--json");
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
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0);
                let diff = next.next_run_epoch - now;
                let countdown = if diff > 0 {
                    format!("dalam {}m {}s", diff / 60, diff % 60)
                } else {
                    format!("terlewat {}s", -diff)
                };
                println!("  Tugas Berikutnya        : Task #{} '{}'", next.id, next.title);
                println!("    Jadwal: {} ({}) | Eksekusi: {} (Epoch: {})", next.schedule_type, next.schedule_expr, countdown, next.next_run_epoch);
            }
            println!("================================================================================");
            Ok(())
        }
        _ => {
            eprintln!("Subcommand schedule tidak dikenal: `{}`. Pilihan: list, get, add, delete, runs, diag.", subcmd);
            Ok(())
        }
    }
}
