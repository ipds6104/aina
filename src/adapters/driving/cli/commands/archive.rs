use crate::adapters::driving::cli::workspace_resolver::resolve_workspace;
use crate::core::domain::{ArchiveEngine, ArchiveSearchFilter};
use std::path::PathBuf;

pub fn handle_archive(args: &[String]) -> anyhow::Result<()> {
    if args.is_empty() {
        eprintln!("Operasi archive belum ditentukan. Gunakan: search, stats, import");
        std::process::exit(1);
    }

    let sub = args[0].as_str();
    let ws = resolve_workspace(args);
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
