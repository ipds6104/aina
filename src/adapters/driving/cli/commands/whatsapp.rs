pub fn handle_whatsapp(args: &[String]) -> anyhow::Result<()> {
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
