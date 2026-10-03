use crate::core::domain::{PersonaLifecycleManager, VersionEngine};
use crate::core::ports::SessionStorePort;
use std::path::PathBuf;

pub async fn handle_model(args: &[String]) -> anyhow::Result<()> {
    if args.is_empty() {
        eprintln!("Operasi model belum ditentukan. Gunakan: get, list, set <model_name>");
        std::process::exit(1);
    }

    let sub = args[0].as_str();
    let port = std::env::var("SERVER_PORT")
        .or_else(|_| std::env::var("PORT"))
        .unwrap_or_else(|_| "8090".to_string());
    let base_url = format!("http://127.0.0.1:{}", port);
    let admin_key = std::env::var("ADMIN_KEY")
        .or_else(|_| std::env::var("AINA_ADMIN_KEY"))
        .unwrap_or_default();
    let is_json = args.iter().any(|a| a == "--json");
    let client = reqwest::Client::new();

    match sub {
        "get" => {
            let url = format!("{}/api/models", base_url);
            let mut req = client.get(&url);
            if !admin_key.is_empty() {
                req = req.header("X-Admin-Key", &admin_key).bearer_auth(&admin_key);
            }
            let res = req.send().await?;
            if !res.status().is_success() {
                let status = res.status();
                let body = res.text().await.unwrap_or_default();
                eprintln!("Gagal mendapatkan info model ({status}): {body}");
                std::process::exit(1);
            }
            let data: serde_json::Value = res.json().await?;
            if is_json {
                println!("{}", serde_json::to_string_pretty(&data)?);
            } else {
                let current = data.get("current").and_then(|v| v.as_str()).unwrap_or("unknown");
                println!("Model aktif saat ini: {}", current);
            }
        }
        "list" => {
            let url = format!("{}/api/models", base_url);
            let mut req = client.get(&url);
            if !admin_key.is_empty() {
                req = req.header("X-Admin-Key", &admin_key).bearer_auth(&admin_key);
            }
            let res = req.send().await?;
            if !res.status().is_success() {
                let status = res.status();
                let body = res.text().await.unwrap_or_default();
                eprintln!("Gagal mendapatkan daftar model ({status}): {body}");
                std::process::exit(1);
            }
            let data: serde_json::Value = res.json().await?;
            if is_json {
                println!("{}", serde_json::to_string_pretty(&data)?);
            } else {
                let current = data.get("current").and_then(|v| v.as_str()).unwrap_or("unknown");
                println!("Model Aktif: {}\n", current);
                println!("Daftar Model yang Didukung:");
                if let Some(arr) = data.get("available").and_then(|v| v.as_array()) {
                    for item in arr {
                        let id = item.get("id").and_then(|v| v.as_str()).unwrap_or("");
                        let name = item.get("name").and_then(|v| v.as_str()).unwrap_or("");
                        let marker = if id == current { " -> [AKTIF]" } else { "" };
                        println!("- {}: {}{}", id, name, marker);
                    }
                }
            }
        }
        "set" => {
            if args.len() < 2 {
                eprintln!("Error: Harap sebutkan nama model. Contoh: aina model set gemini-3.8-flash-high");
                std::process::exit(1);
            }
            let model_name = &args[1];
            let url = format!("{}/api/model", base_url);
            let payload = serde_json::json!({
                "model": model_name,
                "admin_key": admin_key
            });
            let mut req = client.post(&url).json(&payload);
            if !admin_key.is_empty() {
                req = req.header("X-Admin-Key", &admin_key).bearer_auth(&admin_key);
            }
            let res = req.send().await?;
            if !res.status().is_success() {
                let status = res.status();
                let body = res.text().await.unwrap_or_default();
                eprintln!("Gagal mengubah model ({status}): {body}");
                std::process::exit(1);
            }
            let data: serde_json::Value = res.json().await?;
            if is_json {
                println!("{}", serde_json::to_string_pretty(&data)?);
            } else {
                let active = data.get("model").and_then(|v| v.as_str()).unwrap_or(model_name);
                println!("Berhasil! Model aktif sekarang: {}", active);
            }
        }
        _ => {
            eprintln!("Operasi model tidak dikenal: `{}`. Gunakan: get, list, set <model_name>", sub);
            std::process::exit(1);
        }
    }

    Ok(())
}

