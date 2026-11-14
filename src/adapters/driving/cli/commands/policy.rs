//! `aina policy` — the sanctioned way for the agent to *propose* an
//! authorization change.
//!
//! Division of labour:
//!   * the **agent** understands intent ("semua di grup ini rekan kerja BPS,
//!     bantuin ya") and decides *which* action to propose;
//!   * the **core** decides whether that action is allowed, using the
//!     short-lived [`TurnCapability`] minted for the verified WhatsApp sender
//!     of the current turn.
//!
//! The agent can therefore never self-authorize: without a capability that
//! resolves to `admin`, every mutating subcommand is refused. `admin` itself is
//! bound to the configured Companion JID and cannot be granted through here.

use crate::core::domain::capability::{
    evaluate, Authority, PolicyAction, PolicyDecision, TurnCapability,
};
use rusqlite::Connection;
use serde_json::json;
use std::io::IsTerminal;

pub fn handle_policy(args: &[String]) -> anyhow::Result<()> {
    if args.is_empty() || matches!(args[0].as_str(), "help" | "--help" | "-h") {
        print_help();
        return Ok(());
    }

    let flags = Flags::parse(args);
    let sub = flags.positional[0].as_str();
    let as_json = flags.json;

    let conn = match open_db() {
        Ok(c) => c,
        Err(e) => {
            emit(
                as_json,
                json!({ "ok": false, "reason": "database_unavailable", "detail": e.to_string() }),
                format!("❌ Tidak dapat membuka database: {}", e),
            );
            return Ok(());
        }
    };

    if sub == "status" {
        return cmd_status(&conn, &flags);
    }

    // Every mutating subcommand needs a verified principal.
    let capability = match resolve_capability(&conn, &flags) {
        Ok(Some(c)) => c,
        Ok(None) => {
            emit(
                as_json,
                json!({
                    "ok": false,
                    "action": sub,
                    "reason": "missing_turn_capability",
                    "hint": "Perubahan otorisasi hanya boleh diajukan dari dalam percakapan WhatsApp yang terverifikasi. Jangan mengarang hasil; sampaikan bahwa akses belum berubah."
                }),
                "⛔ *Tidak ada kredensial tur yang valid.*\n\nPerubahan otorisasi tidak dapat dilakukan dari konteks ini, dan tidak ada data yang diubah.".to_string(),
            );
            return Ok(());
        }
        Err(e) => {
            emit(
                as_json,
                json!({ "ok": false, "action": sub, "reason": "capability_lookup_failed", "detail": e.to_string() }),
                format!("❌ Gagal memvalidasi kredensial tur: {}", e),
            );
            return Ok(());
        }
    };

    let now = chrono::Utc::now().timestamp();
    let action = match build_action(sub, &flags, &capability) {
        Ok(a) => a,
        Err(msg) => {
            emit(
                as_json,
                json!({ "ok": false, "action": sub, "reason": "invalid_arguments", "detail": msg }),
                format!("⚠️ {}", msg),
            );
            return Ok(());
        }
    };

    let decision = evaluate(&capability, &action, now);
    if !decision.allowed {
        emit_denied(as_json, &capability, &action, &decision);
        return Ok(());
    }

    let applied = match apply(&conn, &action, &capability, &flags) {
        Ok(v) => v,
        Err(e) => {
            emit(
                as_json,
                json!({ "ok": false, "action": action.kind(), "reason": "apply_failed", "detail": e.to_string() }),
                format!("❌ Otorisasi disetujui tetapi gagal disimpan: {}", e),
            );
            return Ok(());
        }
    };

    emit(as_json, applied.0, applied.1);
    Ok(())
}

// ---------------------------------------------------------------------------
// Subcommands
// ---------------------------------------------------------------------------

