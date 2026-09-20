use crate::adapters::driving::cli::commands::github::{
    handle_gh_create, handle_gh_device, handle_gh_login, handle_gh_status,
};
use crate::adapters::driving::cli::workspace_resolver::resolve_workspace;
use crate::core::domain::KnowledgeEngine;
use std::path::Path;

pub fn handle_workspace(args: &[String]) -> anyhow::Result<()> {
    if args.is_empty() {
        eprintln!("Operasi workspace belum ditentukan. Gunakan: info, init, clone");
        std::process::exit(1);
    }

    let sub = args[0].as_str();
    let is_json = args.iter().any(|a| a == "--json");

    match sub {
        "info" => {
            let ws = resolve_workspace(args);
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
        "clone" => handle_workspace_clone(&args[1..]),
        "sync" => handle_workspace_sync(&args[1..]),
        "link" => handle_workspace_link(&args[1..]),
        "gh-status" => handle_gh_status(),
        "gh-login" | "login" => handle_gh_login(&args[1..]),
        "gh-device" | "device" => handle_gh_device(&args[1..]),
        "gh-create" => handle_gh_create(&args[1..]),
        _ => {
            eprintln!("Subcommand workspace tidak dikenal: `{}`", sub);
            std::process::exit(1);
        }
    }
}

pub fn handle_workspace_clone(args: &[String]) -> anyhow::Result<()> {
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

pub fn handle_workspace_sync(args: &[String]) -> anyhow::Result<()> {
    let ws = resolve_workspace(args);
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

pub fn handle_workspace_link(args: &[String]) -> anyhow::Result<()> {
    if args.is_empty() {
        eprintln!("Error: URL git repository wajib disertakan.");
        eprintln!("Contoh: aina workspace link https://github.com/my-org/knowledge-base.git [--workspace <dir>]");
        std::process::exit(1);
    }
    let git_url = &args[0];
    let ws = resolve_workspace(&args[1..]);

    println!("🔗 Menghubungkan workspace {:?} ke remote Git: {}...", ws, git_url);
    KnowledgeEngine::link_workspace(&ws, git_url)?;
    println!("✅ Berhasil! Workspace telah terhubung ke Git origin.");
    println!("   Jalankan `aina sync` untuk sinkronisasi otomatis.");
    Ok(())
}
