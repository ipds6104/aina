use crate::core::domain::knowledge::GhPollStatus;
use crate::core::domain::{ArchiveEngine, AuditEngine, KnowledgeEngine};
use std::path::{Path, PathBuf};

pub struct CliDispatcher;

impl CliDispatcher {
    pub fn print_help() {
        println!(
            r#"Aina CLI - High-Performance Agentic Utilities

PENGGUNAAN:
    aina [SUBCOMMAND] [OPTIONS]
    aina [server | daemon]    # Menjalankan daemon webhook Aina (default)

SUBCOMMANDS:
    archive search <query>    Pencarian instan riwayat obrolan (SQLite FTS5 BM25)
                              Options:
                                --workspace, -w <dir>   Path workspace (default: workspaces/default)
                                --limit, -l <n>         Batas hasil (default: 10)
                                --json                  Output format JSON terstruktur

    archive stats             Statistik arsip obrolan di workspace
                              Options:
                                --workspace, -w <dir>   Path workspace (default: workspaces/default)
                                --json                  Output format JSON terstruktur

    kb lint                   Pemeriksaan kepatuhan kriteria kerapian Knowledge Base
                              Options:
                                --workspace, -w <dir>   Path workspace (default: workspaces/default)
                                --auto-heal             Perbaiki struktur & kompilasi ulang otomatis
                                --json                  Output format JSON terstruktur

    kb groom                  Kompilasi ulang katalog `knowledge/index.md` deterministik
                              Options:
                                --workspace, -w <dir>   Path workspace (default: workspaces/default)

    kb schedule               Tampilkan ringkasan tenggat waktu (deadlines)
                              Options:
                                --workspace, -w <dir>   Path workspace (default: workspaces/default)

    audit                     Audit jejak eksekusi dan tindakan Antigravity CLI
                              Options:
                                --query, -q <text>      Pencarian kata kunci pada audit trail
                                --errors-only, -e       Hanya tampilkan langkah yang berstatus ERROR
                                --limit, -l <n>         Batas hasil (default: 20)
                                --json                  Output format JSON terstruktur

    workspace info            Informasi status & metadata workspace aktif
                              Options:
                                --workspace, -w <dir>   Path workspace (default: AGENT_WORKSPACE / default)
                                --json                  Output format JSON terstruktur

    workspace init <path>     Inisialisasi direktori workspace baru (di mana saja di disk)
                              Options:
                                --title, -t <nama>      Judul / deskripsi domain workspace

    workspace sync            Tarik perubahan, kompilasi index, commit, dan push ke Git
                              Options:
                                --workspace, -w <dir>   Path workspace
                                --message, -m <text>    Custom commit message
                                --json                  Output format JSON

    workspace link <git-url>  Hubungkan workspace ke Git origin remote (auto .gitignore)

    workspace gh-status       Periksa status autentikasi GitHub CLI (`gh`)

    workspace gh-device       Mulai login GitHub via Device Code OAuth (Non-Blocking)
                              Subcommand: start (default), poll

    workspace gh-login <pat>  Login ke GitHub CLI secara non-interaktif via Personal Access Token

    workspace gh-create <n>   Buat repositori GitHub baru secara instan via `gh`
                              Options:
                                --public                Buat repositori publik (default: private)

    clone <git-url> [path]    Clone repositori GitHub knowledge base yang sudah ada
                              (Otomatis sinkronisasi, lint, dan generate katalog index.md)

    sync                      Alias cepat untuk `workspace sync`

    link <git-url>            Alias cepat untuk `workspace link`

    gh-device                 Alias cepat untuk `workspace gh-device start`

    gh-poll                   Alias cepat untuk `workspace gh-device poll`

    gh-login <pat>            Alias cepat untuk `workspace gh-login`

    help, --help, -h          Tampilkan panduan ini
"#
        );
    }

    pub fn run(args: Vec<String>) -> anyhow::Result<()> {
        if args.len() < 2 {
            Self::print_help();
            return Ok(());
        }

        let cmd = args[1].as_str();
        match cmd {
            "help" | "--help" | "-h" => {
                Self::print_help();
                Ok(())
            }
            "archive" => Self::handle_archive(&args[2..]),
            "kb" => Self::handle_kb(&args[2..]),
            "workspace" | "ws" => Self::handle_workspace(&args[2..]),
            "clone" => Self::handle_workspace_clone(&args[2..]),
            "sync" => Self::handle_workspace_sync(&args[2..]),
            "link" => Self::handle_workspace_link(&args[2..]),
            "gh-device" => Self::handle_gh_device(&args[2..]),
            "gh-poll" => Self::handle_gh_device(&["poll".to_string()]),
            "gh-login" => Self::handle_gh_login(&args[2..]),
            "audit" => Self::handle_audit(&args[2..]),
            _ => {
                eprintln!("Subcommand tidak dikenal: `{}`. Ketik `aina help`.", cmd);
                std::process::exit(1);
            }
        }
    }