fn cmd_status(conn: &Connection, flags: &Flags) -> anyhow::Result<()> {
    let as_json = flags.json;
    let capability = resolve_capability(conn, flags).unwrap_or(None);

    let Some(cap) = capability else {
        emit(
            as_json,
            json!({
                "ok": true,
                "authenticated": false,
                "reason": "no_turn_capability",
                "hint": "Tidak ada capability tur. Aksi yang mengubah otorisasi akan ditolak."
            }),
            "ℹ️ *Status Kebijakan Aina*\n\nTidak ada capability tur yang aktif pada konteks ini.\nAksi yang mengubah otorisasi (trust/untrust grup, grant/revoke user) akan ditolak.".to_string(),
        );
        return Ok(());
    };

    let now = chrono::Utc::now().timestamp();
    let remaining = (cap.expires_at - now).max(0);
    let chat_trusted = crate::adapters::driven::sqlite_store::session::is_group_trusted(
        conn,
        &cap.chat_jid,
    )
    .unwrap_or(false);
    let profile = crate::adapters::driven::sqlite_store::session::get_user_profile(
        conn,
        &cap.principal_jid,
    )
    .ok()
    .flatten();

    let payload = json!({
        "ok": true,
        "authenticated": true,
        "principal_jid": cap.principal_jid,
        "authority": cap.authority.as_str(),
        "can_delegate": cap.authority.can_delegate(),
        "can_access_internal": cap.authority.can_access_internal(),
        "chat_jid": cap.chat_jid,
        "chat_type": cap.chat_type,
        "expires_in_seconds": remaining,
        "expired": cap.is_expired(now),
        "chat_is_trusted_group": if cap.chat_type == "group" { Some(chat_trusted) } else { None },
        "principal_profile": profile.as_ref().map(|p| json!({
            "name": p.name,
            "role": p.role,
            "authority_level": p.authority_level,
            "granted_by": p.granted_by,
        })),
    });

    let human = format!(
        "🔎 *Status Kebijakan Tur Ini*\n\n• *Pengirim terverifikasi*: {}\n• *Otoritas*: *{}*\n• *Boleh mendelegasikan*: {}\n• *Chat*: {} ({})\n• *Status grup ini*: {}\n• *Berlaku*: {} detik lagi\n\n{}",
        cap.principal_jid,
        cap.authority.as_str().to_uppercase(),
        if cap.authority.can_delegate() { "Ya" } else { "Tidak" },
        cap.chat_jid,
        cap.chat_type,
        if cap.chat_type == "group" {
            if chat_trusted { "TERPERCAYA (tim kerja)" } else { "umum (Zero Implicit Trust)" }
        } else {
            "-"
        },
        remaining,
        if cap.authority.can_delegate() {
            "Kamu boleh mengajukan perubahan via `aina policy trust-group|untrust-group|grant-user|revoke-user`."
        } else {
            "Kamu TIDAK berwenang mengubah otorisasi siapa pun. Eskalasi ke Admin/Companion."
        }
    );

    emit(as_json, payload, human);
    Ok(())
}

fn build_action(
    sub: &str,
    flags: &Flags,
    cap: &TurnCapability,
) -> Result<PolicyAction, String> {
    match sub {
        "trust-group" | "trust" => Ok(PolicyAction::TrustGroup {
            group_jid: resolve_group_jid(flags, cap)?,
        }),
        "untrust-group" | "untrust" => Ok(PolicyAction::UntrustGroup {
            group_jid: resolve_group_jid(flags, cap)?,
        }),
        "grant-user" | "grant" => {
            let raw = flags.positional.get(1).ok_or_else(|| {
                "Format: `aina policy grant-user <whatsapp_jid> [--authority staff|atasan] [--role <peran>] [--notes <catatan>]`".to_string()
            })?;
            let user_jid = normalize_user_jid(raw);
            if user_jid.is_empty() {
                return Err("JID pengguna tidak valid.".to_string());
            }
            let authority = Authority::parse(flags.authority.as_deref().unwrap_or("staff"));
            Ok(PolicyAction::GrantUser { user_jid, authority })
        }
        "revoke-user" | "revoke" => {
            let raw = flags.positional.get(1).ok_or_else(|| {
                "Format: `aina policy revoke-user <whatsapp_jid>`".to_string()
            })?;
            let user_jid = normalize_user_jid(raw);
            if user_jid.is_empty() {
                return Err("JID pengguna tidak valid.".to_string());
            }
            Ok(PolicyAction::RevokeUser { user_jid })
        }
        other => Err(format!(
            "Subcommand tidak dikenal: `{}`. Gunakan: status | trust-group | untrust-group | grant-user | revoke-user.",
            other
        )),
    }
}

