use rusqlite::Connection;

pub fn handle_group(args: &[String]) -> anyhow::Result<()> {
    if args.is_empty() {
        println!(
            r#"Penggunaan manajemen direktori grup WhatsApp & otorisasi lintas-kanal:
    aina group list [--json]
    aina group members <group_jid> [--json]
    aina group user <user_jid> [--json]
    aina group add <group_jid> <user_jid> [--name <nama>] [--role <peran>]
    aina group remove <group_jid> <user_jid>
"#
        );
        return Ok(());
    }

    let config_path = std::env::var("AINA_CONFIG").unwrap_or_else(|_| "config/config.yaml".to_string());
    let config = crate::config::AppConfig::load_from_file_or_default(&config_path);
    let db_path = std::env::var("DATABASE_PATH").unwrap_or(config.database.path);

    let conn = match Connection::open(&db_path) {
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
                "SELECT group_jid, COUNT(user_jid) as member_count, MAX(last_synced_at) as last_sync
                 FROM group_memberships
                 GROUP BY group_jid
                 ORDER BY member_count DESC",
            )?;

            let rows = stmt.query_map([], |row| {
                Ok(serde_json::json!({
                    "group_jid": row.get::<_, String>(0)?,
                    "member_count": row.get::<_, i64>(1)?,
                    "last_synced_at": row.get::<_, Option<String>>(2)?,
                }))
            })?;

            let mut groups = Vec::new();
            for r in rows.flatten() {
                groups.push(r);
            }

            if as_json {
                println!("{}", serde_json::to_string_pretty(&groups)?);
                return Ok(());
            }

            println!("👥 Direktori Grup WhatsApp Terdaftar (Aina Group Directory Cache)");
            println!("================================================================================");
            if groups.is_empty() {
                println!("(Belum ada grup yang tersinkronisasi di direktori lokal)");
            } else {
                for g in &groups {
                    let jid = g["group_jid"].as_str().unwrap_or("-");
                    let count = g["member_count"].as_i64().unwrap_or(0);
                    let sync = g["last_synced_at"].as_str().unwrap_or("-");
                    println!("• {} ({} anggota)", jid, count);
                    println!("  Terakhir Sinkron: {}", sync);
                    println!("--------------------------------------------------------------------------------");
                }
            }
            Ok(())
        }
        "members" => {
            if args.len() < 2 {
                eprintln!("Format salah. Contoh: aina group members 120363253842861469@g.us");
                std::process::exit(1);
            }
            let group_jid = &args[1];
            let as_json = args.iter().any(|a| a == "--json");

            let members = crate::adapters::driven::sqlite_store::session::get_group_members(&conn, group_jid)?;

            if as_json {
                println!("{}", serde_json::to_string_pretty(&members)?);
                return Ok(());
            }

            println!("👥 Daftar Anggota Grup: {}", group_jid);
            println!("================================================================================");
            if members.is_empty() {
                println!("(Tidak ada anggota yang terdaftar untuk grup ini)");
            } else {
                for m in &members {
                    let name = m.user_name.as_deref().unwrap_or("-");
                    println!("• {} ({}) - Peran: {}", name, m.user_jid, m.role_in_group);
                }
                println!("--------------------------------------------------------------------------------");
                println!("Total: {} anggota", members.len());
            }
            Ok(())
        }
        "user" => {
            if args.len() < 2 {
                eprintln!("Format salah. Contoh: aina group user 628123456789@s.whatsapp.net");
                std::process::exit(1);
            }
            let user_jid = &args[1];
            let as_json = args.iter().any(|a| a == "--json");

            let groups = crate::adapters::driven::sqlite_store::session::get_user_groups(&conn, user_jid)?;

            if as_json {
                println!("{}", serde_json::to_string_pretty(&groups)?);
                return Ok(());
            }

            println!("👤 Grup WhatsApp Terdaftar untuk User: {}", user_jid);
            println!("================================================================================");
            if groups.is_empty() {
                println!("(Pengguna ini tidak terdaftar di grup WhatsApp manapun)");
            } else {
                for g in &groups {
                    println!("• {}", g);
                }
            }
            Ok(())
        }
        "add" => {
            if args.len() < 3 {
                eprintln!("Format salah. Contoh: aina group add <group_jid> <user_jid> [--name <nama>] [--role <peran>]");
                std::process::exit(1);
            }
            let group_jid = &args[1];
            let user_jid = &args[2];
            let mut name: Option<String> = None;
            let mut role = "member".to_string();

            let mut i = 3;
            while i < args.len() {
                match args[i].as_str() {
                    "--name" | "-n" if i + 1 < args.len() => {
                        name = Some(args[i + 1].clone());
                        i += 2;
                    }
                    "--role" | "-r" if i + 1 < args.len() => {
                        role = args[i + 1].clone();
                        i += 2;
                    }
                    _ => i += 1,
                }
            }

            crate::adapters::driven::sqlite_store::session::record_group_membership(
                &conn,
                group_jid,
                user_jid,
                name.as_deref(),
                Some(&role),
            )?;

            println!("✅ Anggota `{}` berhasil ditambahkan ke grup `{}`.", user_jid, group_jid);
            Ok(())
        }
        "remove" | "rm" => {
            if args.len() < 3 {
                eprintln!("Format salah. Contoh: aina group remove <group_jid> <user_jid>");
                std::process::exit(1);
            }
            let group_jid = &args[1];
            let user_jid = &args[2];

            conn.execute(
                "DELETE FROM group_memberships WHERE group_jid = ?1 AND user_jid = ?2",
                rusqlite::params![group_jid, user_jid],
            )?;

            println!("✅ Anggota `{}` berhasil dihapus dari grup `{}`.", user_jid, group_jid);
            Ok(())
        }
        _ => {
            eprintln!("Subcommand group tidak dikenal: `{}`. Pilihan: list, members, user, add, remove.", subcmd);
            Ok(())
        }
    }
}