    fn resolve_workspace(args: &[String]) -> PathBuf {
        let mut idx = 0;
        while idx < args.len() {
            if (args[idx] == "--workspace" || args[idx] == "-w") && idx + 1 < args.len() {
                let val = &args[idx + 1];
                let p = PathBuf::from(val);
                if p.is_dir() {
                    return p;
                }
                // Check if relative to AINA_WORKSPACES_DIR
                if let Ok(root_env) = std::env::var("AINA_WORKSPACES_DIR") {
                    let cand = Path::new(&root_env).join(val);
                    if cand.is_dir() {
                        return cand;
                    }
                }
                // Check if relative to workspaces/
                let candidate = Path::new("workspaces").join(val);
                if candidate.is_dir() {
                    return candidate;
                }
                return p;
            }
            idx += 1;
        }

        // 1. Check AGENT_WORKSPACE / AINA_WORKSPACE environment variable
        for env_key in &["AGENT_WORKSPACE", "AINA_WORKSPACE"] {
            if let Ok(val) = std::env::var(env_key) {
                if !val.trim().is_empty() {
                    let p = PathBuf::from(val);
                    if p.is_dir() {
                        return p;
                    }
                }
            }
        }

        // 2. Check AINA_WORKSPACES_DIR/default
        if let Ok(root_env) = std::env::var("AINA_WORKSPACES_DIR") {
            if !root_env.trim().is_empty() {
                let p = Path::new(&root_env).join("default");
                if p.is_dir() {
                    return p;
                }
            }
        }

        // 3. Check if current working directory (CWD) is a workspace (has knowledge/)
        let cwd = PathBuf::from(".");
        if cwd.join("knowledge").is_dir() {
            return cwd;
        }

        // 4. Check repo's workspaces/default
        if Path::new("workspaces/default").is_dir() {
            return PathBuf::from("workspaces/default");
        }

        PathBuf::from("workspaces/default")
    }

    fn handle_archive(args: &[String]) -> anyhow::Result<()> {
        if args.is_empty() {
            eprintln!("Operasi archive belum ditentukan. Gunakan: search, stats");
            std::process::exit(1);
        }

        let sub = args[0].as_str();
        let ws = Self::resolve_workspace(args);
        let is_json = args.iter().any(|a| a == "--json");

        match sub {
            "search" => {
                let mut query = None;
                let mut limit = 10;

                let mut idx = 1;
                while idx < args.len() {
                    let arg = &args[idx];
                    if (arg == "--limit" || arg == "-l") && idx + 1 < args.len() {
                        limit = args[idx + 1].parse().unwrap_or(10);
                        idx += 2;
                        continue;
                    }
                    if (arg == "--workspace" || arg == "-w") && idx + 1 < args.len() {
                        idx += 2;
                        continue;
                    }
                    if arg == "--json" {
                        idx += 1;
                        continue;
                    }
                    if query.is_none() {
                        query = Some(arg.as_str());
                    }
                    idx += 1;
                }

                let q = match query {
                    Some(q) => q,
                    None => {
                        eprintln!("Error: Kata kunci pencarian wajib disertakan.");
                        std::process::exit(1);
                    }
                };

                let results = ArchiveEngine::search_all(&ws, q, limit)?;

                if is_json {
                    println!("{}", serde_json::to_string_pretty(&results)?);
                } else {
                    println!(
                        "🔍 Hasil Pencarian Arsip [{}]: {} ditemukan (limit: {})\n",
                        q,
                        results.len(),
                        limit
                    );
                    if results.is_empty() {
                        println!("(Tidak ada percakapan yang cocok)");
                    } else {
                        for (i, r) in results.iter().enumerate() {
                            println!(
                                "{}. [{}] {} (Arsip: {}):",
                                i + 1,
                                r.timestamp,
                                r.sender,
                                r.archive_name
                            );
                            println!("   \"{}\"\n", r.message);
                        }
                    }
                }
                Ok(())
            }
            "stats" => {
                let stats = ArchiveEngine::get_all_stats(&ws)?;
                if is_json {
                    println!("{}", serde_json::to_string_pretty(&stats)?);
                } else {
                    println!("📊 Statistik Arsip Obrolan (Workspace: {:?}):\n", ws);
                    if stats.is_empty() {
                        println!("(Belum ada database arsip `.db` di knowledge/archives/)");
                    } else {
                        for st in stats {
                            let size_kb = (st.file_size_bytes as f64) / 1024.0;
                            println!("• Arsip: {}", st.archive_name);
                            println!("  Path: {}", st.db_path);
                            println!("  Total Pesan: {}", st.total_messages);
                            println!("  Total Partisipan: {}", st.total_participants);
                            println!(
                                "  Rentang Tanggal: {} s.d. {}",
                                st.earliest_date.as_deref().unwrap_or("-"),
                                st.latest_date.as_deref().unwrap_or("-")
                            );
                            println!("  Ukuran Berkas: {:.1} KB\n", size_kb);
                        }
                    }
                }
                Ok(())
            }
            _ => {
                eprintln!("Subcommand archive tidak dikenal: `{}`", sub);
                std::process::exit(1);
            }
        }
    }