/// Inside a group the agent may only touch that same group (enforced again by
/// `evaluate`). In a DM the Companion must name the group explicitly.
fn resolve_group_jid(flags: &Flags, cap: &TurnCapability) -> Result<String, String> {
    if let Some(raw) = flags.positional.get(1) {
        let jid = normalize_group_jid(raw);
        if jid.is_empty() {
            return Err("JID grup tidak valid.".to_string());
        }
        return Ok(jid);
    }
    if cap.chat_type == "group" && !cap.chat_jid.trim().is_empty() {
        return Ok(cap.chat_jid.clone());
    }
    Err("Sebutkan JID grup, contoh: `aina policy trust-group 120363...@g.us`".to_string())
}

// ---------------------------------------------------------------------------
// Applying an allowed action
// ---------------------------------------------------------------------------

fn apply(
    conn: &Connection,
    action: &PolicyAction,
    cap: &TurnCapability,
    flags: &Flags,
) -> anyhow::Result<(serde_json::Value, String)> {
    let granted_by = cap.principal_jid.clone();

    match action {
        PolicyAction::TrustGroup { group_jid } => {
            crate::adapters::driven::sqlite_store::session::set_group_trusted(
                conn,
                group_jid,
                true,
                Some(&granted_by),
            )?;
            Ok((
                json!({
                    "ok": true,
                    "action": "trust_group",
                    "group_jid": group_jid,
                    "group_trusted": true,
                    "granted_by": granted_by,
                    "reason": flags.reason,
                    "message": "Grup ini sekarang terverifikasi sebagai grup tim kerja."
                }),
                format!(
                    "✅ *Grup Tim Kerja Terverifikasi*\n\n• *Grup*: {}\n• *Diverifikasi oleh*: {}\n\nMulai sekarang anggota grup ini diakui sebagai rekan kerja (staf) saat berinteraksi dengan Aina. Data internal tetap hanya dapat diakses oleh staf yang terverifikasi.",
                    group_jid, granted_by
                ),
            ))
        }
        PolicyAction::UntrustGroup { group_jid } => {
            crate::adapters::driven::sqlite_store::session::set_group_trusted(
                conn,
                group_jid,
                false,
                Some(&granted_by),
            )?;
            Ok((
                json!({
                    "ok": true,
                    "action": "untrust_group",
                    "group_jid": group_jid,
                    "group_trusted": false,
                    "granted_by": granted_by,
                    "reason": flags.reason,
                }),
                format!(
                    "🔒 *Kepercayaan Grup Dicabut*\n\n• *Grup*: {}\n• *Dicabut oleh*: {}\n\nGrup ini kembali berstatus umum (Zero Implicit Trust).",
                    group_jid, granted_by
                ),
            ))
        }
        PolicyAction::GrantUser { user_jid, authority } => {
            let role = flags
                .role
                .clone()
                .unwrap_or_else(|| default_role(*authority).to_string());
            let notes = flags
                .notes
                .clone()
                .or_else(|| flags.reason.clone())
                .unwrap_or_else(|| "Diberikan melalui percakapan WhatsApp".to_string());
            crate::adapters::driven::sqlite_store::session::authorize_user(
                conn,
                user_jid,
                &role,
                authority.as_str(),
                Some(&notes),
                &granted_by,
            )?;
            Ok((
                json!({
                    "ok": true,
                    "action": "grant_user",
                    "user_jid": user_jid,
                    "authority": authority.as_str(),
                    "role": role,
                    "notes": notes,
                    "granted_by": granted_by,
                }),
                format!(
                    "✅ *Otorisasi Rekan Kerja Disimpan*\n\n• *Pengguna*: {}\n• *Peran*: {}\n• *Otoritas*: *{}*\n• *Diberikan oleh*: {}\n\nAkses ini berlaku untuk tugas koordinasi kerja, bukan akses administrator.",
                    user_jid, role, authority.as_str().to_uppercase(), granted_by
                ),
            ))
        }
        PolicyAction::RevokeUser { user_jid } => {
            crate::adapters::driven::sqlite_store::session::authorize_user(
                conn,
                user_jid,
                "Tamu",
                "guest",
                Some("Otorisasi dicabut melalui percakapan WhatsApp"),
                &granted_by,
            )?;
            Ok((
                json!({
                    "ok": true,
                    "action": "revoke_user",
                    "user_jid": user_jid,
                    "authority": "guest",
                    "granted_by": granted_by,
                }),
                format!(
                    "🔒 *Otorisasi Dicabut*\n\n• *Pengguna*: {}\n• *Status baru*: *GUEST (Tamu)*\n• *Dicabut oleh*: {}",
                    user_jid, granted_by
                ),
            ))
        }
        PolicyAction::AccessInternalData => Ok((
            json!({ "ok": true, "action": "access_internal_data", "authority": cap.authority.as_str() }),
            "✅ Akses data internal diizinkan.".to_string(),
        )),
    }
}

