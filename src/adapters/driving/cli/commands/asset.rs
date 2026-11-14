use crate::adapters::driving::cli::workspace_resolver::resolve_workspace;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetManifest {
    #[serde(alias = "asset", default)]
    pub asset_name: String,
    #[serde(default)]
    pub asset_slug: String,
    #[serde(default)]
    pub file_path: Option<String>,
    #[serde(default)]
    pub custodian_jid: String,
    #[serde(default = "default_classification")]
    pub classification: String,
    #[serde(default)]
    pub authorized_users: Vec<String>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

fn default_classification() -> String {
    "RESTRICTED".to_string()
}

pub fn handle_asset(args: &[String]) -> anyhow::Result<()> {
    if args.is_empty() {
        println!(
            r#"Manajemen Hak Asuh Berkas & Dokumen Terbatas (Discretionary Access Control):
    aina asset list [--workspace <ws>] [--json]
    aina asset get <asset_slug> [--workspace <ws>] [--json]
    aina asset grant <asset_slug> <target_jid> [--by <custodian_jid>] [--workspace <ws>] [--json]
    aina asset revoke <asset_slug> <target_jid> [--by <custodian_jid>] [--workspace <ws>] [--json]
"#
        );
        return Ok(());
    }

    let subcmd = args[0].as_str();
    let ws = resolve_workspace(args);
    let as_json = args.iter().any(|a| a == "--json");
    let manifests_dir = ws.join("knowledge").join("manifests");

    match subcmd {
        "list" | "ls" => {
            let assets = load_all_manifests(&manifests_dir)?;
            if as_json {
                println!("{}", serde_json::to_string_pretty(&assets)?);
            } else {
                println!("📂 Berkas & Aset Terdaftar (Knowledge Manifests - DAC):");
                println!("================================================================================");
                if assets.is_empty() {
                    println!("(Belum ada berkas/dataset yang terdaftar di `knowledge/manifests/`)");
                } else {
                    for a in &assets {
                        let users_count = a.authorized_users.len();
                        println!("• {} [{}]", a.asset_name, a.classification);
                        println!("  Slug       : {}", a.asset_slug);
                        println!("  Kustodian  : {}", a.custodian_jid);
                        println!("  Izin Akses : {} pengguna terdaftar", users_count);
                        if let Some(ref desc) = a.description {
                            println!("  Keterangan : {}", desc);
                        }
                        println!("--------------------------------------------------------------------------------");
                    }
                }
            }
            Ok(())
        }
        "get" | "show" => {
            if args.len() < 2 || args[1].starts_with("--") {
                eprintln!("Format salah. Contoh: aina asset get laporan-keuangan-q3");
                std::process::exit(1);
            }
            let slug = &args[1];
            let manifest = load_manifest(&manifests_dir, slug)?;

            if as_json {
                println!("{}", serde_json::to_string_pretty(&manifest)?);
            } else {
                println!("📄 Rincian Hak Asuh Aset: {}", manifest.asset_name);
                println!("================================================================================");
                println!("• Slug          : {}", manifest.asset_slug);
                println!("• Klasifikasi   : {}", manifest.classification);
                println!("• Kustodian     : {}", manifest.custodian_jid);
                if let Some(ref path) = manifest.file_path {
                    println!("• Lokasi Berkas : {}", path);
                }
                if let Some(ref desc) = manifest.description {
                    println!("• Deskripsi     : {}", desc);
                }
                println!("• Pengguna Sah  :");
                for (i, u) in manifest.authorized_users.iter().enumerate() {
                    let badge = if u == &manifest.custodian_jid { " (Kustodian Asli)" } else { "" };
                    println!("  {}. {}{}", i + 1, u, badge);
                }
            }
            Ok(())
        }
        "grant" => {
            if args.len() < 3 || args[1].starts_with("--") || args[2].starts_with("--") {
                eprintln!("Format salah. Contoh: aina asset grant laporan-keuangan-q3 628987654321@s.whatsapp.net [--by <custodian_jid>]");
                std::process::exit(1);
            }
            let slug = &args[1];
            let target_jid = &args[2];
            let by_jid = args.iter().position(|a| a == "--by").and_then(|idx| args.get(idx + 1));

            let mut manifest = load_manifest(&manifests_dir, slug)?;

            // Validation: only custodian or admin can grant
            if let Some(by) = by_jid {
                if by != &manifest.custodian_jid && !by.contains("admin") {
                    eprintln!("⛔ Akses ditolak: Hanya Kustodian Berkas ({}) yang berhak memberikan izin.", manifest.custodian_jid);
                    std::process::exit(2);
                }
            }

            if !manifest.authorized_users.iter().any(|u| u == target_jid) {
                manifest.authorized_users.push(target_jid.clone());
                manifest.updated_at = Some(chrono::Utc::now().to_rfc3339());
                save_manifest(&manifests_dir, &manifest)?;
            }

            if as_json {
                println!(
                    "{}",
                    serde_json::json!({
                        "ok": true,
                        "action": "grant_asset",
                        "asset_slug": slug,
                        "target_jid": target_jid,
                        "authorized_users": manifest.authorized_users
                    })
                );
            } else {
                println!("✅ Izin akses berkas `{}` berhasil diberikan kepada: {}", slug, target_jid);
            }
            Ok(())
        }
        "revoke" => {
            if args.len() < 3 || args[1].starts_with("--") || args[2].starts_with("--") {
                eprintln!("Format salah. Contoh: aina asset revoke laporan-keuangan-q3 628987654321@s.whatsapp.net");
                std::process::exit(1);
            }
            let slug = &args[1];
            let target_jid = &args[2];

            let mut manifest = load_manifest(&manifests_dir, slug)?;

            if target_jid == &manifest.custodian_jid {
                eprintln!("⛔ Tidak dapat mencabut izin dari Kustodian pemilik berkas.");
                std::process::exit(2);
            }

            manifest.authorized_users.retain(|u| u != target_jid);
            manifest.updated_at = Some(chrono::Utc::now().to_rfc3339());
            save_manifest(&manifests_dir, &manifest)?;

            if as_json {
                println!(
                    "{}",
                    serde_json::json!({
                        "ok": true,
                        "action": "revoke_asset",
                        "asset_slug": slug,
                        "revoked_jid": target_jid,
                        "authorized_users": manifest.authorized_users
                    })
                );
            } else {
                println!("🔒 Izin akses berkas `{}` dicabut dari: {}", slug, target_jid);
            }
            Ok(())
        }
        _ => {
            eprintln!("Operasi asset tidak dikenal: `{}`", subcmd);
            std::process::exit(1);
        }
    }
}

fn load_all_manifests(dir: &Path) -> anyhow::Result<Vec<AssetManifest>> {
    let mut list = Vec::new();
    if !dir.is_dir() {
        return Ok(list);
    }

    for entry in fs::read_dir(dir)?.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()).map(|e| e == "yaml" || e == "yml").unwrap_or(false) {
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(mut manifest) = serde_yaml::from_str::<AssetManifest>(&content) {
                    if manifest.asset_slug.is_empty() {
                        manifest.asset_slug = stem;
                    }
                    if manifest.asset_name.is_empty() {
                        manifest.asset_name = manifest.asset_slug.clone();
                    }
                    list.push(manifest);
                }
            }
        }
    }
    list.sort_by(|a, b| a.asset_slug.cmp(&b.asset_slug));
    Ok(list)
}

