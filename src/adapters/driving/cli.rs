use crate::core::domain::knowledge::GhPollStatus;
use crate::core::domain::{ArchiveEngine, ArchiveSearchFilter, AuditEngine, KnowledgeEngine, VersionEngine};
use std::path::{Path, PathBuf};

pub struct CliDispatcher;

impl CliDispatcher {
    pub fn print_help() {
        println!(
            r#"Aina CLI - High-Performance Agentic Utilities

PENGGUNAAN:
    aina [SUBCOMMAND] [OPTIONS]
    aina [server | daemon]    # Menjalankan daemon webhook Aina (default)

    archive search [query]    Pencarian instan riwayat obrolan (FTS5 BM25 + Filter Temporal)
                              Options:
                                --since, -s <durasi>    Filter durasi lampau (misal: 1h, 24h, 7d, 30d)
                                --days, -d <n>          Filter n hari terakhir (alias cepat --since nd)
                                --from <YYYY-MM-DD>     Filter tanggal/waktu awal (misal: 2026-09-01)
                                --to <YYYY-MM-DD>       Filter tanggal/waktu akhir (misal: 2026-09-10)
                                --limit, -l <n>         Batas hasil (default: 10)
                                --workspace, -w <dir>   Path workspace (default: workspaces/default)
                                --json                  Output format JSON terstruktur

    archive import <file>     Impor arsip obrolan WhatsApp (.zip / .txt) ke SQLite lokal
                              Options:
                                --slug, -s <nama>       Nama folder / pengenal obrolan
                                --no-media              Lewati ekstraksi dokumen dan lampiran media
                                --workspace, -w <dir>   Path workspace (default: workspaces/default)

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

    whatsapp status           Status pipeline multi-session WhatsApp (Bot Utama & Companion)
                              Options:
                                --json                  Output format JSON terstruktur

    status                    Alias cepat untuk `whatsapp status`

    user list                 Daftar seluruh profil rekan kerja & wewenang (Profiling Memory)
                              Options:
                                --json                  Output format JSON terstruktur

    user get <jid>            Detail profil & wewenang rekan kerja spesifik
                              Options:
                                --json                  Output format JSON terstruktur

    user set <jid>            Perbarui/simpan profil & wewenang rekan kerja
                              Options:
                                --name <nama>           Nama rekan kerja
                                --role <peran>          Peran / jabatan tim
                                --authority <level>     Tingkat wewenang (admin | staff | guest)
                                --notes <catatan>       Catatan otorisasi & izin yang diberikan

    user search <query>       Cari profil rekan kerja berdasarkan kata kunci
                              Options:
                                --json                  Output format JSON terstruktur

    schedule list             Daftar tugas & riset terjadwal aktif
                              Options:
                                --all                   Tampilkan semua tugas (termasuk non-aktif)
                                --json                  Output format JSON terstruktur

    schedule add              Tambah tugas pengingat atau riset terjadwal baru
                              Options:
                                --title <judul>         Judul pengingat / tugas
                                --type <agent|notify>   Tipe: 'agent' (riset/tindakan AI) atau 'notify' (pesan teks langsung)
                                --target <jid>          Target WhatsApp (misal: 628xxx@s.whatsapp.net)
                                --when <once|daily|interval> Tipe: 'once' (satu kali), 'daily' (tiap hari), 'interval' (berkala)
                                --time <waktu>          Waktu eksekusi (misal: '07:30', '22:26', '+10m', '+1h')
                                --payload <pesan>       Prompt riset (untuk agent) atau teks pesan (untuk notify)

    schedule delete <id>      Hapus tugas terjadwal berdasarkan ID

    model get                 Tampilkan model AI aktif saat ini
                              Options:
                                --json                  Output format JSON terstruktur

    model list                Daftar model AI yang didukung dan status aktifnya
                              Options:
                                --json                  Output format JSON terstruktur

    model set <model_name>    Ganti model AI aktif secara instan di runtime
                              Options:
                                --json                  Output format JSON terstruktur

    version, -v, --version    Informasi versi binary, commit hash, dan daftar kapabilitas aktif
                              Options:
                                --check, -c             Periksa & bandingkan commit dengan upstream GitHub
                                --json                  Output format JSON terstruktur

    update [check]            Periksa pembaruan commit dan fitur baru dari upstream repo GitHub
                              Options:
                                --json                  Output format JSON terstruktur

    help, --help, -h          Tampilkan panduan ini
"#
        );
    }