pub async fn handle_version(args: &[String]) -> anyhow::Result<()> {
    let is_json = args.iter().any(|a| a == "--json");
    let is_check = args.iter().any(|a| a == "--check" || a == "-c" || a == "check");

    if is_check {
        let report = VersionEngine::check_upstream_status().await;
        if is_json {
            println!("{}", serde_json::to_string_pretty(&report)?);
        } else {
            println!("================================================================================");
            println!("Aina Self-Version & Upstream Introspection Report");
            println!("================================================================================");
            println!(
                "Versi Lokal:         v{} (Commit: {}, Branch: {})",
                report.current.version, report.current.commit_hash, report.current.git_branch
            );
            println!("Waktu Kompilasi:     {}", report.current.build_timestamp);
            println!("Repositori Resmi:    {}", report.current.repository);
            println!("Upstream Branch:     {}", report.upstream_branch);
            if let Some(ref up_commit) = report.upstream_latest_commit {
                println!("Upstream Commit:     {}", up_commit);
            }
            let status_icon = if report.is_up_to_date {
                "✓ UP-TO-DATE (Sesuai Upstream GitHub)"
            } else {
                "⚠️ UPDATE TERSEDIA DI GITHUB"
            };
            println!("Status Kelayakan:    {}", status_icon);
            println!();
            println!("Pesan Status:");
            println!("  {}", report.message);

            if !report.unpulled_commits.is_empty() {
                println!();
                println!("Commit Terbaru di Upstream (Belum Ditarik ke Container):");
                for c in &report.unpulled_commits {
                    println!("  • {} - {} ({}, {})", c.sha, c.message, c.author, c.date);
                }
                println!();
                println!("Tindakan Direkomendasikan:");
                println!("  Jalankan redeploy di Coolify (atau git pull & cargo build) agar fitur terbaru aktif.");
            }

            println!();
            println!("Kapabilitas Aktif pada Build Saat Ini:");
            for cap in &report.capabilities {
                println!("  [✓] {:<38} | {}", cap.name, cap.verification_hint);
            }
            println!("================================================================================");
        }
    } else {
        let info = VersionEngine::get_build_info();
        let caps = VersionEngine::get_capabilities();
        if is_json {
            let payload = serde_json::json!({
                "build": info,
                "capabilities": caps
            });
            println!("{}", serde_json::to_string_pretty(&payload)?);
        } else {
            println!("Aina AI Assistant v{}", info.version);
            println!("Commit:    {}", info.commit_hash);
            println!("Branch:    {}", info.git_branch);
            println!("Build:     {}", info.build_timestamp);
            println!("Repo:      {}", info.repository);
            println!();
            println!("Daftar Kapabilitas Aktif (Manifest):");
            for cap in caps {
                println!("  • {:<38} ({})", cap.name, cap.introduced_in);
                println!("    Verifikasi: {}", cap.verification_hint);
            }
            println!();
            println!("Gunakan `aina version --check` untuk memeriksa sinkronisasi dengan repo GitHub.");
        }
    }

    Ok(())
}

pub async fn handle_update(args: &[String]) -> anyhow::Result<()> {
    let mut check_args = vec!["--check".to_string()];
    check_args.extend_from_slice(args);
    handle_version(&check_args).await
}

