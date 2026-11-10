use crate::core::domain::NewExperienceRecord;
use crate::core::ports::SessionStorePort;

pub async fn handle_experience(args: &[String]) -> anyhow::Result<()> {
    let subcmd = args.first().map(|s| s.as_str()).unwrap_or("list");
    let is_json = args.iter().any(|a| a == "--json");

    let config_path = std::env::var("AINA_CONFIG").unwrap_or_else(|_| "config/config.yaml".to_string());
    let config = crate::config::AppConfig::load_from_file_or_default(&config_path);
    let db_path = std::env::var("DATABASE_PATH").unwrap_or(config.database.path);
    let store = crate::adapters::driven::SqliteSessionStore::new(&db_path)?;

    match subcmd {
        "list" => {
            let mut domain_filter: Option<String> = None;
            let mut limit: usize = 20;

            let mut idx = 1;
            while idx < args.len() {
                let arg = &args[idx];
                if arg == "--domain" && idx + 1 < args.len() {
                    domain_filter = Some(args[idx + 1].clone());
                    idx += 2;
                    continue;
                }
                if (arg == "--limit" || arg == "-l") && idx + 1 < args.len() {
                    limit = args[idx + 1].parse().unwrap_or(20);
                    idx += 2;
                    continue;
                }
                idx += 1;
            }

            let records = store.list_experiences(domain_filter.as_deref(), limit).await?;

            if is_json {
                println!("{}", serde_json::to_string_pretty(&records)?);
            } else {
                println!("📚 [AINA EXPERIENCE LIBRARY — HARNESS² FORMAT]\n");
                if records.is_empty() {
                    println!("Belum ada entri pengalaman tersimpan. Tambahkan via `aina experience add`.");
                } else {
                    for r in records {
                        println!("• [{}] (Domain: `{}` | Confidence: {:.2})", r.id, r.domain, r.confidence);
                        println!("  - Cue     : {}", r.cue);
                        println!("  - Lesson  : {}", r.lesson);
                        println!("  - Evidence: {}", r.evidence);
                        if let Some(ref t) = r.task_id {
                            println!("  - Task ID : {}", t);
                        }
                        println!();
                    }
                }
            }
            Ok(())
        }
        "search" => {
            if args.len() < 2 || args[1].starts_with('-') {
                eprintln!("Penggunaan: aina experience search <query> [--domain <dom>] [--limit <n>] [--json]");
                std::process::exit(1);
            }
            let query = &args[1];
            let mut domain_filter: Option<String> = None;
            let mut limit: usize = 10;

            let mut idx = 2;
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

            let records = store.search_experiences(query, domain_filter.as_deref(), limit).await?;

            if is_json {
                println!("{}", serde_json::to_string_pretty(&records)?);
            } else {
                println!("🔍 [HASIL PENCARIAN PENGALAMAN: \"{}\"] ({} ditemukan)\n", query, records.len());
                for r in records {
                    println!("• [{}] (Domain: `{}` | Confidence: {:.2})", r.id, r.domain, r.confidence);
                    println!("  - Cue     : {}", r.cue);
                    println!("  - Lesson  : {}", r.lesson);
                    println!("  - Evidence: {}", r.evidence);
                    println!();
                }
            }
            Ok(())
        }
        "get" => {
            if args.len() < 2 {
                eprintln!("Penggunaan: aina experience get <id> [--json]");
                std::process::exit(1);
            }
            let id = &args[1];
            if let Some(rec) = store.get_experience(id).await? {
                if is_json {
                    println!("{}", serde_json::to_string_pretty(&rec)?);
                } else {
                    println!("{}", rec.to_markdown());
                }
            } else {
                eprintln!("Pengalaman dengan ID `{}` tidak ditemukan.", id);
                std::process::exit(1);
            }
            Ok(())
        }
        "add" => {
            let mut cue = String::new();
            let mut lesson = String::new();
            let mut evidence = String::new();
            let mut domain = "general".to_string();
            let mut confidence = 1.0;
            let mut task_id = None;

            let mut idx = 1;
            while idx < args.len() {
                let arg = &args[idx];
                if arg == "--cue" && idx + 1 < args.len() {
                    cue = args[idx + 1].clone();
                    idx += 2;
                    continue;
                }
                if arg == "--lesson" && idx + 1 < args.len() {
                    lesson = args[idx + 1].clone();
                    idx += 2;
                    continue;
                }
                if arg == "--evidence" && idx + 1 < args.len() {
                    evidence = args[idx + 1].clone();
                    idx += 2;
                    continue;
                }
                if arg == "--domain" && idx + 1 < args.len() {
                    domain = args[idx + 1].clone();
                    idx += 2;
                    continue;
                }
                if arg == "--confidence" && idx + 1 < args.len() {
                    confidence = args[idx + 1].parse().unwrap_or(1.0);
                    idx += 2;
                    continue;
                }
                if arg == "--task" && idx + 1 < args.len() {
                    task_id = Some(args[idx + 1].clone());
                    idx += 2;
                    continue;
                }
                idx += 1;
            }

            if cue.is_empty() || lesson.is_empty() || evidence.is_empty() {
                eprintln!("Error: Parameter `--cue`, `--lesson`, dan `--evidence` wajib diisi!");
                eprintln!("Contoh: aina experience add --cue \"Error PRoot 3D\" --lesson \"Gunakan 2D\" --evidence \"Test #1 sukses\" --domain \"coding\"");
                std::process::exit(1);
            }

            let new_rec = NewExperienceRecord {
                domain,
                cue,
                lesson,
                evidence,
                confidence: Some(confidence),
                task_id,
            };

            let exp_id = store.record_experience(&new_rec).await?;
            if is_json {
                println!(
                    "{}",
                    serde_json::json!({
                        "status": "success",
                        "id": exp_id,
                    })
                );
            } else {
                println!("✅ Pengalaman berhasil disimpan dengan ID: `{}`", exp_id);
            }
            Ok(())
        }
        "help" | "--help" | "-h" => {
            print_experience_help();
            Ok(())
        }
        _ => {
            eprintln!("Subcommand tidak dikenal: `{}`. Ketik `aina experience help`.", subcmd);
            std::process::exit(1);
        }
    }
}