fn load_manifest(dir: &Path, slug: &str) -> anyhow::Result<AssetManifest> {
    let file_path = find_manifest_path(dir, slug)
        .ok_or_else(|| anyhow::anyhow!("Manifest untuk asset slug `{}` tidak ditemukan di {:?}", slug, dir))?;

    let content = fs::read_to_string(&file_path)?;
    let mut manifest: AssetManifest = serde_yaml::from_str(&content)?;
    if manifest.asset_slug.is_empty() {
        manifest.asset_slug = slug.to_string();
    }
    if manifest.asset_name.is_empty() {
        manifest.asset_name = manifest.asset_slug.clone();
    }
    Ok(manifest)
}

fn save_manifest(dir: &Path, manifest: &AssetManifest) -> anyhow::Result<()> {
    if !dir.is_dir() {
        fs::create_dir_all(dir)?;
    }
    let file_path = find_manifest_path(dir, &manifest.asset_slug)
        .unwrap_or_else(|| dir.join(format!("{}.yaml", manifest.asset_slug)));

    let yaml_str = serde_yaml::to_string(manifest)?;
    fs::write(file_path, yaml_str)?;
    Ok(())
}

fn find_manifest_path(dir: &Path, slug: &str) -> Option<PathBuf> {
    let cand1 = dir.join(format!("{}.yaml", slug));
    if cand1.is_file() {
        return Some(cand1);
    }
    let cand2 = dir.join(format!("{}.yml", slug));
    if cand2.is_file() {
        return Some(cand2);
    }
    None
}