fn default_role(authority: Authority) -> &'static str {
    match authority {
        Authority::Admin => "Administrator Sistem",
        Authority::Atasan => "Pimpinan / Atasan Organisasi",
        Authority::Staff => "Rekan Tim",
        Authority::Guest => "Tamu",
    }
}

fn emit_denied(
    as_json: bool,
    cap: &TurnCapability,
    action: &PolicyAction,
    decision: &PolicyDecision,
) {
    let (human_reason, escalation) = match decision.reason.as_str() {
        "only_admin_may_change_group_trust" | "only_admin_may_delegate" | "only_admin_may_revoke" => (
            "Hanya Admin/Companion (Root of Trust) yang boleh mengubah otorisasi.",
            true,
        ),
        "group_scope_violation" => (
            "Di dalam grup, kamu hanya boleh mengubah status grup ini sendiri.",
            false,
        ),
        "capability_expired" => (
            "Capability tur ini sudah kedaluwarsa. Minta pengguna mengirim ulang pesannya.",
            false,
        ),
        "admin_is_not_delegable" => (
            "Otoritas `admin` adalah konfigurasi sistem dan tidak bisa diberikan lewat tool.",
            false,
        ),
        other => (other, false),
    };

    emit(
        as_json,
        json!({
            "ok": false,
            "action": action.kind(),
            "reason": decision.reason,
            "principal_jid": cap.principal_jid,
            "principal_authority": cap.authority.as_str(),
            "requires_escalation": escalation,
            "message": human_reason,
        }),
        format!(
            "⛔ *Perubahan Otorisasi Ditolak*\n\n• *Aksi*: {}\n• *Otoritas pengirim*: *{}*\n• *Alasan*: {}\n\nTidak ada data yang diubah.{}",
            action.kind(),
            cap.authority.as_str().to_uppercase(),
            human_reason,
            if escalation {
                "\n\nSilakan sampaikan ke Bg Ihza (Admin/Companion) untuk diputuskan."
            } else {
                ""
            }
        ),
    );
}

// ---------------------------------------------------------------------------
// Capability resolution
// ---------------------------------------------------------------------------

