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
                let p = PathBuf::from(&args[idx + 1]);
                if p.is_dir() {
                    return p;
                }
                // Check if relative to workspaces/
                let candidate = Path::new("workspaces").join(&args[idx + 1]);
                if candidate.is_dir() {
                    return candidate;
                }
                return p;
            }
            idx += 1;
        }

        // Fallback checks
        let candidates = [
            PathBuf::from(std::env::var("AGENT_WORKSPACE").unwrap_or_default()),
            PathBuf::from("workspaces/default"),
            PathBuf::from("."),
        ];
        for c in candidates {
            if c.join("knowledge").is_dir() || c.is_dir() {
                return c;
            }
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
}