pub async fn handle_persona(args: &[String]) -> anyhow::Result<()> {
    if args.is_empty() || args[0] == "help" || args[0] == "--help" || args[0] == "-h" {
        println!(
            r#"Manajemen Persona, Konfigurasi & Status WhatsApp Aina:

  Konfigurasi, Persistensi & Rollback:
    aina persona status [--json]                Periksa status file persona, ukuran & ketersediaan backup
    aina persona backup [target] [-m "alasan"]  Buat titik pemulihan (snapshot) sebelum melakukan perubahan
    aina persona history [target] [--json]      Daftar seluruh riwayat snapshot dan titik rollback
    aina persona rollback [target] [id]         Batalkan perubahan dan kembali ke snapshot sebelumnya
    aina persona reset [target]                 Kembalikan konfigurasi ke template bawaan pabrik (Default)
    aina persona set [target] "<isi_teks>"      Perbarui konfigurasi secara aman (otomatis membuat backup)

    Target yang didukung: persona (default), character, activities, organization

  Observabilitas & Status WhatsApp (Story):
    aina persona diag [--json]                  Diagnostik & observabilitas Persona Status Engine
    aina persona journal [--limit <n>]          Riwayat jurnal publikasi status WhatsApp
    aina persona check [--slot <slot>]          Periksa kuota & evaluasi posting slot waktu saat ini
    aina persona post [--slot <slot>]           Eksekusi pembuatan & publikasi status WhatsApp
    aina persona inspire                        Panduan konteks ruang imajinasi kreatif Aina
"#
        );
        return Ok(());
    }

    let subcmd = args[0].as_str();
    let is_json = args.iter().any(|a| a == "--json");

    // 1. Direct Rust Persona Configuration Lifecycle Subcommands
    match subcmd {
        "status" => {
            let mgr = PersonaLifecycleManager::from_env();
            return handle_persona_status(&mgr, is_json).await;
        }
        "backup" | "snapshot" => {
            let mgr = PersonaLifecycleManager::from_env();
            return handle_persona_backup(&mgr, &args[1..]).await;
        }
        "history" | "backups" | "snapshots" | "history-config" => {
            // If the user specified a config target or passed flags, or if subcmd is explicitly backups/snapshots
            if subcmd == "backups" || subcmd == "snapshots" || subcmd == "history-config" || (args.len() > 1 && !args[1].starts_with("--limit")) {
                let mgr = PersonaLifecycleManager::from_env();
                return handle_persona_history(&mgr, &args[1..], is_json).await;
            }
            // Otherwise, fall through to python persona_status.py history for WhatsApp status publication journal
        }
        "rollback" | "undo" | "revert" => {
            let mgr = PersonaLifecycleManager::from_env();
            return handle_persona_rollback(&mgr, &args[1..]).await;
        }
        "reset" | "restore-default" | "factory-reset" => {
            let mgr = PersonaLifecycleManager::from_env();
            return handle_persona_reset(&mgr, &args[1..]).await;
        }
        "set" | "update" => {
            let mgr = PersonaLifecycleManager::from_env();
            return handle_persona_set(&mgr, &args[1..]).await;
        }
        _ => {}
    }

    // 2. Python-based WhatsApp Status Publication Subcommands
    let script_candidates = [
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("scripts")
            .join("persona_status.py"),
        PathBuf::from("/app/scripts/persona_status.py"),
        PathBuf::from("/root/projects/aina/scripts/persona_status.py"),
    ];

    let resolved_script = script_candidates
        .into_iter()
        .find(|p| p.exists())
        .unwrap_or_else(|| PathBuf::from("scripts/persona_status.py"));

    if !resolved_script.exists() {
        anyhow::bail!(
            "Skrip persona_status.py tidak ditemukan di lingkungan saat ini."
        );
    }

    let mut cmd = tokio::process::Command::new("python3");
    cmd.arg(&resolved_script);
    match subcmd {
        "diag" | "diagnostics" => {
            cmd.arg("diag");
            for a in &args[1..] {
                cmd.arg(a);
            }
        }
        "journal" | "history" | "runs" => {
            cmd.arg("history");
            for a in &args[1..] {
                cmd.arg(a);
            }
        }
        "check" => {
            cmd.arg("check");
            for a in &args[1..] {
                cmd.arg(a);
            }
        }
        "inspire" => {
            cmd.arg("inspire");
            for a in &args[1..] {
                cmd.arg(a);
            }
        }
        "post" => {
            cmd.arg("post");
            for a in &args[1..] {
                cmd.arg(a);
            }
        }
        "generate" => {
            cmd.arg("generate");
            for a in &args[1..] {
                cmd.arg(a);
            }
        }
        _ => {
            eprintln!("Subcommand persona tidak dikenal: `{}`. Ketik `aina persona help`.", subcmd);
            return Ok(());
        }
    }

    let status = cmd.status().await?;
    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }

    Ok(())
}

