use crate::config::AppConfig;
use crate::core::domain::{
    load_default_baseline, Activity, ActivityCategory, ActivityReconciler, ActivitySource,
    NewScheduledTask, ScheduleParser, ScheduledTask, ScheduledTaskType,
};
use crate::core::ports::SessionStorePort;
use std::path::PathBuf;

pub async fn handle_activity(args: &[String]) -> anyhow::Result<()> {
    if args.is_empty() || args[0] == "help" || args[0] == "--help" || args[0] == "-h" {
        print_activity_help();
        return Ok(());
    }

    let config_path = std::env::var("AINA_CONFIG").unwrap_or_else(|_| "config/config.yaml".to_string());
    let config = AppConfig::load_from_file_or_default(&config_path);
    let db_path = std::env::var("DATABASE_PATH").unwrap_or_else(|_| config.database.path.clone());
    let store = crate::adapters::driven::SqliteSessionStore::new(&db_path)?;

    let subcmd = args[0].as_str();
    match subcmd {
        "list" | "ls" => handle_list(&store, &args[1..]).await,
        "get" | "show" | "info" => handle_get(&store, &args[1..]).await,
        "add" | "create" => handle_add(&store, &config, &args[1..]).await,
        "edit" | "update" => handle_edit(&store, &config, &args[1..]).await,
        "toggle" => handle_toggle(&store, &args[1..]).await,
        "enable" => handle_set_active(&store, &args[1..], true).await,
        "disable" => handle_set_active(&store, &args[1..], false).await,
        "reset" => handle_reset(&store, &args[1..]).await,
        "delete" | "rm" => handle_delete(&store, &args[1..]).await,
        "sync" => handle_sync(&store, &config, &args[1..]).await,
        "run" => handle_run(&store, &config, &args[1..]).await,
        _ => {
            eprintln!("Subcommand activity tidak dikenal: `{}`. Ketik `aina activity` untuk panduan.", subcmd);
            std::process::exit(1);
        }
    }
}

fn print_activity_help() {
    println!(
        r#"Aina Activity - Manajemen Baseline & Custom Activities Terjadwal

PENGGUNAAN:
    aina activity [SUBCOMMAND] [OPTIONS]
    aina act [SUBCOMMAND] [OPTIONS]

SUBCOMMANDS:
    list                      Daftar seluruh aktivitas (baseline & custom)
                              Options:
                                --all, -a            Tampilkan semua termasuk yang non-aktif
                                --baseline, -b       Hanya tampilkan baseline (built-in) activities
                                --custom, -c         Hanya tampilkan custom dynamic activities
                                --category <cat>     Filter berdasarkan kategori: status, briefing, reminder, maintenance, custom
                                --json               Output format JSON terstruktur

    get <slug|id>             Detail lengkap aktivitas terjadwal
                              Options:
                                --json               Output format JSON terstruktur

    add <slug> [options]      Tambah custom activity baru (persisten di DB & data/activities/)
                              Options:
                                --title, -t <text>   Judul / label aktivitas
                                --type <agent|notify> Tipe eksekusi (default: agent)
                                --target <jid>       Target WhatsApp (default: status@broadcast)
                                --when <type>        Tipe jadwal: daily, workdays, monthly, last_workday, interval, once (default: daily)
                                --time <expr>        Waktu eksekusi (misal: "07:00", "+15m", "1h")
                                --payload, -p <text> Prompt AI atau pesan notifikasi
                                --category <cat>     Kategori: status, briefing, reminder, maintenance, custom (default: custom)
                                --file               Simpan juga sebagai berkas data/activities/<slug>.json

    edit <slug|id> [options]  Ubah konfigurasi aktivitas
                              Options:
                                --title <text>       Ubah judul
                                --target <jid>       Ubah target WhatsApp
                                --when <type>        Ubah tipe jadwal
                                --time <expr>        Ubah ekspresi waktu
                                --payload <text>     Ganti seluruh prompt instruksi / pesan
                                --append, -a <text>  Tambahkan instruksi baru ke akhir prompt yang ada
                                --category <cat>     Ubah kategori aktivitas
                                --file               Simpan override ke berkas data/activities/<slug>.json

    toggle <slug|id>          Aktifkan / non-aktifkan aktivitas (flip state)
    enable <slug|id>          Aktifkan aktivitas secara eksplisit
    disable <slug|id>         Non-aktifkan aktivitas secara eksplisit
    reset <slug|id>           Reset aktivitas baseline ke konfigurasi bawaan pabrik

    delete <slug|id>          Hapus custom activity (jika baseline, reset ke default config)
                              Options:
                                --force, -f          Lewati peringatan

    sync                      Sinkronisasi baseline activities & berkas custom JSON ke scheduler
                              Options:
                                --json               Output ringkasan format JSON

    run <slug|id>             Jalankan aktivitas sekarang juga (uji coba langsung di terminal)
"#
    );
}