    pub async fn run(args: Vec<String>) -> anyhow::Result<()> {
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
            "version" | "-v" | "--version" => Self::handle_version(&args[2..]).await,
            "update" => Self::handle_update(&args[2..]).await,
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
            "whatsapp" | "wa" => Self::handle_whatsapp(&args[2..]),
            "status" => Self::handle_whatsapp(&args[1..]),
            "user" | "profile" => Self::handle_user(&args[2..]),
            "schedule" | "cron" => Self::handle_schedule(&args[2..]).await,
            "model" => Self::handle_model(&args[2..]).await,
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
            eprintln!("Operasi archive belum ditentukan. Gunakan: search, stats, import");
            std::process::exit(1);
        }

        let sub = args[0].as_str();
        let ws = Self::resolve_workspace(args);
        let is_json = args.iter().any(|a| a == "--json");

        match sub {
            "search" => {
                let mut query = String::new();
                let mut limit = 10;
                let mut since: Option<String> = None;
                let mut from_date: Option<String> = None;
                let mut to_date: Option<String> = None;

                let mut idx = 1;
                while idx < args.len() {
                    let arg = &args[idx];
                    if (arg == "--limit" || arg == "-l") && idx + 1 < args.len() {
                        limit = args[idx + 1].parse().unwrap_or(10);
                        idx += 2;
                        continue;
                    }
                    if (arg == "--since" || arg == "-s") && idx + 1 < args.len() {
                        since = Some(args[idx + 1].clone());
                        idx += 2;
                        continue;
                    }
                    if (arg == "--days" || arg == "-d") && idx + 1 < args.len() {
                        since = Some(format!("{}d", args[idx + 1]));
                        idx += 2;
                        continue;
                    }
                    if arg == "--from" && idx + 1 < args.len() {
                        from_date = Some(args[idx + 1].clone());
                        idx += 2;
                        continue;
                    }
                    if arg == "--to" && idx + 1 < args.len() {
                        to_date = Some(args[idx + 1].clone());
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
                    if query.is_empty() && !arg.starts_with('-') {
                        query = arg.clone();
                    }
                    idx += 1;
                }

                if query.trim().is_empty() && since.is_none() && from_date.is_none() && to_date.is_none() {
                    eprintln!("Error: Harap sertakan kata kunci pencarian atau filter waktu (--since / --days / --from / --to).");
                    std::process::exit(1);
                }

                let filter = ArchiveSearchFilter {
                    query: query.clone(),
                    limit,
                    since: since.clone(),
                    from_date: from_date.clone(),
                    to_date: to_date.clone(),
                };

                let results = ArchiveEngine::search_all_with_filter(&ws, &filter)?;

                if is_json {
                    println!("{}", serde_json::to_string_pretty(&results)?);
                } else {
                    let query_label = if query.is_empty() { "(Semua Topik)" } else { &query };
                    let mut filter_desc = Vec::new();
                    if let Some(s) = &since {
                        filter_desc.push(format!("sejak {} terakhir", s));
                    }
                    if let Some(f) = &from_date {
                        filter_desc.push(format!("dari {}", f));
                    }
                    if let Some(t) = &to_date {
                        filter_desc.push(format!("s.d. {}", t));
                    }
                    let extra_info = if filter_desc.is_empty() {
                        String::new()
                    } else {
                        format!(" [{}]", filter_desc.join(", "))
                    };

                    println!(
                        "🔍 Hasil Pencarian Arsip [{}{}]: {} pesan ditemukan (limit: {})\n",
                        query_label,
                        extra_info,
                        results.len(),
                        limit
                    );
                    if results.is_empty() {
                        println!("(Tidak ada percakapan yang cocok dengan kriteria pencarian)");
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
            "import" => {
                if args.len() < 2 {
                    eprintln!("Penggunaan: aina archive import <path_ke_zip_atau_txt> [--workspace <dir>] [--slug <nama>] [--no-media]");
                    std::process::exit(1);
                }
                let archive_path = &args[1];
                let mut slug: Option<String> = None;
                let mut no_media = false;
                let mut idx = 2;
                while idx < args.len() {
                    let arg = &args[idx];
                    if (arg == "--slug" || arg == "-s") && idx + 1 < args.len() {
                        slug = Some(args[idx + 1].clone());
                        idx += 2;
                        continue;
                    }
                    if arg == "--no-media" {
                        no_media = true;
                        idx += 1;
                        continue;
                    }
                    if (arg == "--workspace" || arg == "-w") && idx + 1 < args.len() {
                        idx += 2;
                        continue;
                    }
                    idx += 1;
                }

                let script_candidates = [
                    PathBuf::from("scripts/chat_importer.py"),
                    PathBuf::from("/root/projects/aina/scripts/chat_importer.py"),
                ];
                let script_path = script_candidates
                    .into_iter()
                    .find(|p| p.is_file())
                    .ok_or_else(|| {
                        anyhow::anyhow!("Skrip chat_importer.py tidak ditemukan di scripts/ atau /root/projects/aina/scripts/")
                    })?;

                let mut cmd = std::process::Command::new("python3");
                cmd.arg(&script_path)
                    .arg("import")
                    .arg(archive_path)
                    .arg("--workspace")
                    .arg(ws.to_string_lossy().to_string());

                if let Some(s) = slug {
                    cmd.arg("--name").arg(s);
                }
                if no_media {
                    cmd.arg("--no-media");
                }

                let status = cmd.status()?;
                if !status.success() {
                    std::process::exit(status.code().unwrap_or(1));
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

    fn handle_whatsapp(args: &[String]) -> anyhow::Result<()> {
        let sub = args.first().map(|s| s.as_str()).unwrap_or("status");
        let as_json = args.iter().any(|a| a == "--json");

        match sub {
            "status" => {
                let config = crate::config::AppConfig::load_from_file_or_default("config/aina.yaml");

                let bot_session = config.whatsmeow.bot_session_id.as_deref().unwrap_or("default");
                let companion_session = config.whatsmeow.companion_session_id.as_deref().unwrap_or("companion");
                let is_companion_active = config.whatsmeow.companion_jid.is_some();

                if as_json {
                    let out = serde_json::json!({
                        "gateway_url": config.whatsmeow.base_url,
                        "primary_bot": {
                            "name": config.whatsmeow.bot_name,
                            "jid": config.whatsmeow.bot_jid,
                            "session_id": bot_session,
                            "role": "PrimaryBot",
                            "status": "Configured"
                        },
                        "companion_sensor": {
                            "enabled": is_companion_active,
                            "name": config.whatsmeow.companion_name,
                            "jid": config.whatsmeow.companion_jid,
                            "session_id": companion_session,
                            "role": "UserCompanion",
                            "status": if is_companion_active { "Active" } else { "Unconfigured" }
                        }
                    });
                    println!("{}", serde_json::to_string_pretty(&out)?);
                    return Ok(());
                }

                println!("📱 Status Koneksi WhatsApp Pipeline Aina (Multi-Session)");
                println!("============================================================");
                println!("🔗 Whatsmeow Gateway : {}", config.whatsmeow.base_url);
                println!();
                println!("🤖 Sesi 1: Bot Utama (Primary Dedicated Bot)");
                println!("   • Nama Bot   : {}", config.whatsmeow.bot_name);
                println!("   • WhatsApp   : {}", config.whatsmeow.bot_jid);
                println!("   • Session ID : {}", bot_session);
                println!("   • Perilaku   : Selalu balas DM; di grup hanya jika di-mention / dipanggil");
                println!();
                println!("👥 Sesi 2: Companion Sensor (Akun Pribadi / Shadow Sensor)");
                if let Some(c_jid) = &config.whatsmeow.companion_jid {
                    let c_name = config.whatsmeow.companion_name.as_deref().unwrap_or("Personal Account");
                    println!("   • Status     : ✅ AKTIF & TERHUBUNG");
                    println!("   • Nama Akun  : {}", c_name);
                    println!("   • WhatsApp   : {}", c_jid);
                    println!("   • Session ID : {}", companion_session);
                    println!("   • Perilaku   : Mode Shadow. DM personal diabaikan demi privasi.");
                    println!("                  Pesan grup direkam otomatis (RecordOnly) untuk konteks.");
                    println!("                  Hanya merespons saat owner/member memanggil '!aina' / nama bot.");
                } else {
                    println!("   • Status     : ⚪ Belum Dikonfigurasi (Opsional)");
                    println!("   • Keterangan : Tambahkan WHATSMEOW_COMPANION_JID di .env untuk menghubungkan");
                    println!("                  akun WhatsApp pribadi Anda tanpa harus mengundang bot baru ke grup.");
                }
                println!("============================================================");
                Ok(())
            }
            _ => {
                println!("Penggunaan: aina whatsapp status [--json]");
                Ok(())
            }
        }
    }

    fn handle_user(args: &[String]) -> anyhow::Result<()> {
        if args.is_empty() {
            println!(
                r#"Penggunaan manajemen profil & wewenang rekan kerja (Aina Profiling Memory):
    aina user list [--json]
    aina user get <jid_atau_nama> [--json]
    aina user set <jid> [--name <nama>] [--role <peran>] [--authority <admin|staff|guest>] [--notes <catatan>]
    aina user search <query> [--json]
"#
            );
            return Ok(());
        }

        let config_path = std::env::var("AINA_CONFIG").unwrap_or_else(|_| "config/config.yaml".to_string());
        let config = crate::config::AppConfig::load_from_file_or_default(&config_path);
        let db_path = std::env::var("DATABASE_PATH").unwrap_or(config.database.path);

        let conn = match rusqlite::Connection::open(&db_path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("⚠️ Tidak dapat membuka database di {}: {}", db_path, e);
                std::process::exit(1);
            }
        };

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

        let subcmd = args[0].as_str();
        match subcmd {
            "list" | "ls" => {
                let as_json = args.iter().any(|a| a == "--json");
                let mut stmt = conn.prepare(
                    "SELECT sender_jid, name, role, authority_level, notes, updated_at
                     FROM user_profiles
                     ORDER BY updated_at DESC",
                )?;

                let rows = stmt.query_map([], |row| {
                    Ok(serde_json::json!({
                        "sender_jid": row.get::<_, String>(0)?,
                        "name": row.get::<_, Option<String>>(1)?,
                        "role": row.get::<_, Option<String>>(2)?,
                        "authority_level": row.get::<_, String>(3)?,
                        "notes": row.get::<_, Option<String>>(4)?,
                        "updated_at": row.get::<_, Option<String>>(5)?,
                    }))
                })?;

                let mut profiles = Vec::new();
                for r in rows.flatten() {
                    profiles.push(r);
                }

                if as_json {
                    println!("{}", serde_json::to_string_pretty(&profiles)?);
                    return Ok(());
                }

                println!("👤 Profil Pengguna Terdaftar (Aina Profiling Memory & Access Control)");
                println!("================================================================================");
                if profiles.is_empty() {
                    println!("(Belum ada profil pengguna tersimpan)");
                } else {
                    for p in &profiles {
                        let jid = p["sender_jid"].as_str().unwrap_or("-");
                        let name = p["name"].as_str().unwrap_or("-");
                        let role = p["role"].as_str().unwrap_or("-");
                        let auth = p["authority_level"].as_str().unwrap_or("staff").to_uppercase();
                        let notes = p["notes"].as_str().unwrap_or("-");
                        println!("• {} ({})", name, jid);
                        println!("  Peran: {} | Otoritas: {}", role, auth);
                        println!("  Catatan / Izin: {}", notes);
                        println!("--------------------------------------------------------------------------------");
                    }
                }
                Ok(())
            }
            "get" => {
                if args.len() < 2 {
                    eprintln!("Format salah. Contoh: aina user get 628123456789@s.whatsapp.net");
                    std::process::exit(1);
                }
                let target = &args[1];
                let as_json = args.iter().any(|a| a == "--json");
                let target_like = format!("%{}%", target);

                let mut stmt = conn.prepare(
                    "SELECT sender_jid, name, role, authority_level, notes, updated_at
                     FROM user_profiles
                     WHERE sender_jid = ?1 OR sender_jid LIKE ?2 OR name LIKE ?2
                     LIMIT 1",
                )?;

                let mut rows = stmt.query(rusqlite::params![target, target_like])?;
                if let Some(row) = rows.next()? {
                    let jid: String = row.get(0)?;
                    let name: Option<String> = row.get(1)?;
                    let role: Option<String> = row.get(2)?;
                    let auth: String = row.get(3)?;
                    let notes: Option<String> = row.get(4)?;
                    let updated: Option<String> = row.get(5)?;

                    if as_json {
                        let val = serde_json::json!({
                            "sender_jid": jid,
                            "name": name,
                            "role": role,
                            "authority_level": auth,
                            "notes": notes,
                            "updated_at": updated,
                        });
                        println!("{}", serde_json::to_string_pretty(&val)?);
                    } else {
                        println!("👤 Detail Profil Pengguna");
                        println!("• WhatsApp JID : {}", jid);
                        println!("• Nama         : {}", name.as_deref().unwrap_or("-"));
                        println!("• Peran / Tim  : {}", role.as_deref().unwrap_or("-"));
                        println!("• Otoritas     : {}", auth.to_uppercase());
                        println!("• Catatan/Izin : {}", notes.as_deref().unwrap_or("-"));
                        println!("• Terakhir Update: {}", updated.as_deref().unwrap_or("-"));
                    }
                } else {
                    println!("⚠️ Profil untuk `{}` tidak ditemukan.", target);
                }
                Ok(())
            }
            "set" => {
                if args.len() < 2 {
                    eprintln!("Format salah. Contoh: aina user set 628123456789@s.whatsapp.net --name 'Budi' --role 'Developer' --authority staff --notes 'Diizinkan akses rekap'");
                    std::process::exit(1);
                }
                let jid = if args[1].contains('@') {
                    args[1].clone()
                } else {
                    format!("{}@s.whatsapp.net", args[1])
                };

                let mut name: Option<String> = None;
                let mut role: Option<String> = None;
                let mut authority: Option<String> = None;
                let mut notes: Option<String> = None;

                let mut i = 2;
                while i < args.len() {
                    match args[i].as_str() {
                        "--name" | "-n" if i + 1 < args.len() => {
                            name = Some(args[i + 1].clone());
                            i += 2;
                        }
                        "--role" | "-r" if i + 1 < args.len() => {
                            role = Some(args[i + 1].clone());
                            i += 2;
                        }
                        "--authority" | "-a" if i + 1 < args.len() => {
                            authority = Some(args[i + 1].to_lowercase());
                            i += 2;
                        }
                        "--notes" | "-m" if i + 1 < args.len() => {
                            notes = Some(args[i + 1].clone());
                            i += 2;
                        }
                        _ => i += 1,
                    }
                }

                conn.execute(
                    "INSERT INTO user_profiles (sender_jid, name, role, authority_level, notes, updated_at)
                     VALUES (?1, ?2, ?3, COALESCE(?4, 'staff'), ?5, CURRENT_TIMESTAMP)
                     ON CONFLICT(sender_jid) DO UPDATE SET
                         name = COALESCE(?2, user_profiles.name),
                         role = COALESCE(?3, user_profiles.role),
                         authority_level = COALESCE(?4, user_profiles.authority_level),
                         notes = COALESCE(?5, user_profiles.notes),
                         updated_at = CURRENT_TIMESTAMP",
                    rusqlite::params![jid, name, role, authority, notes],
                )?;

                println!("✅ Profil pengguna `{}` berhasil diperbarui di Aina Profiling Memory.", jid);
                Ok(())
            }
            "search" | "find" => {
                if args.len() < 2 {
                    eprintln!("Format salah. Contoh: aina user search 'Budi'");
                    std::process::exit(1);
                }
                let q = &args[1];
                let as_json = args.iter().any(|a| a == "--json");
                let like_expr = format!("%{}%", q);

                let mut stmt = conn.prepare(
                    "SELECT sender_jid, name, role, authority_level, notes, updated_at
                     FROM user_profiles
                     WHERE sender_jid LIKE ?1 OR name LIKE ?1 OR role LIKE ?1 OR notes LIKE ?1
                     ORDER BY updated_at DESC",
                )?;

                let rows = stmt.query_map(rusqlite::params![like_expr], |row| {
                    Ok(serde_json::json!({
                        "sender_jid": row.get::<_, String>(0)?,
                        "name": row.get::<_, Option<String>>(1)?,
                        "role": row.get::<_, Option<String>>(2)?,
                        "authority_level": row.get::<_, String>(3)?,
                        "notes": row.get::<_, Option<String>>(4)?,
                        "updated_at": row.get::<_, Option<String>>(5)?,
                    }))
                })?;

                let mut matches = Vec::new();
                for r in rows.flatten() {
                    matches.push(r);
                }

                if as_json {
                    println!("{}", serde_json::to_string_pretty(&matches)?);
                    return Ok(());
                }

                println!("🔍 Hasil Pencarian Profil untuk kata kunci `{}` (Ditemukan: {})", q, matches.len());
                println!("================================================================================");
                for m in &matches {
                    let jid = m["sender_jid"].as_str().unwrap_or("-");
                    let name = m["name"].as_str().unwrap_or("-");
                    let role = m["role"].as_str().unwrap_or("-");
                    let auth = m["authority_level"].as_str().unwrap_or("staff").to_uppercase();
                    let notes = m["notes"].as_str().unwrap_or("-");
                    println!("• {} ({})", name, jid);
                    println!("  Peran: {} | Otoritas: {}", role, auth);
                    println!("  Catatan / Izin: {}", notes);
                    println!("--------------------------------------------------------------------------------");
                }
                Ok(())
            }
            _ => {
                eprintln!("Subcommand user tidak dikenal: `{}`. Pilihan: list, get, set, search.", subcmd);
                Ok(())
            }
        }
    }

    async fn handle_schedule(args: &[String]) -> anyhow::Result<()> {
        use crate::core::ports::SessionStorePort;

        if args.is_empty() {
            println!(
                r#"Manajemen Tugas & Riset Terjadwal (Aina Scheduled Wake-up & Reminders):
    aina schedule list [--all] [--json]
    aina schedule get <id> [--json]
    aina schedule add --title <judul> --type <agent|notify> --target <jid> --when <once|daily|interval> --time <waktu> --payload <isi>
    aina schedule delete <id>
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
            _ => {
                eprintln!("Subcommand schedule tidak dikenal: `{}`. Pilihan: list, get, add, delete.", subcmd);
                Ok(())
            }
        }
    }

    async fn handle_model(args: &[String]) -> anyhow::Result<()> {
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

    async fn handle_version(args: &[String]) -> anyhow::Result<()> {
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

    async fn handle_update(args: &[String]) -> anyhow::Result<()> {
        let mut check_args = vec!["--check".to_string()];
        check_args.extend_from_slice(args);
        Self::handle_version(&check_args).await
    }
}
