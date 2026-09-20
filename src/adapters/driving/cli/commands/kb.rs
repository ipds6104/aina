use crate::adapters::driving::cli::workspace_resolver::resolve_workspace;
use crate::core::domain::KnowledgeEngine;

pub fn handle_kb(args: &[String]) -> anyhow::Result<()> {
    if args.is_empty() {
        eprintln!("Operasi kb belum ditentukan. Gunakan: lint, groom, schedule");
        std::process::exit(1);
    }

    let sub = args[0].as_str();
    let ws = resolve_workspace(args);
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