async fn resolve_task(store: &dyn SessionStorePort, id_or_slug: &str) -> anyhow::Result<Option<ScheduledTask>> {
    if let Ok(id) = id_or_slug.parse::<i64>() {
        if let Some(task) = store.get_scheduled_task(id).await? {
            return Ok(Some(task));
        }
    }
    store.get_scheduled_task_by_slug(id_or_slug).await
}

fn get_custom_activities_dir() -> PathBuf {
    let docker_path = PathBuf::from("/app/data/activities");
    if docker_path.exists() {
        return docker_path;
    }
    PathBuf::from("data/activities")
}

async fn handle_list(store: &dyn SessionStorePort, args: &[String]) -> anyhow::Result<()> {
    let as_json = args.iter().any(|a| a == "--json");
    let show_all = args.iter().any(|a| a == "--all" || a == "-a");
    let baseline_only = args.iter().any(|a| a == "--baseline" || a == "-b");
    let custom_only = args.iter().any(|a| a == "--custom" || a == "-c");

    let category_filter = args
        .windows(2)
        .find(|w| w[0] == "--category")
        .map(|w| w[1].to_lowercase());

    let tasks = store.list_scheduled_tasks(!show_all).await?;
    let mut activities: Vec<Activity> = tasks.into_iter().map(Activity::from).collect();

    if baseline_only {
        activities.retain(|a| a.source == ActivitySource::Baseline);
    } else if custom_only {
        activities.retain(|a| a.source == ActivitySource::Custom);
    }

    if let Some(ref cat) = category_filter {
        activities.retain(|a| a.category.as_str() == cat.as_str());
    }

    if as_json {
        println!("{}", serde_json::to_string_pretty(&activities)?);
        return Ok(());
    }

    println!("📋 Daftar Aktivitas Terjadwal (Aina Activities)");
    println!("================================================================================");
    if activities.is_empty() {
        println!("(Tidak ada aktivitas yang sesuai kriteria pencarian)");
    } else {
        for a in &activities {
            let status_str = if a.is_active { "AKTIF" } else { "NON-AKTIF" };
            let source_str = match a.source {
                ActivitySource::Baseline => "BASELINE (Built-in)",
                ActivitySource::Custom => "CUSTOM (User)",
            };
            let type_str = match a.task_type {
                ScheduledTaskType::AgentAction => "Riset / Tindakan Agentik",
                ScheduledTaskType::DirectNotification => "Pesan Pengingat Langsung",
            };

            let next_run_str = a.next_run_epoch
                .map(|e| {
                    if let Some(dt) = chrono::DateTime::from_timestamp(e, 0) {
                        dt.format("%Y-%m-%d %H:%M UTC").to_string()
                    } else {
                        format!("Epoch: {}", e)
                    }
                })
                .unwrap_or_else(|| "-".to_string());

            println!(
                "• [{}] {} (#ID: {}) - [{}]",
                a.slug,
                a.title,
                a.id.map(|i| i.to_string()).unwrap_or_else(|| "-".to_string()),
                status_str
            );
            println!("  Sumber    : {} | Kategori: {}", source_str, a.category.as_str());
            println!("  Tipe      : {}", type_str);
            println!("  Jadwal    : {} ({}) | Target WA: {}", a.schedule_type, a.schedule_expr, a.target_jid);
            println!("  Next Run  : {}", next_run_str);
            let payload_snippet = if a.payload.len() > 90 {
                format!("{}...", &a.payload[..87])
            } else {
                a.payload.clone()
            };
            println!("  Instruksi : {}", payload_snippet);
            println!("--------------------------------------------------------------------------------");
        }
    }

    Ok(())
}