/// Resolves the acting principal.
///
/// Two sources only:
/// 1. `AINA_TURN_TOKEN` injected into the agent process by the daemon — the
///    normal agentic path, bound to the verified WhatsApp sender.
/// 2. Explicit local operator mode (`--operator` from a real TTY), for a human
///    at the server shell.
///
/// Absence of the env var is *never* treated as operator mode: that would let
/// the agent bypass everything with `env -u AINA_TURN_TOKEN`.
fn resolve_capability(
    conn: &Connection,
    flags: &Flags,
) -> anyhow::Result<Option<TurnCapability>> {
    if flags.operator {
        let is_term = (std::io::stdin().is_terminal() && std::io::stdout().is_terminal())
            && std::env::var("AINA_FORCE_NON_TERMINAL").is_err();
        if !is_term {
            anyhow::bail!(
                "mode --operator hanya tersedia pada sesi terminal interaktif (bukan proses non-interaktif/agen)"
            );
        }
        let now = chrono::Utc::now().timestamp();
        let mut cap = TurnCapability::mint(
            "local:operator",
            Authority::Admin,
            "",
            "direct",
            "operator_session",
            now,
        );
        // Operator shells are human-driven; keep the window short regardless.
        cap.expires_at = now + 60;
        return Ok(Some(cap));
    }

    let token = std::env::var("AINA_TURN_TOKEN").unwrap_or_default();
    if token.trim().is_empty() {
        return Ok(None);
    }

    let cap = crate::adapters::driven::sqlite_store::session::get_turn_capability(conn, &token)?;
    Ok(cap)
}

// ---------------------------------------------------------------------------
// Plumbing
// ---------------------------------------------------------------------------

fn open_db() -> anyhow::Result<Connection> {
    let config_path =
        std::env::var("AINA_CONFIG").unwrap_or_else(|_| "config/config.yaml".to_string());
    let config = crate::config::AppConfig::load_from_file_or_default(&config_path);
    let db_path = std::env::var("DATABASE_PATH").unwrap_or(config.database.path);

    let conn = Connection::open(&db_path)
        .map_err(|e| anyhow::anyhow!("gagal membuka {}: {}", db_path, e))?;
    // The daemon holds its own connection. Without a busy timeout, a write from
    // this CLI during a daemon write fails instantly with SQLITE_BUSY.
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    crate::adapters::driven::sqlite_store::schema::init_schema(&conn)?;
    Ok(conn)
}

fn normalize_user_jid(raw: &str) -> String {
    let cleaned = raw
        .trim()
        .trim_matches(|c: char| c == '@' || c == ',' || c == '.')
        .to_string();
    if cleaned.is_empty() {
        return String::new();
    }
    if cleaned.contains('@') {
        return cleaned;
    }
    let digits: String = cleaned.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        String::new()
    } else {
        format!("{}@s.whatsapp.net", digits)
    }
}

fn normalize_group_jid(raw: &str) -> String {
    let cleaned = raw.trim().to_string();
    if cleaned.is_empty() {
        return String::new();
    }
    if cleaned.contains('@') {
        return cleaned;
    }
    let digits: String = cleaned.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        String::new()
    } else {
        format!("{}@g.us", digits)
    }
}

fn emit(as_json: bool, payload: serde_json::Value, human: String) {
    if as_json {
        match serde_json::to_string_pretty(&payload) {
            Ok(s) => println!("{}", s),
            Err(_) => println!("{}", payload),
        }
    } else {
        println!("{}", human);
    }
}

fn print_help() {
    println!(
        r#"Penggunaan `aina policy` (gerbang otorisasi deterministik):

    aina policy status [--json]
    aina policy trust-group [<group_jid>] [--reason <teks>] [--json]
    aina policy untrust-group [<group_jid>] [--json]
    aina policy grant-user <user_jid> [--authority staff|atasan] [--role <peran>] [--notes <catatan>] [--json]
    aina policy revoke-user <user_jid> [--json]

Sumber kewenangan:
    • Agen: token `AINA_TURN_TOKEN` yang disuntikkan daemon per-tur, terikat pada
      pengirim WhatsApp yang terverifikasi.
    • Operator lokal: `--operator` dari terminal interaktif.

Catatan:
    • Tanpa capability yang valid, semua aksi yang mengubah otorisasi DITOLAK.
    • `admin` hanya berasal dari konfigurasi (Companion JID), tidak pernah via tool.
    • Perubahan otorisasi hanya boleh dilakukan jika output berisi `"ok": true`.
"#
    );
}

