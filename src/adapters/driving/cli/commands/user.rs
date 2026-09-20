pub fn handle_user(args: &[String]) -> anyhow::Result<()> {
    if args.is_empty() {
        println!(
            r#"Penggunaan manajemen profil & wewenang rekan kerja (Aina Profiling Memory):
    aina user list [--json]
    aina user get <jid_atau_nama> [--json]
    aina user set <jid> [--name <nama>] [--role <peran>] [--authority <admin|staff|guest>] [--notes <catatan>]
    aina user search <query> [--json]
"#
        );
        return Ok(());
    }

    let config_path = std::env::var("AINA_CONFIG").unwrap_or_else(|_| "config/config.yaml".to_string());
    let config = crate::config::AppConfig::load_from_file_or_default(&config_path);
    let db_path = std::env::var("DATABASE_PATH").unwrap_or(config.database.path);

    let conn = match rusqlite::Connection::open(&db_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("⚠️ Tidak dapat membuka database di {}: {}", db_path, e);
            std::process::exit(1);
        }
    };

    conn.execute(
        "CREATE TABLE IF NOT EXISTS user_profiles (
            sender_jid TEXT PRIMARY KEY,
            name TEXT,
            role TEXT,
            authority_level TEXT NOT NULL DEFAULT 'staff',
            notes TEXT,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
        )",
        [],
    )?;

    let subcmd = args[0].as_str();
    match subcmd {
        "list" | "ls" => {
            let as_json = args.iter().any(|a| a == "--json");
            let mut stmt = conn.prepare(
                "SELECT sender_jid, name, role, authority_level, notes, updated_at
                 FROM user_profiles
                 ORDER BY updated_at DESC",
            )?;

            let rows = stmt.query_map([], |row| {
                Ok(serde_json::json!({
                    "sender_jid": row.get::<_, String>(0)?,
                    "name": row.get::<_, Option<String>>(1)?,
                    "role": row.get::<_, Option<String>>(2)?,
                    "authority_level": row.get::<_, String>(3)?,
                    "notes": row.get::<_, Option<String>>(4)?,
                    "updated_at": row.get::<_, Option<String>>(5)?,
                }))
            })?;

            let mut profiles = Vec::new();
            for r in rows.flatten() {
                profiles.push(r);
            }

            if as_json {
                println!("{}", serde_json::to_string_pretty(&profiles)?);
                return Ok(());
            }

            println!("👤 Profil Pengguna Terdaftar (Aina Profiling Memory & Access Control)");
            println!("================================================================================");
            if profiles.is_empty() {
                println!("(Belum ada profil pengguna tersimpan)");
            } else {
                for p in &profiles {
                    let jid = p["sender_jid"].as_str().unwrap_or("-");
                    let name = p["name"].as_str().unwrap_or("-");
                    let role = p["role"].as_str().unwrap_or("-");
                    let auth = p["authority_level"].as_str().unwrap_or("staff").to_uppercase();
                    let notes = p["notes"].as_str().unwrap_or("-");
                    println!("• {} ({})", name, jid);
                    println!("  Peran: {} | Otoritas: {}", role, auth);
                    println!("  Catatan / Izin: {}", notes);
                    println!("--------------------------------------------------------------------------------");
                }
            }
            Ok(())
        }
        "get" => {
            if args.len() < 2 {
                eprintln!("Format salah. Contoh: aina user get 628123456789@s.whatsapp.net");
                std::process::exit(1);
            }
            let target = &args[1];
            let as_json = args.iter().any(|a| a == "--json");
            let target_like = format!("%{}%", target);

            let mut stmt = conn.prepare(
                "SELECT sender_jid, name, role, authority_level, notes, updated_at
                 FROM user_profiles
                 WHERE sender_jid = ?1 OR sender_jid LIKE ?2 OR name LIKE ?2
                 LIMIT 1",
            )?;

            let mut rows = stmt.query(rusqlite::params![target, target_like])?;
            if let Some(row) = rows.next()? {
                let jid: String = row.get(0)?;
                let name: Option<String> = row.get(1)?;
                let role: Option<String> = row.get(2)?;
                let auth: String = row.get(3)?;
                let notes: Option<String> = row.get(4)?;
                let updated: Option<String> = row.get(5)?;

                if as_json {
                    let val = serde_json::json!({
                        "sender_jid": jid,
                        "name": name,
                        "role": role,
                        "authority_level": auth,
                        "notes": notes,
                        "updated_at": updated,
                    });
                    println!("{}", serde_json::to_string_pretty(&val)?);
                } else {
                    println!("👤 Detail Profil Pengguna");
                    println!("• WhatsApp JID : {}", jid);
                    println!("• Nama         : {}", name.as_deref().unwrap_or("-"));
                    println!("• Peran / Tim  : {}", role.as_deref().unwrap_or("-"));
                    println!("• Otoritas     : {}", auth.to_uppercase());
                    println!("• Catatan/Izin : {}", notes.as_deref().unwrap_or("-"));
                    println!("• Terakhir Update: {}", updated.as_deref().unwrap_or("-"));
                }
            } else {
                println!("⚠️ Profil untuk `{}` tidak ditemukan.", target);
            }
            Ok(())
        }
        "set" => {
            if args.len() < 2 {
                eprintln!("Format salah. Contoh: aina user set 628123456789@s.whatsapp.net --name 'Budi' --role 'Developer' --authority staff --notes 'Diizinkan akses rekap'");
                std::process::exit(1);
            }
            let jid = if args[1].contains('@') {
                args[1].clone()
            } else {
                format!("{}@s.whatsapp.net", args[1])
            };

            let mut name: Option<String> = None;
            let mut role: Option<String> = None;
            let mut authority: Option<String> = None;
            let mut notes: Option<String> = None;

            let mut i = 2;
            while i < args.len() {
                match args[i].as_str() {
                    "--name" | "-n" if i + 1 < args.len() => {
                        name = Some(args[i + 1].clone());
                        i += 2;
                    }
                    "--role" | "-r" if i + 1 < args.len() => {
                        role = Some(args[i + 1].clone());
                        i += 2;
                    }
                    "--authority" | "-a" if i + 1 < args.len() => {
                        authority = Some(args[i + 1].to_lowercase());
                        i += 2;
                    }
                    "--notes" | "-m" if i + 1 < args.len() => {
                        notes = Some(args[i + 1].clone());
                        i += 2;
                    }
                    _ => i += 1,
                }
            }

            conn.execute(
                "INSERT INTO user_profiles (sender_jid, name, role, authority_level, notes, updated_at)
                 VALUES (?1, ?2, ?3, COALESCE(?4, 'staff'), ?5, CURRENT_TIMESTAMP)
                 ON CONFLICT(sender_jid) DO UPDATE SET
                     name = COALESCE(?2, user_profiles.name),
                     role = COALESCE(?3, user_profiles.role),
                     authority_level = COALESCE(?4, user_profiles.authority_level),
                     notes = COALESCE(?5, user_profiles.notes),
                     updated_at = CURRENT_TIMESTAMP",
                rusqlite::params![jid, name, role, authority, notes],
            )?;

            println!("✅ Profil pengguna `{}` berhasil diperbarui di Aina Profiling Memory.", jid);
            Ok(())
        }
        "search" | "find" => {
            if args.len() < 2 {
                eprintln!("Format salah. Contoh: aina user search 'Budi'");
                std::process::exit(1);
            }
            let q = &args[1];
            let as_json = args.iter().any(|a| a == "--json");
            let like_expr = format!("%{}%", q);

            let mut stmt = conn.prepare(
                "SELECT sender_jid, name, role, authority_level, notes, updated_at
                 FROM user_profiles
                 WHERE sender_jid LIKE ?1 OR name LIKE ?1 OR role LIKE ?1 OR notes LIKE ?1
                 ORDER BY updated_at DESC",
            )?;

            let rows = stmt.query_map(rusqlite::params![like_expr], |row| {
                Ok(serde_json::json!({
                    "sender_jid": row.get::<_, String>(0)?,
                    "name": row.get::<_, Option<String>>(1)?,
                    "role": row.get::<_, Option<String>>(2)?,
                    "authority_level": row.get::<_, String>(3)?,
                    "notes": row.get::<_, Option<String>>(4)?,
                    "updated_at": row.get::<_, Option<String>>(5)?,
                }))
            })?;

            let mut matches = Vec::new();
            for r in rows.flatten() {
                matches.push(r);
            }

            if as_json {
                println!("{}", serde_json::to_string_pretty(&matches)?);
                return Ok(());
            }

            println!("🔍 Hasil Pencarian Profil untuk kata kunci `{}` (Ditemukan: {})", q, matches.len());
            println!("================================================================================");
            for m in &matches {
                let jid = m["sender_jid"].as_str().unwrap_or("-");
                let name = m["name"].as_str().unwrap_or("-");
                let role = m["role"].as_str().unwrap_or("-");
                let auth = m["authority_level"].as_str().unwrap_or("staff").to_uppercase();
                let notes = m["notes"].as_str().unwrap_or("-");
                println!("• {} ({})", name, jid);
                println!("  Peran: {} | Otoritas: {}", role, auth);
                println!("  Catatan / Izin: {}", notes);
                println!("--------------------------------------------------------------------------------");
            }
            Ok(())
        }
        _ => {
            eprintln!("Subcommand user tidak dikenal: `{}`. Pilihan: list, get, set, search.", subcmd);
            Ok(())
        }
    }
}