async fn handle_get(store: &dyn SessionStorePort, args: &[String]) -> anyhow::Result<()> {
    if args.is_empty() {
        eprintln!("Parameter slug atau ID wajib diisi. Contoh: aina activity get status_pagi");
        std::process::exit(1);
    }

    let target = &args[0];
    let as_json = args.iter().any(|a| a == "--json");
    let task = resolve_task(store, target).await?;

    match task {
        Some(t) => {
            let act = Activity::from(t);
            if as_json {
                println!("{}", serde_json::to_string_pretty(&act)?);
            } else {
                println!("🔍 Detail Aktivitas: {}", act.slug);
                println!("================================================================================");
                println!("ID Database : #{}", act.id.unwrap_or(0));
                println!("Slug        : {}", act.slug);
                println!("Judul       : {}", act.title);
                println!("Kategori    : {}", act.category.as_str());
                println!("Sumber      : {:?}", act.source);
                println!("Status      : {}", if act.is_active { "AKTIF" } else { "NON-AKTIF" });
                println!("Tipe Aksi   : {:?}", act.task_type);
                println!("Target WA   : {}", act.target_jid);
                println!("Jadwal      : {} ({})", act.schedule_type, act.schedule_expr);
                println!("Next Epoch  : {:?}", act.next_run_epoch);
                println!("Last Epoch  : {:?}", act.last_run_epoch);
                if let Some(ref meta) = act.metadata {
                    println!("Metadata    : {}", serde_json::to_string(meta)?);
                }
                println!("Payload/Prompt:\n{}", act.payload);
                println!("================================================================================");
            }
        }
        None => {
            eprintln!("Aktivitas `{}` tidak ditemukan di sistem.", target);
            std::process::exit(1);
        }
    }

    Ok(())
}

async fn handle_add(
    store: &dyn SessionStorePort,
    config: &AppConfig,
    args: &[String],
) -> anyhow::Result<()> {
    if args.is_empty() || args[0].starts_with('-') {
        eprintln!("Slug aktivitas wajib diisi. Contoh: aina activity add daily_briefing --time 08:00 --payload '...'");
        std::process::exit(1);
    }

    let slug = args[0].trim().to_lowercase();
    if store.get_scheduled_task_by_slug(&slug).await?.is_some() {
        eprintln!("Aktivitas dengan slug `{}` sudah ada. Gunakan `aina activity edit {}` untuk mengubahnya.", slug, slug);
        std::process::exit(1);
    }

    let mut title = slug.replace('_', " ");
    let mut task_type = ScheduledTaskType::AgentAction;
    let mut target_jid = "status@broadcast".to_string();
    let mut schedule_type = "daily".to_string();
    let mut schedule_expr = String::new();
    let mut payload = String::new();
    let mut category = ActivityCategory::Custom;
    let mut save_file = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--title" | "-t" if i + 1 < args.len() => {
                title = args[i + 1].clone();
                i += 2;
            }
            "--type" if i + 1 < args.len() => {
                task_type = ScheduledTaskType::from_str(&args[i + 1]);
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
            "--payload" | "-p" if i + 1 < args.len() => {
                payload = args[i + 1].clone();
                i += 2;
            }
            "--category" if i + 1 < args.len() => {
                category = ActivityCategory::from_str(&args[i + 1]);
                i += 2;
            }
            "--file" => {
                save_file = true;
                i += 1;
            }
            _ => i += 1,
        }
    }

    if schedule_expr.is_empty() || payload.is_empty() {
        eprintln!("Parameter --time dan --payload wajib diisi.");
        std::process::exit(1);
    }

    let now_epoch = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let next_epoch = ScheduleParser::compute_next_run(
        &schedule_type,
        &schedule_expr,
        config.app.timezone_offset_hours,
        now_epoch,
    )?;

    let new_task = NewScheduledTask {
        title: title.clone(),
        task_type,
        target_jid: target_jid.clone(),
        payload: payload.clone(),
        schedule_type: schedule_type.clone(),
        schedule_expr: schedule_expr.clone(),
        next_run_epoch: next_epoch,
        slug: Some(slug.clone()),
        category: Some(category.as_str().to_string()),
        source: Some(ActivitySource::Custom.as_str().to_string()),
        metadata: None,
    };

    let id = store.create_scheduled_task(&new_task).await?;
    println!("✅ Berhasil menambahkan custom activity `{}` (#ID: {})", slug, id);
    println!("   Judul     : {}", title);
    println!("   Kategori  : {}", category.as_str());
    println!("   Jadwal    : {} ({})", schedule_type, schedule_expr);
    println!("   Target WA : {}", target_jid);
    println!("   Next Epoch: {}", next_epoch);

    if save_file {
        let dir = get_custom_activities_dir();
        let _ = std::fs::create_dir_all(&dir);
        let file_path = dir.join(format!("{}.json", slug));
        let act = Activity {
            id: Some(id),
            slug: slug.clone(),
            title,
            category,
            task_type,
            target_jid,
            schedule_type,
            schedule_expr,
            payload,
            source: ActivitySource::Custom,
            is_active: true,
            next_run_epoch: Some(next_epoch),
            last_run_epoch: None,
            metadata: None,
        };
        if let Ok(json) = serde_json::to_string_pretty(&act) {
            let _ = std::fs::write(&file_path, json);
            println!("   Berkas    : {} (tersimpan)", file_path.display());
        }
    }

    Ok(())
}