async fn handle_persona_status(mgr: &PersonaLifecycleManager, is_json: bool) -> anyhow::Result<()> {
    let statuses = mgr.get_status()?;
    if is_json {
        println!("{}", serde_json::to_string_pretty(&statuses)?);
        return Ok(());
    }

    println!("🌸 Status Konfigurasi & Persistensi Persona Aina:");
    println!("Direktori Konfigurasi: {}", mgr.config_dir.display());
    println!("{:-<78}", "");
    println!("{:<18} | {:<8} | {:<10} | {:<10} | {:<22}", "Target", "Status", "Ukuran", "Snapshots", "Update Terakhir");
    println!("{:-<78}", "");
    for s in &statuses {
        let status_str = if s.exists { "Aktif" } else { "Kosong" };
        let size_str = if s.exists { format!("{} B", s.byte_size) } else { "-".to_string() };
        let default_badge = if s.default_template_exists { "✓ Default" } else { "✗" };
        let time_str = s.last_modified.as_deref().unwrap_or("-");
        println!(
            "{:<18} | {:<8} | {:<10} | {:<10} | {:<22}",
            s.target_file,
            status_str,
            size_str,
            format!("{} ({})", s.snapshot_count, default_badge),
            time_str
        );
    }
    println!("{:-<78}", "");
    println!("💡 Perintah Rollback & Restore:");
    println!("   • Buat snapshot   : `aina persona backup [target] [-m \"alasan\"]`");
    println!("   • Lihat riwayat   : `aina persona history [target]`");
    println!("   • Rollback versi  : `aina persona rollback [target] [id]`");
    println!("   • Reset ke pabrik : `aina persona reset [target]`");
    Ok(())
}

async fn handle_persona_backup(mgr: &PersonaLifecycleManager, args: &[String]) -> anyhow::Result<()> {
    let mut target = "persona";
    let mut reason = "Manual CLI snapshot".to_string();

    let mut i = 0;
    while i < args.len() {
        if (args[i] == "--reason" || args[i] == "-m") && i + 1 < args.len() {
            reason = args[i + 1].clone();
            i += 2;
        } else if !args[i].starts_with('-') {
            target = args[i].as_str();
            i += 1;
        } else {
            i += 1;
        }
    }

    let snap = mgr.create_snapshot(target, &reason)?;
    println!("✅ Snapshot konfigurasi berhasil dibuat!");
    println!("   • Target       : {}", snap.target_file);
    println!("   • Snapshot ID  : {}", snap.id);
    println!("   • Waktu        : {}", snap.created_at_formatted);
    println!("   • Ukuran       : {} byte ({} baris)", snap.byte_size, snap.line_count);
    println!("   • Keterangan   : {}", snap.reason);
    println!("   • Berkas Backup: history/{}", snap.backup_filename);
    println!("\n💡 Untuk membatalkan perubahan kapan saja: `aina persona rollback {} {}`", target, snap.id);
    Ok(())
}