    fn handle_kb(args: &[String]) -> anyhow::Result<()> {
        if args.is_empty() {
            eprintln!("Operasi kb belum ditentukan. Gunakan: lint, groom, schedule");
            std::process::exit(1);
        }

        let sub = args[0].as_str();
        let ws = Self::resolve_workspace(args);
        let is_json = args.iter().any(|a| a == "--json");
        let auto_heal = args.iter().any(|a| a == "--auto-heal");

        match sub {
            "lint" => {
                let report = KnowledgeEngine::lint(&ws, auto_heal);
                if is_json {
                    println!("{}", serde_json::to_string_pretty(&report)?);
                } else {
                    println!("📋 Laporan Kerapian Knowledge Base (Workspace: {:?}):\n", ws);
                    if report.is_clean {
                        println!("✅ KNOWLEDGE BASE BERSIH & RAPI! (0 Pelanggaran)");
                        if report.auto_healed {
                            println!("   (Telah dilakukan perbaikan dan kompilasi ulang otomatis)");
                        }
                    } else {
                        println!("⚠️ DITEMUKAN {} PELANGGARAN KERAPIAN:\n", report.violations_count);
                        for (i, v) in report.violations.iter().enumerate() {
                            println!("{}. [{}] {}:", i + 1, v.severity, v.rule);
                            println!("   Berkas: {}", v.file);
                            println!("   Pesan:  {}\n", v.message);
                        }
                        println!("Tips: Jalankan `aina kb lint --auto-heal` untuk perbaikan otomatis.");
                    }
                }
                if !report.is_clean {
                    std::process::exit(1);
                }
                Ok(())
            }
            "groom" => {
                println!("🧹 Mengompilasi katalog knowledge base untuk {:?}...", ws);
                let catalog = KnowledgeEngine::groom_knowledge_base(&ws)?;
                println!("✅ Katalog `knowledge/index.md` berhasil diperbarui ({} karakter).", catalog.len());
                Ok(())
            }
            "schedule" => {
                let activities = KnowledgeEngine::scan_activities(&ws);
                let mut with_deadlines: Vec<_> = activities
                    .into_iter()
                    .filter(|a| a.deadline.is_some() && a.status != "completed" && a.status != "archived")
                    .collect();
                with_deadlines.sort_by(|a, b| a.deadline.cmp(&b.deadline));

                if is_json {
                    println!("{}", serde_json::to_string_pretty(&with_deadlines)?);
                } else {
                    println!("📅 Ringkasan Jadwal & Tenggat Waktu (Workspace: {:?}):\n", ws);
                    if with_deadlines.is_empty() {
                        println!("(Tidak ada tenggat waktu aktif)");
                    } else {
                        for (i, act) in with_deadlines.iter().enumerate() {
                            println!(
                                "{}. [{}] {} - PIC: {} (Periode: {})",
                                i + 1,
                                act.deadline.as_deref().unwrap_or("-"),
                                act.nama,
                                act.pic.as_deref().unwrap_or("-"),
                                act.periode
                            );
                        }
                    }
                }
                Ok(())
            }
            _ => {
                eprintln!("Subcommand kb tidak dikenal: `{}`", sub);
                std::process::exit(1);
            }
        }
    }