async fn handle_edit(
    store: &dyn SessionStorePort,
    config: &AppConfig,
    args: &[String],
) -> anyhow::Result<()> {
    if args.is_empty() {
        eprintln!("Format salah. Contoh: aina activity edit status_pagi --time 07:15");
        std::process::exit(1);
    }

    let target = &args[0];
    let task = resolve_task(store, target).await?.ok_or_else(|| {
        anyhow::anyhow!("Aktivitas `{}` tidak ditemukan.", target)
    })?;

    let mut title: Option<String> = None;
    let mut target_jid: Option<String> = None;
    let mut schedule_type: Option<String> = None;
    let mut schedule_expr: Option<String> = None;
    let mut payload: Option<String> = None;
    let mut append_payload: Option<String> = None;
    let mut category: Option<String> = None;
    let mut save_file = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--title" if i + 1 < args.len() => {
                title = Some(args[i + 1].clone());
                i += 2;
            }
            "--target" if i + 1 < args.len() => {
                target_jid = Some(args[i + 1].clone());
                i += 2;
            }
            "--when" if i + 1 < args.len() => {
                schedule_type = Some(args[i + 1].clone());
                i += 2;
            }
            "--time" if i + 1 < args.len() => {
                schedule_expr = Some(args[i + 1].clone());
                i += 2;
            }
            "--payload" if i + 1 < args.len() => {
                payload = Some(args[i + 1].clone());
                i += 2;
            }
            "--append" | "--append-payload" | "-a" if i + 1 < args.len() => {
                append_payload = Some(args[i + 1].clone());
                i += 2;
            }
            "--category" if i + 1 < args.len() => {
                category = Some(args[i + 1].clone());
                i += 2;
            }
            "--file" => {
                save_file = true;
                i += 1;
            }
            _ => i += 1,
        }
    }

    if let Some(ref add_text) = append_payload {
        let current_p = payload.as_deref().unwrap_or(&task.payload);
        payload = Some(format!("{}\n\n{}", current_p, add_text));
    }

    let next_run = if schedule_expr.is_some() || schedule_type.is_some() {
        let st = schedule_type.as_deref().unwrap_or(&task.schedule_type);
        let se = schedule_expr.as_deref().unwrap_or(&task.schedule_expr);
        let now_epoch = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        ScheduleParser::compute_next_run(st, se, config.app.timezone_offset_hours, now_epoch).ok()
    } else {
        Some(task.next_run_epoch)
    };

    let updated = store.update_scheduled_task(
        task.id,
        title.as_deref(),
        target_jid.as_deref(),
        payload.as_deref(),
        schedule_type.as_deref(),
        schedule_expr.as_deref(),
        next_run,
        category.as_deref(),
    ).await?;

    if updated {
        println!("✅ Aktivitas `{}` (#ID: {}) berhasil diperbarui.", target, task.id);
        if save_file {
            let slug = task.slug.as_deref().unwrap_or(target);
            let dir = get_custom_activities_dir();
            let _ = std::fs::create_dir_all(&dir);
            let file_path = dir.join(format!("{}.json", slug));
            let act = Activity {
                id: Some(task.id),
                slug: slug.to_string(),
                title: title.unwrap_or_else(|| task.title.clone()),
                category: category.as_deref().map(ActivityCategory::from_str).unwrap_or_else(|| {
                    task.category.as_deref().map(ActivityCategory::from_str).unwrap_or(ActivityCategory::Custom)
                }),
                task_type: task.task_type,
                target_jid: target_jid.unwrap_or_else(|| task.target_jid.clone()),
                schedule_type: schedule_type.unwrap_or_else(|| task.schedule_type.clone()),
                schedule_expr: schedule_expr.unwrap_or_else(|| task.schedule_expr.clone()),
                payload: payload.unwrap_or_else(|| task.payload.clone()),
                source: task.source.as_deref().map(ActivitySource::from_str).unwrap_or(ActivitySource::Baseline),
                is_active: task.is_active,
                next_run_epoch: next_run,
                last_run_epoch: task.last_run_epoch,
                metadata: task.metadata.as_deref().and_then(|s| serde_json::from_str(s).ok()),
            };
            if let Ok(json) = serde_json::to_string_pretty(&act) {
                let _ = std::fs::write(&file_path, json);
                println!("   Berkas override : {} (tersimpan di storage persisten)", file_path.display());
            }
        }
    } else {
        eprintln!("⚠️ Gagal memperbarui aktivitas `{}`.", target);
    }

    Ok(())
}

