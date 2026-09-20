use crate::adapters::driving::cli::workspace_resolver::resolve_workspace;
use crate::core::domain::knowledge::GhPollStatus;
use crate::core::domain::KnowledgeEngine;

pub fn handle_gh_status() -> anyhow::Result<()> {
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

pub fn handle_gh_create(args: &[String]) -> anyhow::Result<()> {
    if args.is_empty() {
        eprintln!("Error: Nama repositori GitHub wajib disertakan.");
        eprintln!("Contoh: aina workspace gh-create my-org/knowledge-base [--private]");
        std::process::exit(1);
    }

    let repo_name = &args[0];
    let ws = resolve_workspace(&args[1..]);
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

pub fn handle_gh_login(args: &[String]) -> anyhow::Result<()> {
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

pub fn handle_gh_device(args: &[String]) -> anyhow::Result<()> {
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