async fn handle_persona_history(
    mgr: &PersonaLifecycleManager,
    args: &[String],
    is_json: bool,
) -> anyhow::Result<()> {
    let target = args.iter().find(|a| !a.starts_with('-')).map(|s| s.as_str()).unwrap_or("persona");
    let snapshots = mgr.list_snapshots(target)?;

    if is_json {
        println!("{}", serde_json::to_string_pretty(&snapshots)?);
        return Ok(());
    }

    println!("📜 Riwayat Snapshot & Titik Rollback ({}):", PersonaLifecycleManager::normalize_target(target));
    if snapshots.is_empty() {
        println!("   (Belum ada snapshot untuk {}. Buat dengan `aina persona backup {}`)", target, target);
        return Ok(());
    }

    println!("{:-<78}", "");
    println!("{:<18} | {:<22} | {:<8} | {}", "ID Snapshot", "Waktu Pembuatan", "Ukuran", "Keterangan");
    println!("{:-<78}", "");
    for s in &snapshots {
        println!(
            "{:<18} | {:<22} | {:<8} | {}",
            s.id,
            s.created_at_formatted,
            format!("{} B", s.byte_size),
            s.reason
        );
    }
    println!("{:-<78}", "");
    println!("💡 Untuk rollback ke salah satu versi: `aina persona rollback {} <ID>`", target);
    println!("💡 Untuk kembali ke versi sebelumnya secara instan: `aina persona rollback {}`", target);
    Ok(())
}

async fn handle_persona_rollback(mgr: &PersonaLifecycleManager, args: &[String]) -> anyhow::Result<()> {
    let mut target = "persona";
    let mut snapshot_id = None;

    for arg in args {
        if !arg.starts_with('-') {
            if target == "persona" && (arg == "character" || arg == "activities" || arg == "organization" || arg == "persona") {
                target = arg.as_str();
            } else if snapshot_id.is_none() {
                snapshot_id = Some(arg.as_str());
            }
        }
    }

    let outcome = mgr.rollback(target, snapshot_id)?;
    println!("🔄 Berhasil melakukan rollback konfigurasi!");
    println!("   • Target            : {}", outcome.target_file);
    println!("   • Dipulihkan Dari   : {}", outcome.restored_from_snapshot_id);
    println!("   • Ukuran Setelahnya : {} byte", outcome.restored_byte_size);
    println!("   • Safety Backup ID  : {} (Keadaan sebelum rollback disimpan aman)", outcome.safety_snapshot_id);
    println!("\n✨ Perubahan langsung aktif seketika pada giliran percakapan berikutnya (hot-reloaded)!");
    Ok(())
}

async fn handle_persona_reset(mgr: &PersonaLifecycleManager, args: &[String]) -> anyhow::Result<()> {
    let target = args.iter().find(|a| !a.starts_with('-')).map(|s| s.as_str()).unwrap_or("persona");

    let outcome = mgr.restore_default(target)?;
    println!("🏭 Konfigurasi berhasil direset ke template bawaan pabrik (Default)!");
    println!("   • Target            : {}", outcome.target_file);
    println!("   • Template Rujukan  : {}", outcome.template_path);
    println!("   • Ukuran Dipulihkan : {} byte", outcome.restored_byte_size);
    println!("   • Safety Backup ID  : {} (Keadaan sebelum reset tersimpan aman)", outcome.safety_snapshot_id);
    println!("\n✨ Perubahan langsung aktif seketika pada giliran percakapan berikutnya (hot-reloaded)!");
    Ok(())
}

async fn handle_persona_set(mgr: &PersonaLifecycleManager, args: &[String]) -> anyhow::Result<()> {
    if args.is_empty() {
        eprintln!("Penggunaan: aina persona set [target] \"<konten_baru>\" [-m \"alasan\"]");
        std::process::exit(1);
    }

    let mut target = "persona";
    let mut content = None;
    let mut reason = "Updated via CLI aina persona set".to_string();

    let mut i = 0;
    while i < args.len() {
        if (args[i] == "--reason" || args[i] == "-m") && i + 1 < args.len() {
            reason = args[i + 1].clone();
            i += 2;
        } else if content.is_none() && (args[i] == "persona" || args[i] == "character" || args[i] == "activities" || args[i] == "organization") {
            target = args[i].as_str();
            i += 1;
        } else if content.is_none() {
            content = Some(args[i].clone());
            i += 1;
        } else {
            i += 1;
        }
    }

    let new_content = match content {
        Some(c) => c,
        None => {
            eprintln!("Teks konten baru belum ditentukan. Contoh: `aina persona set \"Gaya baru...\"`");
            std::process::exit(1);
        }
    };

    let snap = mgr.set_content(target, &new_content, &reason)?;
    println!("✅ Konfigurasi {} berhasil diperbarui!", snap.target_file);
    println!("   • Pre-update Safety Snapshot ID: {}", snap.id);
    println!("   • Ukuran Baru                  : {} byte", new_content.len());
    println!("\n✨ Perubahan langsung aktif seketika pada giliran percakapan berikutnya (hot-reloaded)!");
    println!("💡 Untuk membatalkan kapan saja: `aina persona rollback {}`", target);
    Ok(())
}