    fn handle_audit(args: &[String]) -> anyhow::Result<()> {
        let mut query = None;
        let mut errors_only = false;
        let mut limit = 20;
        let is_json = args.iter().any(|a| a == "--json");

        let mut idx = 0;
        while idx < args.len() {
            let arg = &args[idx];
            if (arg == "--query" || arg == "-q") && idx + 1 < args.len() {
                query = Some(args[idx + 1].clone());
                idx += 2;
                continue;
            }
            if arg == "--errors-only" || arg == "-e" {
                errors_only = true;
                idx += 1;
                continue;
            }
            if (arg == "--limit" || arg == "-l") && idx + 1 < args.len() {
                limit = args[idx + 1].parse().unwrap_or(20);
                idx += 2;
                continue;
            }
            idx += 1;
        }

        let brain_path = PathBuf::from(std::env::var("APP_DATA_DIR").unwrap_or_else(|_| {
            "/root/.gemini/antigravity-cli/brain".to_string()
        }));

        let steps = AuditEngine::query_audit_trail(&brain_path, query.as_deref(), errors_only, limit);

        if is_json {
            println!("{}", serde_json::to_string_pretty(&steps)?);
        } else {
            println!(
                "🕵️ Audit Trail Antigravity: {} entri (errors_only: {}, limit: {})\n",
                steps.len(),
                errors_only,
                limit
            );
            if steps.is_empty() {
                println!("(Tidak ada entri yang cocok)");
            } else {
                for (i, s) in steps.iter().enumerate() {
                    let ts = s.created_at.as_deref().unwrap_or("-");
                    let st = s.status.as_deref().unwrap_or("OK");
                    println!(
                        "{}. [{}] [{}] Sesi: {}...",
                        i + 1,
                        ts,
                        st,
                        &s.session_id[..s.session_id.len().min(8)]
                    );
                    println!("   Tindakan: {}", s.summary);
                }
            }
        }
        Ok(())
    }

    fn handle_workspace(args: &[String]) -> anyhow::Result<()> {
        if args.is_empty() {
            eprintln!("Operasi workspace belum ditentukan. Gunakan: info, init, clone");
            std::process::exit(1);
        }

        let sub = args[0].as_str();
        let is_json = args.iter().any(|a| a == "--json");

        match sub {
            "info" => {
                let ws = Self::resolve_workspace(args);
                let info = KnowledgeEngine::get_workspace_info(&ws);
                if is_json {
                    println!("{}", serde_json::to_string_pretty(&info)?);
                } else {
                    println!("🏠 Informasi Workspace Aktif:\n");
                    println!("• Path Relatif     : {}", info.path);
                    println!("• Path Absolut     : {}", info.absolute_path);
                    println!("• Repositori Git   : {}", if info.is_git_repo { "Ya" } else { "Tidak (Lokal Disk)" });
                    if let Some(ref remote) = info.git_remote {
                        println!("• Git Remote URL   : {}", remote);
                    }
                    println!("• Dokumen Universal: {} berkas", info.universal_docs_count);
                    println!("• Kegiatan / Proyek: {} entri", info.activities_count);
                    println!("• Arsip Chat SQLite: {} basis data", info.archives_count);
                    println!("• Status Katalog   : {}", if info.has_index { "Tersedia (index.md)" } else { "Belum dibuat (Jalankan aina kb groom)" });
                    println!("• Kebersihan / Lint: {}\n", if info.is_clean { "✅ Bersih & Rapi" } else { "⚠️ Perlu dirapikan (Jalankan aina kb lint --auto-heal)" });
                }
                Ok(())
            }
            "init" => {
                if args.len() < 2 {
                    eprintln!("Error: Path target direktori workspace wajib disertakan.");
                    eprintln!("Contoh: aina workspace init /var/lib/aina/workspaces/kantor-bps");
                    std::process::exit(1);
                }
                let target_path = Path::new(&args[1]);
                let mut title = None;
                let mut idx = 2;
                while idx < args.len() {
                    if (args[idx] == "--title" || args[idx] == "-t") && idx + 1 < args.len() {
                        title = Some(args[idx + 1].as_str());
                        idx += 2;
                        continue;
                    }
                    idx += 1;
                }

                println!("🚀 Menginisialisasi workspace baru di {:?}...", target_path);
                KnowledgeEngine::init_workspace(target_path, title)?;
                println!("✅ Berhasil! Workspace siap digunakan.");
                println!("   Untuk mengaktifkannya, set environment variable:");
                println!("   export AGENT_WORKSPACE={}", target_path.display());
                Ok(())
            }
            "clone" => Self::handle_workspace_clone(&args[1..]),
            "sync" => Self::handle_workspace_sync(&args[1..]),
            "link" => Self::handle_workspace_link(&args[1..]),
            "gh-status" => Self::handle_gh_status(),
            "gh-login" | "login" => Self::handle_gh_login(&args[1..]),
            "gh-device" | "device" => Self::handle_gh_device(&args[1..]),
            "gh-create" => Self::handle_gh_create(&args[1..]),
            _ => {
                eprintln!("Subcommand workspace tidak dikenal: `{}`", sub);
                std::process::exit(1);
            }
        }
    }

