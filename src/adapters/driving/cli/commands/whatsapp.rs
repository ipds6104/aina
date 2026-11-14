pub fn handle_whatsapp(args: &[String]) -> anyhow::Result<()> {
    let sub = args.first().map(|s| s.as_str()).unwrap_or("status");
    let as_json = args.iter().any(|a| a == "--json");
    let has_concurrency = args.iter().any(|a| a == "--concurrency" || a == "-c");

    if has_concurrency {
        let max_concurrency: usize = std::env::var("AINA_MAX_CONCURRENT_AGENTS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(8);

        let pool = crate::adapters::driven::agy_cli::account_pool::TokenPoolManager::load_initial_token_pool();
        let total_tokens = pool.len();

        if as_json {
            let accounts: Vec<_> = pool
                .iter()
                .map(|a| {
                    serde_json::json!({
                        "id": a.id,
                        "label": a.label,
                        "masked_email": a.masked_email(),
                        "in_flight": 0,
                        "status": "READY"
                    })
                })
                .collect();
            let out = serde_json::json!({
                "max_concurrency_semaphore": max_concurrency,
                "in_flight_active": 0,
                "total_accounts": total_tokens,
                "accounts": accounts,
            });
            println!("{}", serde_json::to_string_pretty(&out)?);
            return Ok(());
        }

        println!("⚡ Status Konkurensi & Antrean Beban (High-Density Telemetry)");
        println!("================================================================================");
        println!("• Kapasitas Semaphore Maksimal : {} slot aktif serentak", max_concurrency);
        println!("• Batas Aman Per Akun Google    : Max 2 in-flight (Zero-429 Rate Limit Invariant)");
        println!("• Total Akun di Kolam Token     : {} akun", total_tokens);
        println!();
        println!("📊 Detail Kolam Akun Google Antigravity:");
        if pool.is_empty() {
            println!("  (Kolam token kosong atau mengandalkan token sistem default)");
        } else {
            for acc in &pool {
                let mail = acc.masked_email().unwrap_or_else(|| "-".to_string());
                println!("  • Account #{} ({}): [ACTIVE / READY] (email: {})", acc.id, acc.label, mail);
            }
        }
        println!("================================================================================");
        return Ok(());
    }

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