async fn handle_reset(store: &dyn SessionStorePort, args: &[String]) -> anyhow::Result<()> {
    if args.is_empty() {
        eprintln!("Format salah. Contoh: aina activity reset status_pagi");
        std::process::exit(1);
    }

    let target = &args[0];
    let task = resolve_task(store, target).await?.ok_or_else(|| {
        anyhow::anyhow!("Aktivitas `{}` tidak ditemukan.", target)
    })?;

    let slug = task.slug.as_deref().unwrap_or(target);
    let baseline = load_default_baseline();
    let def = baseline.iter().find(|b| b.slug == slug).ok_or_else(|| {
        anyhow::anyhow!("Aktivitas `{}` bukan merupakan Baseline Activity bawaan sistem.", slug)
    })?;

    let updated = store.update_scheduled_task(
        task.id,
        Some(&def.title),
        Some(&def.target_jid),
        Some(&def.payload),
        Some(&def.schedule_type),
        Some(&def.schedule_expr),
        Some(task.next_run_epoch),
        Some(def.category.as_str()),
    ).await?;

    if updated {
        println!("✅ Aktivitas baseline `{}` (#ID: {}) berhasil di-reset ke konfigurasi awal bawaan.", slug, task.id);
        let custom_file = get_custom_activities_dir().join(format!("{}.json", slug));
        if custom_file.exists() {
            let _ = std::fs::remove_file(&custom_file);
            println!("   Berkas override `{}` telah dihapus dari storage persisten.", custom_file.display());
        }
    } else {
        eprintln!("⚠️ Gagal me-reset aktivitas baseline `{}`.", slug);
    }

    Ok(())
}

async fn handle_toggle(store: &dyn SessionStorePort, args: &[String]) -> anyhow::Result<()> {
    if args.is_empty() {
        eprintln!("Format salah. Contoh: aina activity toggle status_pagi");
        std::process::exit(1);
    }

    let target = &args[0];
    let task = resolve_task(store, target).await?.ok_or_else(|| {
        anyhow::anyhow!("Aktivitas `{}` tidak ditemukan.", target)
    })?;

    let new_state = store.toggle_scheduled_task(task.id).await?;
    let state_str = if new_state { "AKTIF" } else { "NON-AKTIF" };
    println!("🔄 Status aktivitas `{}` (#ID: {}) diubah menjadi: {}", target, task.id, state_str);
    Ok(())
}

async fn handle_set_active(store: &dyn SessionStorePort, args: &[String], is_active: bool) -> anyhow::Result<()> {
    if args.is_empty() {
        eprintln!("Format salah. Contoh: aina activity enable status_pagi");
        std::process::exit(1);
    }

    let target = &args[0];
    let task = resolve_task(store, target).await?.ok_or_else(|| {
        anyhow::anyhow!("Aktivitas `{}` tidak ditemukan.", target)
    })?;

    store.set_scheduled_task_active(task.id, is_active).await?;
    let state_str = if is_active { "AKTIF" } else { "NON-AKTIF" };
    println!("✅ Aktivitas `{}` (#ID: {}) sekarang: {}", target, task.id, state_str);
    Ok(())
}

