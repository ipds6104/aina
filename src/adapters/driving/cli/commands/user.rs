pub fn handle_user(args: &[String]) -> anyhow::Result<()> {
    if args.is_empty() {
        println!(
            r#"Penggunaan manajemen profil & wewenang rekan kerja (Aina Profiling Memory):
    aina user list [--json]
    aina user get <jid_atau_nama> [--json]
    aina user set <jid> [--name <nama>] [--role <peran>] [--authority <admin|atasan|staff|guest>] [--notes <catatan>] [--granted-by <jid>]
    aina user search <query> [--json]
    aina user unconfirmed [--json]
    aina user verify <jid> [--name <nama>] [--role <peran>] [--authority <staff|atasan>] [--notes <catatan>] [--by <admin_jid>] [--json]
    aina user reject-claim <jid> [--reason <alasan>] [--by <admin_jid>] [--json]
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

    crate::adapters::driven::sqlite_store::schema::init_schema(&conn)?;

    let subcmd = args[0].as_str();
    match subcmd {
        "list" | "ls" => {
            let as_json = args.iter().any(|a| a == "--json");
            let mut stmt = conn.prepare(
                "SELECT sender_jid, name, role, authority_level, notes, granted_by, updated_at,
                        verification_status, claimed_name, claimed_role, verification_notes
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
                    "granted_by": row.get::<_, Option<String>>(5)?,
                    "updated_at": row.get::<_, Option<String>>(6)?,
                    "verification_status": row.get::<_, Option<String>>(7)?.unwrap_or_else(|| "UNCONFIRMED".to_string()),
                    "claimed_name": row.get::<_, Option<String>>(8)?,
                    "claimed_role": row.get::<_, Option<String>>(9)?,
                    "verification_notes": row.get::<_, Option<String>>(10)?,
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
                    let auth = p["authority_level"].as_str().unwrap_or("guest").to_uppercase();
                    let status = p["verification_status"].as_str().unwrap_or("UNCONFIRMED");
                    let notes = p["notes"].as_str().unwrap_or("-");
                    let granter = p["granted_by"].as_str().unwrap_or("Otomatis/None");
                    println!("• {} ({})", name, jid);
                    println!("  Peran: {} | Otoritas: {} | Status: {} | Izin Oleh: {}", role, auth, status, granter);
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
                "SELECT sender_jid, name, role, authority_level, notes, granted_by, updated_at,
                        verification_status, claimed_name, claimed_role, verification_notes
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
                let granter: Option<String> = row.get(5)?;
                let updated: Option<String> = row.get(6)?;
                let status: String = row.get::<_, Option<String>>(7)?.unwrap_or_else(|| "UNCONFIRMED".to_string());
                let claimed_name: Option<String> = row.get(8)?;
                let claimed_role: Option<String> = row.get(9)?;
                let v_notes: Option<String> = row.get(10)?;

                if as_json {
                    let val = serde_json::json!({
                        "sender_jid": jid,
                        "name": name,
                        "role": role,
                        "authority_level": auth,
                        "verification_status": status,
                        "claimed_name": claimed_name,
                        "claimed_role": claimed_role,
                        "notes": notes,
                        "verification_notes": v_notes,
                        "granted_by": granter,
                        "updated_at": updated,
                    });
                    println!("{}", serde_json::to_string_pretty(&val)?);
                } else {
                    println!("👤 Detail Profil Pengguna");
                    println!("• WhatsApp JID    : {}", jid);
                    println!("• Nama            : {}", name.as_deref().unwrap_or("-"));
                    println!("• Peran / Tim     : {}", role.as_deref().unwrap_or("-"));
                    println!("• Otoritas        : {}", auth.to_uppercase());
                    println!("• Status Identitas: {}", status);
                    if let Some(cn) = claimed_name {
                        println!("• Klaim Nama Asli : {}", cn);
                    }
                    if let Some(cr) = claimed_role {
                        println!("• Klaim Peran     : {}", cr);
                    }
                    println!("• Izin Dari       : {}", granter.as_deref().unwrap_or("Otomatis/None"));
                    println!("• Catatan/Izin    : {}", notes.as_deref().unwrap_or("-"));
                    if let Some(vn) = v_notes {
                        println!("• Catatan Verifikasi: {}", vn);
                    }
                    println!("• Terakhir Update : {}", updated.as_deref().unwrap_or("-"));
                }
            } else {
                println!("⚠️ Profil untuk `{}` tidak ditemukan.", target);
            }
            Ok(())
        }
        "set" => {
            if args.len() < 2 {
                eprintln!("Format salah. Contoh: aina user set 628123456789@s.whatsapp.net --name 'Budi' --role 'Developer' --authority staff --notes 'Diizinkan akses rekap' --granted-by 6289625345646@s.whatsapp.net");
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
            let mut granted_by: Option<String> = None;

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
                    "--granted-by" | "-g" if i + 1 < args.len() => {
                        granted_by = Some(args[i + 1].clone());
                        i += 2;
                    }
                    _ => i += 1,
                }
            }

            let role_hierarchy = authority.clone();

            // `admin` is the Root of Trust and is bound to ADMIN_JID config only.
            // Accepting it here would turn a single CLI call into full escalation.
            if authority.as_deref() == Some("admin") {
                eprintln!(
                    "⛔ Otoritas `admin` tidak dapat diberikan melalui `aina user set`.\n\
                     `admin` hanya berasal dari konfigurasi ADMIN_JID (Root of Trust).\n\
                     Gunakan `aina policy grant-user <jid> --authority atasan|staff` untuk delegasi."
                );
                std::process::exit(2);
            }

            conn.execute(
                "INSERT INTO user_profiles (sender_jid, name, role, authority_level, notes, granted_by, role_hierarchy, updated_at)
                 VALUES (?1, ?2, ?3, COALESCE(?4, 'guest'), ?5, ?6, ?7, CURRENT_TIMESTAMP)
                 ON CONFLICT(sender_jid) DO UPDATE SET
                     name = COALESCE(?2, user_profiles.name),
                     role = COALESCE(?3, user_profiles.role),
                     authority_level = COALESCE(?4, user_profiles.authority_level),
                     notes = COALESCE(?5, user_profiles.notes),
                     granted_by = COALESCE(?6, user_profiles.granted_by),
                     role_hierarchy = COALESCE(?7, user_profiles.role_hierarchy),
                     updated_at = CURRENT_TIMESTAMP",
                rusqlite::params![jid, name, role, authority, notes, granted_by, role_hierarchy],
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
        "unconfirmed" | "pending" => {
            let as_json = args.iter().any(|a| a == "--json");
            let list = crate::adapters::driven::sqlite_store::session::list_unconfirmed_profiles(&conn)?;

            if as_json {
                println!("{}", serde_json::to_string_pretty(&list)?);
                return Ok(());
            }

            println!("📋 Daftar Klaim Identitas Belum Terverifikasi (UNCONFIRMED / TENTATIVE)");
            println!("================================================================================");
            if list.is_empty() {
                println!("(Tidak ada klaim identitas yang menunggu verifikasi)");
            } else {
                for p in &list {
                    let jid = &p.sender_jid;
                    let claimed_name = p.claimed_name.as_deref().or(p.name.as_deref()).unwrap_or("-");
                    let claimed_role = p.claimed_role.as_deref().or(p.role.as_deref()).unwrap_or("-");
                    let status = &p.verification_status;
                    let auth = p.authority_level.to_uppercase();
                    let notes = p.verification_notes.as_deref().or(p.notes.as_deref()).unwrap_or("-");
                    let time_str = if let Some(epoch) = p.claimed_at_epoch {
                        chrono::DateTime::from_timestamp(epoch, 0)
                            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S UTC").to_string())
                            .unwrap_or_else(|| epoch.to_string())
                    } else {
                        "-".to_string()
                    };

                    println!("• JID            : {}", jid);
                    println!("  Klaim Nama     : {}", claimed_name);
                    println!("  Klaim Peran    : {}", claimed_role);
                    println!("  Waktu Klaim    : {}", time_str);
                    println!("  Status Identitas: {} (Otoritas: {})", status, auth);
                    println!("  Catatan        : {}", notes);
                    println!("--------------------------------------------------------------------------------");
                }
            }
            Ok(())
        }
        "verify" => {
            if args.len() < 2 {
                eprintln!("Format salah. Contoh: aina user verify 628123456789@s.whatsapp.net --name 'Budi' --role 'IPDS' --authority staff");
                std::process::exit(1);
            }
            let jid = if args[1].contains('@') {
                args[1].clone()
            } else {
                format!("{}@s.whatsapp.net", args[1])
            };

            let mut name: Option<String> = None;
            let mut role: Option<String> = None;
            let mut authority = "staff".to_string();
            let mut notes: Option<String> = None;
            let mut by = "cli_admin".to_string();
            let as_json = args.iter().any(|a| a == "--json");

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
                        authority = args[i + 1].to_lowercase();
                        i += 2;
                    }
                    "--notes" | "-m" if i + 1 < args.len() => {
                        notes = Some(args[i + 1].clone());
                        i += 2;
                    }
                    "--by" | "-b" if i + 1 < args.len() => {
                        by = args[i + 1].clone();
                        i += 2;
                    }
                    _ => i += 1,
                }
            }

            if authority == "admin" {
                eprintln!(
                    "⛔ Otoritas `admin` tidak dapat diberikan melalui `aina user verify`.\n\
                     `admin` hanya berasal dari konfigurasi ADMIN_JID (Root of Trust)."
                );
                std::process::exit(2);
            }

            crate::adapters::driven::sqlite_store::session::verify_user_profile(
                &conn,
                &jid,
                name.as_deref(),
                role.as_deref(),
                &authority,
                &by,
                notes.as_deref(),
            )?;

            if as_json {
                println!(
                    "{}",
                    serde_json::json!({
                        "status": "VERIFIED",
                        "sender_jid": jid,
                        "name": name,
                        "role": role,
                        "authority_level": authority,
                        "verified_by": by,
                    })
                );
            } else {
                println!("✅ Pengguna `{}` berhasil DIVERIFIKASI (Status: VERIFIED, Otoritas: {}).", jid, authority.to_uppercase());
            }
            Ok(())
        }
        "reject-claim" | "reject" => {
            if args.len() < 2 {
                eprintln!("Format salah. Contoh: aina user reject-claim 628123456789@s.whatsapp.net --reason 'Mencoba mengklaim identitas staf lain'");
                std::process::exit(1);
            }
            let jid = if args[1].contains('@') {
                args[1].clone()
            } else {
                format!("{}@s.whatsapp.net", args[1])
            };

            let mut reason = "Klaim identitas ditolak oleh administrator".to_string();
            let mut by = "cli_admin".to_string();
            let as_json = args.iter().any(|a| a == "--json");

            let mut i = 2;
            while i < args.len() {
                match args[i].as_str() {
                    "--reason" | "-r" if i + 1 < args.len() => {
                        reason = args[i + 1].clone();
                        i += 2;
                    }
                    "--by" | "-b" if i + 1 < args.len() => {
                        by = args[i + 1].clone();
                        i += 2;
                    }
                    _ => i += 1,
                }
            }

            crate::adapters::driven::sqlite_store::session::reject_user_claim(&conn, &jid, &reason, &by)?;

            if as_json {
                println!(
                    "{}",
                    serde_json::json!({
                        "status": "REJECTED",
                        "sender_jid": jid,
                        "reason": reason,
                        "rejected_by": by,
                    })
                );
            } else {
                println!("🚫 Klaim identitas pengguna `{}` berhasil DITOLAK (Status: REJECTED). Alasan: {}", jid, reason);
            }
            Ok(())
        }
        _ => {
            eprintln!("Subcommand user tidak dikenal: `{}`. Pilihan: list, get, set, search, unconfirmed, verify, reject-claim.", subcmd);
            Ok(())
        }
    }
}