    fn handle_workspace_clone(args: &[String]) -> anyhow::Result<()> {
        if args.is_empty() {
            eprintln!("Error: URL git repository wajib disertakan.");
            eprintln!("Contoh: aina clone https://github.com/my-org/knowledge-base.git [target-dir]");
            std::process::exit(1);
        }

        let git_url = &args[0];
        let default_target = format!(
            "workspaces/{}",
            git_url
                .trim_end_matches('/')
                .trim_end_matches(".git")
                .split('/')
                .last()
                .unwrap_or("cloned-kb")
        );
        let target_str = if args.len() > 1 && !args[1].starts_with('-') {
            &args[1]
        } else {
            &default_target
        };
        let target_path = Path::new(target_str);

        println!("📦 Meng-clone knowledge base dari {} ke {:?}...", git_url, target_path);
        KnowledgeEngine::clone_workspace(git_url, target_path)?;
        println!("✅ Clone selesai dan katalog `knowledge/index.md` otomatis digenerate!");
        println!("\nUntuk menghubungkan ke Aina daemon, tambahkan ke file `.env`:");
        println!("AGENT_WORKSPACE={}", target_path.display());
        Ok(())
    }

    fn handle_workspace_sync(args: &[String]) -> anyhow::Result<()> {
        let ws = Self::resolve_workspace(args);
        let mut msg = None;
        let mut idx = 0;
        let is_json = args.iter().any(|a| a == "--json");

        while idx < args.len() {
            if (args[idx] == "--message" || args[idx] == "-m") && idx + 1 < args.len() {
                msg = Some(args[idx + 1].as_str());
                idx += 2;
                continue;
            }
            idx += 1;
        }

        println!("🔄 Mensinkronisasikan workspace {:?} dengan remote Git...", ws);
        let result = KnowledgeEngine::sync_workspace(&ws, msg)?;

        if is_json {
            println!("{}", serde_json::to_string_pretty(&result)?);
        } else {
            println!("\n✅ SINKRONISASI BERHASIL!");
            println!("• Pull Status  : {}", if result.pulled { "Berhasil (Up to date / rebased)" } else { "Gagal / Konflik" });
            if !result.pull_summary.is_empty() {
                println!("  Output       : {}", result.pull_summary.lines().next().unwrap_or(""));
            }
            println!("• Commit Status: {}", if result.committed { format!("Tersimpan ({})", result.commit_message.as_deref().unwrap_or("-")) } else { "Tidak ada perubahan baru".to_string() });
            println!("• Push Status  : {}", if result.pushed { "Berhasil dikirim ke origin" } else { "Tidak dikirim / gagal" });
            if !result.push_summary.is_empty() {
                println!("  Output       : {}", result.push_summary.lines().next().unwrap_or(""));
            }
        }
        Ok(())
    }

    fn handle_workspace_link(args: &[String]) -> anyhow::Result<()> {
        if args.is_empty() {
            eprintln!("Error: URL git repository wajib disertakan.");
            eprintln!("Contoh: aina workspace link https://github.com/my-org/knowledge-base.git [--workspace <dir>]");
            std::process::exit(1);
        }
        let git_url = &args[0];
        let ws = Self::resolve_workspace(&args[1..]);

        println!("🔗 Menghubungkan workspace {:?} ke remote Git: {}...", ws, git_url);
        KnowledgeEngine::link_workspace(&ws, git_url)?;
        println!("✅ Berhasil! Workspace telah terhubung ke Git origin.");
        println!("   Jalankan `aina sync` untuk sinkronisasi otomatis.");
        Ok(())
    }