async fn handle_delete(store: &dyn SessionStorePort, args: &[String]) -> anyhow::Result<()> {
    if args.is_empty() {
        eprintln!("Format salah. Contoh: aina activity delete custom_task");
        std::process::exit(1);
    }

    let target = &args[0];
    let force = args.iter().any(|a| a == "--force" || a == "-f");

    let task = resolve_task(store, target).await?.ok_or_else(|| {
        anyhow::anyhow!("Aktivitas `{}` tidak ditemukan.", target)
    })?;

    let is_baseline = task.source.as_deref() == Some("baseline");
    if is_baseline && !force {
        eprintln!(
            "⚠️ PERHATIAN: `{}` adalah Baseline Activity bawaan sistem.\nJika Anda ingin menonaktifkannya tanpa menghapus, gunakan:\n   aina activity disable {}\nJika Anda yakin ingin menghapus dari database, gunakan opsi `--force`.",
            target, target
        );
        return Ok(());
    }

    let deleted = store.delete_scheduled_task(task.id).await?;
    if deleted {
        println!("✅ Aktivitas `{}` (#ID: {}) berhasil dihapus.", target, task.id);
        if let Some(ref slug) = task.slug {
            let custom_file = get_custom_activities_dir().join(format!("{}.json", slug));
            if custom_file.exists() {
                let _ = std::fs::remove_file(&custom_file);
                println!("   Berkas `{}` juga dihapus dari disk.", custom_file.display());
            }
        }
    } else {
        eprintln!("⚠️ Gagal menghapus aktivitas `{}`.", target);
    }

    Ok(())
}

async fn handle_sync(
    store: &dyn SessionStorePort,
    config: &AppConfig,
    args: &[String],
) -> anyhow::Result<()> {
    let as_json = args.iter().any(|a| a == "--json");
    let baseline = load_default_baseline();
    let custom_dir = get_custom_activities_dir();

    let now_epoch = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let report = ActivityReconciler::sync_all(
        store,
        &baseline,
        Some(&custom_dir),
        config.app.timezone_offset_hours,
        now_epoch,
    )
    .await?;

    if as_json {
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }

    println!("🔄 Hasil Rekonsiliasi & Sinkronisasi Aktivitas (Activity Sync)");
    println!("================================================================================");
    println!("Baseline Baru Dibuat    : {}", report.created_baseline);
    println!("Baseline Sudah Ada      : {} (status aktif & override dipertahankan)", report.existing_baseline);
    println!("Custom Baru Dibuat      : {}", report.created_custom);
    println!("Custom Diperbarui       : {}", report.updated_custom);
    if !report.errors.is_empty() {
        println!("Peringatan / Error      : {} item", report.errors.len());
        for err in &report.errors {
            println!("  - ⚠️ {}", err);
        }
    }
    println!("================================================================================");
    println!("✅ Sinkronisasi aktivitas selesai.");
    Ok(())
}

async fn handle_run(
    store: &dyn SessionStorePort,
    _config: &AppConfig,
    args: &[String],
) -> anyhow::Result<()> {
    if args.is_empty() {
        eprintln!("Format salah. Contoh: aina activity run status_pagi");
        std::process::exit(1);
    }

    let target = &args[0];
    let task = resolve_task(store, target).await?.ok_or_else(|| {
        anyhow::anyhow!("Aktivitas `{}` tidak ditemukan.", target)
    })?;

    println!("🚀 Menjalankan aktivitas instan: `{}` (#ID: {})", target, task.id);
    println!("Tipe Aksi : {:?}", task.task_type);
    println!("Target WA : {}", task.target_jid);
    println!("Payload   : {}", task.payload);
    println!("--------------------------------------------------------------------------------");

    match task.task_type {
        ScheduledTaskType::DirectNotification => {
            println!("🔔 [SIMULASI PENGIRIMAN PESAN LANGSUNG]");
            println!("Kirim ke: {}", task.target_jid);
            println!("Pesan:\n{}", task.payload);
            println!("✅ Eksekusi selesai.");
        }
        ScheduledTaskType::AgentAction => {
            println!("🤖 [EKSEKUSI TINDAKAN AGENTIK]");
            if task.payload.contains("scripts/persona_status.py") {
                let status = std::process::Command::new("python3")
                    .arg("scripts/persona_status.py")
                    .arg("--stdout-only")
                    .status();
                match status {
                    Ok(s) => {
                        println!("Status script exit code: {:?}", s.code());
                    }
                    Err(e) => {
                        println!("Gagal mengeksekusi python script: {}", e);
                    }
                }
            } else {
                println!("Menjalankan prompt instruksi: {}", task.payload);
            }
            println!("✅ Eksekusi selesai.");
        }
    }

    Ok(())
}