fn print_experience_help() {
    println!(
        r#"Aina Experience Library (Harness² Paradigm)

PENGGUNAAN:
    aina experience [SUBCOMMAND] [OPTIONS]

SUBCOMMANDS:
    list                      Daftar seluruh pengalaman dan pelajaran tersimpan
                              Options:
                                --domain <domain>       Filter domain tertentu
                                --limit, -l <n>         Batas entri (default: 20)
                                --json                  Format output JSON

    search <query>            Pencarian instan cue, lesson, atau evidence
                              Options:
                                --domain <domain>       Filter domain tertentu
                                --limit, -l <n>         Batas entri (default: 10)
                                --json                  Format output JSON

    get <id>                  Tampilkan detail pengalaman dalam format Markdown EXPERIENCE.md
                              Options:
                                --json                  Format output JSON

    add                       Tambah entri pengalaman baru
                              Options:
                                --cue <text>            Pemicu situasional / gejala error (Wajib)
                                --lesson <text>         Pelajaran transferable / prosedur (Wajib)
                                --evidence <text>       Bukti empiris eksekusi nyata (Wajib)
                                --domain <domain>       Kategori domain (default: general)
                                --confidence <val>      Skor keyakinan 0.0 - 1.0 (default: 1.0)
                                --task <task_id>        Task ID terkait (Opsional)
                                --json                  Format output JSON

CONTOH:
    aina experience add --cue "Error Blender di container PRoot" \
      --lesson "Jangan panggil binary blender, buat skrip python atau generate_image" \
      --evidence "Task triage verifikasi exit 0" --domain "coding_agent"
    aina experience search "Blender"
    aina experience list --domain coding_agent
"#
    );
}
