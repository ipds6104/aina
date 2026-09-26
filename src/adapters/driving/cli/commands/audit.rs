use crate::core::domain::AuditEngine;
use crate::core::ports::SessionStorePort;
use std::path::{Path, PathBuf};

pub async fn handle_audit(args: &[String]) -> anyhow::Result<()> {
    let subcmd = args.first().map(|s| s.as_str()).unwrap_or("");
    match subcmd {
        "summary" => {
            let is_json = args.iter().any(|a| a == "--json");
            let config_path = std::env::var("AINA_CONFIG").unwrap_or_else(|_| "config/config.yaml".to_string());
            let config = crate::config::AppConfig::load_from_file_or_default(&config_path);
            let db_path = std::env::var("DATABASE_PATH").unwrap_or(config.database.path);
            let store = crate::adapters::driven::SqliteSessionStore::new(&db_path)?;
            let summary = store.get_action_audit_summary().await?;

            if is_json {
                println!("{}", serde_json::to_string_pretty(&summary)?);
            } else {
                println!("📊 Ringkasan Observability & Audit Aina:\n");
                println!("• Total Aksi Tercatat  : {}", summary.total_actions);
                println!("• Total Respons AI     : {}", summary.total_responses);
                println!("• Total Catatan Pasif  : {}", summary.total_recorded_only);
                println!("• Total Diabaikan      : {}", summary.total_ignored);
                println!("• Total Kesalahan/Gagal: {}", summary.total_errors);
                println!("• Rata-rata Durasi     : {:.2}s\n", summary.avg_duration_seconds);

                if !summary.top_usecases.is_empty() {
                    println!("🎯 Usecase Paling Sering Digunakan:");
                    for (i, u) in summary.top_usecases.iter().enumerate() {
                        let label = crate::core::domain::UseCaseCategory::from_str(&u.key).display_name();
                        let pct = if summary.total_actions > 0 {
                            (u.count as f64 / summary.total_actions as f64) * 100.0
                        } else {
                            0.0
                        };
                        println!("  {}. {} ({} kali, {:.1}%)", i + 1, label, u.count, pct);
                    }
                    println!();
                }

                if !summary.top_tools_used.is_empty() {
                    println!("🛠️ Alat AI Paling Sering Digunakan (Frekuensi Aksi):");
                    for (i, t) in summary.top_tools_used.iter().enumerate() {
                        println!("  {}. {} ({} aksi)", i + 1, t.key, t.count);
                    }
                    println!();
                }

                if !summary.tool_call_frequency.is_empty() {
                    println!("📈 Total Pemanggilan Alat (Tool Call Volume):");
                    for (i, t) in summary.tool_call_frequency.iter().enumerate() {
                        println!("  {}. {} ({} kali dipanggil)", i + 1, t.key, t.count);
                    }
                    println!();
                }

                if !summary.most_active_chats.is_empty() {
                    println!("💬 Ruang Obrolan Paling Aktif:");
                    for (i, c) in summary.most_active_chats.iter().enumerate() {
                        println!("  {}. {} ({} pesan)", i + 1, c.key, c.count);
                    }
                    println!();
                }
            }
            Ok(())
        }
        "diagnostics" | "diag" => {
            let is_json = args.iter().any(|a| a == "--json");
            let brain_path = crate::core::domain::AuditEngine::default_brain_path();
            let (rss, virt) = crate::core::domain::AuditEngine::read_process_memory();
            let config_path = std::env::var("AINA_CONFIG").unwrap_or_else(|_| "config/config.yaml".to_string());
            let config = crate::config::AppConfig::load_from_file_or_default(&config_path);
            let db_path = std::env::var("DATABASE_PATH").unwrap_or(config.database.path);
            let db_size = std::fs::metadata(&db_path).map(|m| m.len()).unwrap_or(0);
            let brain_dir_size = crate::core::domain::AuditEngine::compute_dir_size(&brain_path);
            let total_convs = crate::core::domain::AuditEngine::find_transcripts(&brain_path).len();
            let workspaces = crate::core::domain::AuditEngine::inspect_workspaces(Path::new(&config.agent.workspace_dir));

            let scheduler = if let Ok(store) = crate::adapters::driven::SqliteSessionStore::new(&db_path) {
                store.get_scheduler_diagnostics().await.ok()
            } else {
                None
            };

            let diag = crate::core::domain::SystemDiagnostics {
                uptime_seconds: crate::core::domain::get_process_uptime_secs(),
                memory_rss_bytes: rss,
                memory_virt_bytes: virt,
                memory_rss_human: crate::core::domain::AuditEngine::format_bytes(rss),
                memory_virt_human: crate::core::domain::AuditEngine::format_bytes(virt),
                db_size_bytes: db_size,
                brain_dir_size_bytes: brain_dir_size,
                total_conversations_in_brain: total_convs,
                active_model: config.agent.model.clone(),
                bot_jid: config.whatsmeow.bot_jid.clone(),
                bot_name: "Aina".to_string(),
                workspaces,
                scheduler,
                timestamp_epoch: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64,
            };

            if is_json {
                println!("{}", serde_json::to_string_pretty(&diag)?);
            } else {
                println!("🩺 Diagnostik Sistem & Memori Aina:\n");
                println!("• Memori RSS (Resident)  : {} ({} bytes)", diag.memory_rss_human, diag.memory_rss_bytes);
                println!("• Memori Virtual (Total) : {} ({} bytes)", diag.memory_virt_human, diag.memory_virt_bytes);
                println!("• Ukuran Basis Data SQLite: {}", crate::core::domain::AuditEngine::format_bytes(diag.db_size_bytes));
                println!("• Ukuran Direktori Brain : {}", crate::core::domain::AuditEngine::format_bytes(diag.brain_dir_size_bytes));
                println!("• Total Sesi Percakapan  : {}", diag.total_conversations_in_brain);
                println!("• Model AI Terkonfigurasi: {}", diag.active_model);
                println!("\n📁 Status Repositori Workspace:");
                for ws in &diag.workspaces {
                    let git_str = if ws.is_git_repo {
                        format!("Git: {} (branch: {}, dirty: {})", ws.git_commit.as_deref().unwrap_or("-"), ws.git_branch.as_deref().unwrap_or("-"), ws.is_dirty)
                    } else {
                        "Non-Git".to_string()
                    };
                    println!("  • {} -> {} [{} skrip, {}]", ws.name, ws.path, ws.scripts_count, git_str);
                }
            }
            Ok(())
        }
        "actions" => {
            let is_json = args.iter().any(|a| a == "--json");
            let mut limit = 20;
            let mut query = None;
            let mut status = None;
            let mut usecase = None;
            let mut tool = None;
            let mut idx = 1;
            while idx < args.len() {
                let a = &args[idx];
                if (a == "--limit" || a == "-l") && idx + 1 < args.len() {
                    limit = args[idx + 1].parse().unwrap_or(20);
                    idx += 2;
                    continue;
                }
                if (a == "--query" || a == "-q") && idx + 1 < args.len() {
                    query = Some(args[idx + 1].clone());
                    idx += 2;
                    continue;
                }
                if a == "--status" && idx + 1 < args.len() {
                    status = Some(args[idx + 1].clone());
                    idx += 2;
                    continue;
                }
                if (a == "--usecase" || a == "-u") && idx + 1 < args.len() {
                    usecase = Some(args[idx + 1].clone());
                    idx += 2;
                    continue;
                }
                if (a == "--tool" || a == "-t") && idx + 1 < args.len() {
                    tool = Some(args[idx + 1].clone());
                    idx += 2;
                    continue;
                }
                idx += 1;
            }

            let config_path = std::env::var("AINA_CONFIG").unwrap_or_else(|_| "config/config.yaml".to_string());
            let config = crate::config::AppConfig::load_from_file_or_default(&config_path);
            let db_path = std::env::var("DATABASE_PATH").unwrap_or(config.database.path);
            let store = crate::adapters::driven::SqliteSessionStore::new(&db_path)?;

            let filter = crate::core::domain::ActionAuditFilter {
                query,
                status,
                usecase,
                tool,
                limit: Some(limit),
                ..Default::default()
            };
            let actions = store.query_action_audits(&filter).await?;

            if is_json {
                println!("{}", serde_json::to_string_pretty(&actions)?);
            } else {
                println!("📋 Riwayat Audit Tindakan WhatsApp Aina ({} aksi):\n", actions.len());
                if actions.is_empty() {
                    println!("(Belum ada riwayat tindakan)");
                } else {
                    for (i, act) in actions.iter().enumerate() {
                        let dur_str = act.duration_seconds.map(|d| format!("{:.2}s", d)).unwrap_or_else(|| "-".to_string());
                        let sender = act.sender_name.as_deref().unwrap_or(&act.sender_jid);
                        let uc_label = crate::core::domain::UseCaseCategory::from_str(&act.usecase).display_name();
                        println!("{}. [{}] {} [{}] | Pengirim: {} | Status: {} (Durasi: {})",
                            i + 1, act.id, act.decision.to_uppercase(), uc_label, sender, act.status, dur_str);
                        println!("   Pesan Masuk: {}", act.input_text.lines().next().unwrap_or(""));
                        if let Some(ref resp) = act.response_text {
                            let resp_line: &str = resp.as_str().lines().next().unwrap_or("");
                            let trunc: String = resp_line.chars().take(80).collect();
                            println!("   Balasan AI : {}...", trunc);
                        }
                        if !act.tools_invoked.is_empty() {
                            println!("   Alat Digunakan: {}", act.tools_invoked.join(", "));
                        }
                        println!();
                    }
                }
            }
            Ok(())
        }
        _ => {
            // Default: query transcript steps
            let mut query = None;
            let mut errors_only = false;
            let mut limit = 20;
            let is_json = args.iter().any(|a| a == "--json");

            let start_idx = if subcmd == "transcripts" { 1 } else { 0 };
            let mut idx = start_idx;
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
}