#[derive(Default)]
struct Flags {
    json: bool,
    operator: bool,
    reason: Option<String>,
    role: Option<String>,
    notes: Option<String>,
    authority: Option<String>,
    positional: Vec<String>,
}

impl Flags {
    fn parse(args: &[String]) -> Self {
        let mut f = Flags::default();
        let mut i = 0;
        while i < args.len() {
            let a = args[i].as_str();
            match a {
                "--json" | "-j" => {
                    f.json = true;
                    i += 1;
                }
                "--operator" | "--local" => {
                    f.operator = true;
                    i += 1;
                }
                "--reason" | "-m" if i + 1 < args.len() => {
                    f.reason = Some(args[i + 1].clone());
                    i += 2;
                }
                "--role" | "-r" if i + 1 < args.len() => {
                    f.role = Some(args[i + 1].clone());
                    i += 2;
                }
                "--notes" | "-n" if i + 1 < args.len() => {
                    f.notes = Some(args[i + 1].clone());
                    i += 2;
                }
                "--authority" | "-a" if i + 1 < args.len() => {
                    f.authority = Some(args[i + 1].clone());
                    i += 2;
                }
                other => {
                    f.positional.push(other.to_string());
                    i += 1;
                }
            }
        }
        f
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_jid_normalization() {
        assert_eq!(normalize_user_jid("628123456789"), "628123456789@s.whatsapp.net");
        assert_eq!(normalize_user_jid("@628123456789"), "628123456789@s.whatsapp.net");
        assert_eq!(
            normalize_user_jid("628123456789@s.whatsapp.net"),
            "628123456789@s.whatsapp.net"
        );
        assert_eq!(normalize_user_jid("   "), "");
        assert_eq!(normalize_user_jid("abc"), "");
    }

    #[test]
    fn group_jid_normalization() {
        assert_eq!(normalize_group_jid("120363253842861469"), "120363253842861469@g.us");
        assert_eq!(
            normalize_group_jid("120363253842861469@g.us"),
            "120363253842861469@g.us"
        );
    }

    #[test]
    fn flags_parse_values_and_positionals() {
        let args: Vec<String> = [
            "grant-user",
            "628111@s.whatsapp.net",
            "--authority",
            "staff",
            "--json",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let f = Flags::parse(&args);
        assert!(f.json);
        assert_eq!(f.positional, vec!["grant-user", "628111@s.whatsapp.net"]);
        assert_eq!(f.authority.as_deref(), Some("staff"));
    }

    /// A non-interactive process must never be able to claim operator mode.
    #[test]
    fn operator_mode_requires_a_terminal() {
        // SAFETY: single-threaded unit test manipulating a process-global var.
        unsafe { std::env::set_var("AINA_FORCE_NON_TERMINAL", "1") };
        let args: Vec<String> = ["status", "--operator"].iter().map(|s| s.to_string()).collect();
        let f = Flags::parse(&args);
        assert!(f.operator);
        let res = resolve_capability(&Connection::open_in_memory().unwrap(), &f);
        unsafe { std::env::remove_var("AINA_FORCE_NON_TERMINAL") };
        assert!(res.is_err());
    }

    /// Absence of the env token is a hard "no capability", never operator mode.
    #[test]
    fn missing_token_yields_no_capability() {
        // SAFETY: single-threaded unit test manipulating a process-global var.
        unsafe { std::env::remove_var("AINA_TURN_TOKEN") };
        let args: Vec<String> = ["trust-group"].iter().map(|s| s.to_string()).collect();
        let f = Flags::parse(&args);
        let conn = Connection::open_in_memory().unwrap();
        assert!(resolve_capability(&conn, &f).unwrap().is_none());
    }
}
