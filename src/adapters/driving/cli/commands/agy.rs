//! `aina agy` — Agent-ready Antigravity CLI (`agy`) version introspection,
//! canary-verified atomic updater, and rollback command.

use crate::adapters::driven::agy_cli::AgyUpdater;
use crate::config::AppConfig;
use std::path::PathBuf;

fn resolve_configured_agy_bin() -> PathBuf {
    if let Ok(val) = std::env::var("AGENT_BINARY_PATH").or_else(|_| std::env::var("AGY_BINARY_PATH"))
    {
        let trimmed = val.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    let config_path =
        std::env::var("AINA_CONFIG").unwrap_or_else(|_| "config/config.yaml".to_string());
    let cfg = AppConfig::load_from_file_or_default(&config_path);
    PathBuf::from(cfg.agent.binary_path)
}

pub async fn handle_agy(args: &[String]) -> anyhow::Result<()> {
    let sub = args.first().map(|s| s.as_str()).unwrap_or("status");
    let as_json = args.iter().any(|a| a == "--json" || a == "-j");
    let dry_run = args.iter().any(|a| a == "--dry-run");
    let force = args.iter().any(|a| a == "--force" || a == "-f");
    let skip_llm_canary = args.iter().any(|a| a == "--skip-llm-canary");

    if matches!(sub, "help" | "--help" | "-h") {
        print_help();
        return Ok(());
    }

    let bin_path = resolve_configured_agy_bin();

    match sub {
        "status" | "check" => {
            let force_refresh = sub == "check" || args.iter().any(|a| a == "--refresh");
            let report = AgyUpdater::get_health_report_cached(&bin_path, force_refresh).await;
            let changelog = if sub == "check" && report.update_available {
                AgyUpdater::fetch_changelog_excerpt().await
            } else {
                None
            };
            if as_json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "report": report,
                        "changelog_excerpt": changelog,
                    }))?
                );
            } else {
                println!("=== Status Antigravity CLI (`agy`) ===");
                println!("• Binary Path       : {}", report.binary_path);
                println!("• Canonical Path    : {}", report.canonical_path);
                println!(
                    "• Versi Terpasang   : {}",
                    report.installed_version.as_deref().unwrap_or("tidak terdeteksi")
                );
                println!(
                    "• Versi Manifest    : {}",
                    report.latest_version.as_deref().unwrap_or("offline/gagal")
                );
                println!(
                    "• Update Tersedia   : {}",
                    if report.update_available { "YA" } else { "Tidak (Up-to-Date)" }
                );
                println!(
                    "• Auto-Update Guard : {}",
                    if report.auto_update_disabled {
                        "TERKUNCI (AGY_CLI_DISABLE_AUTO_UPDATE=1)"
                    } else {
                        "Aktif (upstream background)"
                    }
                );
                println!(
                    "• Auto Canary Job   : {}",
                    if report.auto_canary_enabled { "AKTIF" } else { "Non-aktif (notify-only)" }
                );
                println!(
                    "• Rollback Backup   : {}",
                    if report.has_rollback_backup { "Tersedia (agy.prev)" } else { "Belum ada" }
                );
                if let Some(ref st) = report.local_updater_status {
                    println!("• Local Updater Log : {}", st);
                }
                if let Some(ref err) = report.manifest_error {
                    println!("• Catatan Manifest  : {}", err);
                }
                if let Some(ref cl) = changelog {
                    println!("\n--- Ringkasan Changelog Terbaru ---\n{}", cl);
                }
            }
            Ok(())
        }
        "update" | "upgrade" => {
            let result =
                AgyUpdater::perform_canary_update(&bin_path, dry_run, force, skip_llm_canary)
                    .await?;
            if as_json {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                println!("{}", result.message);
                if let Some(ref cl) = result.changelog_excerpt {
                    println!("\n--- Changelog ---\n{}", cl);
                }
            }
            if !result.ok {
                std::process::exit(1);
            }
            Ok(())
        }
        "rollback" => {
            let result = AgyUpdater::rollback_binary(&bin_path).await?;
            if as_json {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                println!("{}", result.message);
            }
            if !result.ok {
                std::process::exit(1);
            }
            Ok(())
        }
        other => {
            eprintln!(
                "Subcommand `aina agy {}` tidak dikenal. Gunakan: status | check | update | rollback.",
                other
            );
            std::process::exit(1);
        }
    }
}

fn print_help() {
    println!(
        r#"Penggunaan `aina agy` (Observabilitas & Canary-Verified Updater Antigravity CLI):

    aina agy status [--json]
    aina agy check [--json]
    aina agy update [--dry-run] [--force] [--skip-llm-canary] [--json]
    aina agy rollback [--json]

Keterangan:
    • status    Menampilkan versi `agy` terpasang, versi manifest resmi, dan status proteksi auto-update.
    • check     Memeriksa manifest rilis terbaru secara langsung beserta cuplikan changelog CLI.
    • update    Mengunduh rilis terbaru ke staging, verifikasi SHA-512, uji Canary flag + JSON output,
                mencadangkan biner lama (`agy.prev`), lalu melakukan atomic swap zero-downtime.
    • rollback  Mengembalikan biner produksi ke versi sebelumnya (`agy.prev`) secara atomik.
"#
    );
}