    fn handle_gh_status() -> anyhow::Result<()> {
        println!("🐙 Memeriksa status autentikasi GitHub CLI (`gh`)...");
        match KnowledgeEngine::check_gh_status() {
            Ok(status) => {
                println!("✅ GitHub CLI aktif dan terhubung:\n");
                println!("{}\n", status);
            }
            Err(e) => {
                eprintln!("⚠️ {}", e);
            }
        }
        Ok(())
    }

    fn handle_gh_create(args: &[String]) -> anyhow::Result<()> {
        if args.is_empty() {
            eprintln!("Error: Nama repositori GitHub wajib disertakan.");
            eprintln!("Contoh: aina workspace gh-create my-org/knowledge-base [--private]");
            std::process::exit(1);
        }

        let repo_name = &args[0];
        let ws = Self::resolve_workspace(&args[1..]);
        let private = !args.iter().any(|a| a == "--public");

        println!("🐙 Membuat repositori GitHub `{}` via gh CLI...", repo_name);
        match KnowledgeEngine::create_github_repo(&ws, repo_name, private) {
            Ok(res) => {
                println!("🎉 Repositori GitHub berhasil dibuat dan di-push:\n");
                println!("{}\n", res);
                println!("Workspace {:?} kini terhubung ke GitHub!", ws);
            }
            Err(e) => {
                eprintln!("❌ {}", e);
                std::process::exit(1);
            }
        }
        Ok(())
    }

    fn handle_gh_login(args: &[String]) -> anyhow::Result<()> {
        if args.is_empty() {
            eprintln!("Error: Personal Access Token (PAT) wajib disertakan.");
            eprintln!("Contoh: aina gh-login ghp_xxxxxxxxxxxx");
            std::process::exit(1);
        }
        let token = &args[0];
        println!("🐙 Masuk ke GitHub CLI menggunakan Personal Access Token...");
        let msg = KnowledgeEngine::login_github_token(token)?;
        println!("✅ {}", msg);
        Ok(())
    }

    fn handle_gh_device(args: &[String]) -> anyhow::Result<()> {
        let action = args.first().map(|s| s.as_str()).unwrap_or("start");
        match action {
            "poll" | "check" | "status" => {
                println!("🔄 Memeriksa status otorisasi GitHub...");
                match KnowledgeEngine::poll_gh_device_flow()? {
                    GhPollStatus::Success { user } => {
                        println!("\n🎉 OTORISASI BERHASIL!");
                        println!("Akun GitHub @{} telah aktif dan terhubung ke server!", user);
                        println!("Anda kini bisa menjalankan `aina sync` untuk sinkronisasi repository.");
                    }
                    GhPollStatus::Pending { user_code, verification_uri } => {
                        println!("\n⏳ Masih Menunggu Otorisasi Pengguna:");
                        println!("1. Buka browser: {}", verification_uri);
                        println!("2. Masukkan kode: {}", user_code);
                        println!("3. Klik 'Authorize github'.");
                        println!("\nSetelah selesai klik Authorize, jalankan `aina gh-device poll` atau balas chat dengan 'sudah'.");
                    }
                    GhPollStatus::Expired => {
                        println!("\n⚠️ Sesi otorisasi telah kadaluwarsa (lebih dari 15 menit).");
                        println!("Silakan jalankan `aina gh-device` untuk meminta kode verifikasi baru.");
                    }
                    GhPollStatus::Error(e) => {
                        eprintln!("\n❌ {}", e);
                    }
                }
            }
            _ => {
                // start
                println!("🔐 Memulai Otorisasi GitHub Device Flow (Non-Blocking)...");
                let session = KnowledgeEngine::start_gh_device_flow()?;
                println!("\n👉 LANGKAH OTORISASI GITHUB:");
                println!("1. Buka tautan berikut di browser HP / laptop:");
                println!("   🔗 {}", session.verification_uri);
                println!("2. Masukkan kode verifikasi 8-digit ini:");
                println!("   🔑 {}", session.user_code);
                println!("3. Klik tombol 'Authorize github'.");
                println!("\n⏳ Sesi ini aktif selama 15 menit.");
                println!("Setelah klik Authorize di browser, ketik `aina gh-device poll` atau balas chat WhatsApp dengan 'sudah'!");
            }
        }
        Ok(())
    }
}