pub async fn handle_metacognition(args: &[String]) -> anyhow::Result<()> {
    let subcmd = args.first().map(|s| s.as_str()).unwrap_or("diag");
    let is_json = args.iter().any(|a| a == "--json");
    let mut domain_filter: Option<String> = None;
    let mut limit: usize = 10;

    let mut idx = 1;
    while idx < args.len() {
        let arg = &args[idx];
        if arg == "--domain" && idx + 1 < args.len() {
            domain_filter = Some(args[idx + 1].clone());
            idx += 2;
            continue;
        }
        if (arg == "--limit" || arg == "-l") && idx + 1 < args.len() {
            limit = args[idx + 1].parse().unwrap_or(10);
            idx += 2;
            continue;
        }
        idx += 1;
    }

    let manifest = crate::core::domain::metacognition::AgentCapabilityManifest::default_manifest();
    let config_path = std::env::var("AINA_CONFIG").unwrap_or_else(|_| "config/config.yaml".to_string());
    let config = crate::config::AppConfig::load_from_file_or_default(&config_path);
    let db_path = std::env::var("DATABASE_PATH").unwrap_or(config.database.path);
    let store = crate::adapters::driven::SqliteSessionStore::new(&db_path)?;

    match subcmd {
        "capabilities" | "caps" => {
            if is_json {
                println!("{}", serde_json::to_string_pretty(&manifest)?);
            } else {
                println!("🧠 [MANIFEST KAPABILITAS & KONTRAK AGENT AINA]\n");
                println!("• Engine Model        : {} (Konteks: {} tokens, Vision: {}, Video: {})",
                    manifest.model_profile.model_name,
                    manifest.model_profile.context_window_tokens,
                    manifest.model_profile.supports_vision,
                    manifest.model_profile.supports_video_gen
                );
                println!("• Lingkungan Eksekusi : {} | Arsitektur: {}", manifest.environment.os_name, manifest.environment.architecture);
                println!("• Konteks Container   : {}", manifest.environment.container_context);
                println!("• Batas RAM Aman      : <{} MB (Hard Limit: {} MB)", manifest.environment.safe_ram_mb, manifest.environment.hard_limit_ram_mb);
                println!("• Domain Didukung     : {}", manifest.supported_domains.join(", "));
                println!("• Domain Tidak Support: {}", manifest.unsupported_domains.join(", "));
                println!("\n📦 KONTRAK TOOLS RESMI ({} Tools):", manifest.tool_contracts.len());
                for (name, tc) in &manifest.tool_contracts {
                    println!("  - `{}`: {}", name, tc.description);
                    println!("    • Intensitas: {} | Timeout: {}s | Multimodal: {}", tc.resource_intensity, tc.timeout_seconds, tc.supports_multimodal);
                    println!("    • Tag: {}", tc.capability_tags.join(", "));
                    if !tc.forbidden_patterns.is_empty() {
                        println!("    • Larangan: {}", tc.forbidden_patterns.join(" | "));
                    }
                }
                println!("\n🚫 OPERASI TERLARANG SISTEM:");
                for op in &manifest.environment.forbidden_operations {
                    println!("  ✖ {}", op);
                }
            }
            Ok(())
        }
        "calibration" | "calib" => {
            let stats = store.get_metacognitive_calibration_stats().await?;
            if is_json {
                println!("{}", serde_json::to_string_pretty(&stats)?);
            } else {
                println!("🎯 [KALIBRASI PREDIKSI & BRIER SCORE METAKOGNISI]\n");
                println!("• Total Prediksi         : {}", stats.total_predictions);
                println!("• Total Prediksi Selesai : {}", stats.resolved_predictions);
                println!("• Brier Score Rata-rata  : {:.4} (Makin mendekati 0.0000 makin sempurna)", stats.mean_brier_score);
                println!("• Base Rate Sukses       : {:.2}%", stats.base_rate * 100.0);
                println!("• Brier Skill Score (BSS): {:.4}", stats.brier_skill_score);
                println!("• Status Kalibrasi       : {}\n", stats.calibration_status);

                println!("📊 RELIABILITY DIAGRAM (5 BUCKETS):");
                println!("{:<16} {:<8} {:<16} {:<16}", "Rentang", "Jumlah", "Prediksi Rata2", "Frekuensi Riil");
                println!("{:-<60}", "");
                for b in &stats.reliability_buckets {
                    println!("[{:.1} - {:.1}]        {:<8} {:<16.2}% {:<16.2}%",
                        b.range_start,
                        b.range_end,
                        b.count,
                        b.mean_predicted * 100.0,
                        b.observed_frequency * 100.0
                    );
                }

                if !stats.domain_brier_scores.is_empty() {
                    println!("\n🌐 DOMAIN BRIER SCORES:");
                    for (dom, brier) in &stats.domain_brier_scores {
                        println!("  • {}: Brier Score {:.4}", dom, brier);
                    }
                }
            }
            Ok(())
        }
        "diag" | "diagnostics" | _ => {
            let stats = store.get_metacognitive_calibration_stats().await?;
            let recent = store.list_metacognitive_predictions(limit, domain_filter.as_deref()).await?;

            if is_json {
                let out = serde_json::json!({
                    "manifest": manifest,
                    "calibration": stats,
                    "recent_predictions": recent,
                });
                println!("{}", serde_json::to_string_pretty(&out)?);
            } else {
                println!("🧠 [DIAGNOSTIK METAKOGNISI & SELF-AWARENESS AINA]\n");
                println!("1. REPRESENTASI DIRI (R):");
                println!("   • Engine        : {}", manifest.model_profile.model_name);
                println!("   • Lingkungan    : {} ({})", manifest.environment.os_name, manifest.environment.architecture);
                println!("   • Status RAM    : Safe <{}MB / Hard {}MB", manifest.environment.safe_ram_mb, manifest.environment.hard_limit_ram_mb);
                println!("   • Tools Aktif   : {} tool terdaftar", manifest.tool_contracts.len());

                println!("\n2. KALIBRASI PROBABILISTIK & BRIER SCORE:");
                println!("   • Total Selesai : {}", stats.resolved_predictions);
                println!("   • Brier Score   : {:.4}", stats.mean_brier_score);
                println!("   • BSS           : {:.4} | Status: {}", stats.brier_skill_score, stats.calibration_status);

                println!("\n3. PREDIKSI TERAKHIR ({} entri):", recent.len());
                for p in &recent {
                    let status = match (p.actual_outcome, p.resolved_at_epoch) {
                        (Some(y), _) if y >= 0.5 => "✅ SUCCESS",
                        (Some(_), _) => "❌ FAILED",
                        (None, _) => "⏳ PENDING",
                    };
                    let dur = p.execution_duration_secs.map(|d| format!("{:.1}s", d)).unwrap_or_else(|| "-".to_string());
                    let bs = p.brier_score.map(|b| format!("{:.4}", b)).unwrap_or_else(|| "-".to_string());
                    println!("   • [{}] #{:<4} {} ({}) (P: {:.0}%, Dur: {}, BS: {})", status, p.id, p.prediction_id, p.domain_type.as_str(), p.predicted_probability * 100.0, dur, bs);
                }
            }
            Ok(())
        }
    }
}
