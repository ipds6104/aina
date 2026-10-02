#!/usr/bin/env python3
"""
Aina Interlocutor & User Profiling Utility (Aina Profiling Memory)
Manages user profiles, role assignments, authority levels, and conversational preferences.
"""

import sys
import os
import sqlite3
import json
import re
import argparse
from typing import Optional, Dict, Any

def get_db_path() -> str:
    db_env = os.environ.get("DATABASE_PATH")
    if db_env and os.path.exists(db_env):
        return db_env
    # Check default paths
    candidates = [
        "data/aina.db",
        "/root/projects/aina/data/aina.db",
        "/app/data/aina.db",
        "workspaces/default/data/aina.db",
    ]
    for cand in candidates:
        if os.path.exists(cand):
            return cand
    return "data/aina.db"

def init_db(conn: sqlite3.Connection):
    conn.execute("""
        CREATE TABLE IF NOT EXISTS user_profiles (
            sender_jid TEXT PRIMARY KEY,
            name TEXT,
            role TEXT,
            authority_level TEXT NOT NULL DEFAULT 'staff',
            notes TEXT,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
        )
    """)
    conn.commit()

def extract_entities_from_text(text: str) -> Dict[str, Optional[str]]:
    """Extracts name, callsign, role, and department from conversational text."""
    res = {
        "name": None,
        "callsign": None,
        "role": None,
        "notes": None
    }
    trimmed = text.strip()
    if not trimmed or "?" in trimmed:
        return res

    lower = trimmed.lower()

    # 1. Callsign
    callsign_patterns = [
        r"(?:panggil saja|panggil aku|panggil saya|panggil gue|panggil gw|panggil)\s+([A-Za-z0-9\s]+?)(?:ya|yaa|deh|aja|saja|[.,;!\n-]|$)",
    ]
    for pat in callsign_patterns:
        m = re.search(pat, lower)
        if m:
            raw = m.group(1).strip()
            words = [w.capitalize() for w in raw.split() if w.lower() not in {"ya", "yaa", "deh", "aja", "saja"}]
            if words and len(words) <= 3:
                res["callsign"] = " ".join(words)
                res["name"] = " ".join(words)
                break

    # 2. Name
    name_patterns = [
        r"(?:nama saya|namaku|nama ku|nama gue|nama gw|kenalkan,?\s*saya|perkenalkan,?\s*saya|kenalan,?\s*saya|halo aina,?\s*saya|hai aina,?\s*saya)\s+([A-Za-z0-9\s]+?)(?:ya|yaa|dari|staf|sebagai|[.,;!\n-]|$)",
    ]
    for pat in name_patterns:
        m = re.search(pat, lower)
        if m:
            raw = m.group(1).strip()
            words = [w.capitalize() for w in raw.split() if w.lower() not in {"ya", "yaa", "deh", "aja", "saja"}]
            if words and len(words) <= 4:
                cand_name = " ".join(words)
                if not res["name"]:
                    res["name"] = cand_name
                break

    # 3. Role / Department
    role_patterns = [
        r"(?:saya staf|saya bagian|saya divisi|saya tim|saya seksi|staf bagian|staf divisi|staf seksi|staf|bagian|divisi|seksi|tim)\s+([A-Za-z0-9\s]+?)(?:ya|yaa|salam|kenal|[.,;!\n-]|$)",
        r"(?:saya dari)\s+([A-Za-z0-9\s]+?)(?:ya|yaa|salam|kenal|[.,;!\n-]|$)",
    ]
    for pat in role_patterns:
        m = re.search(pat, lower)
        if m:
            raw = m.group(1).strip()
            words = [w.capitalize() for w in raw.split() if w.lower() not in {"ya", "yaa", "salam", "kenal"}]
            if words and 1 <= len(words) <= 5:
                res["role"] = " ".join(words)
                break

    notes = []
    if res["callsign"]:
        notes.append(f"Preferensi panggilan: {res['callsign']}")
    if res["role"]:
        notes.append(f"Identifikasi peran: {res['role']}")
    if notes:
        notes.append("Terdeteksi otomatis via Profiler")
        res["notes"] = ". ".join(notes)

    return res

def cmd_get(args):
    target = args.target
    db_path = get_db_path()
    conn = sqlite3.connect(db_path)
    init_db(conn)
    c = conn.cursor()

    c.execute("""
        SELECT sender_jid, name, role, authority_level, notes, updated_at
        FROM user_profiles
        WHERE sender_jid = ? OR sender_jid LIKE ? OR name LIKE ?
        LIMIT 1
    """, (target, f"%{target}%", f"%{target}%"))
    row = c.fetchone()
    conn.close()

    if not row:
        if args.json:
            print(json.dumps({"error": f"Profil untuk '{target}' tidak ditemukan"}))
        else:
            print(f"⚠️ Profil untuk '{target}' tidak ditemukan di database.")
        return

    data = {
        "sender_jid": row[0],
        "name": row[1],
        "role": row[2],
        "authority_level": row[3],
        "notes": row[4],
        "updated_at": row[5]
    }

    if args.json:
        print(json.dumps(data, indent=2))
    else:
        print("👤 Detail Profil Pengguna (Aina Profiling Memory)")
        print(f"• WhatsApp JID  : {data['sender_jid']}")
        print(f"• Nama          : {data['name'] or '-'}")
        print(f"• Peran / Posisi: {data['role'] or '-'}")
        print(f"• Wewenang      : {data['authority_level'].upper()}")
        print(f"• Catatan / Izin: {data['notes'] or '-'}")
        print(f"• Terakhir Update: {data['updated_at'] or '-'}")

def cmd_list(args):
    db_path = get_db_path()
    conn = sqlite3.connect(db_path)
    init_db(conn)
    c = conn.cursor()
    c.execute("SELECT sender_jid, name, role, authority_level, notes, updated_at FROM user_profiles ORDER BY updated_at DESC")
    rows = c.fetchall()
    conn.close()

    profiles = []
    for r in rows:
        profiles.append({
            "sender_jid": r[0],
            "name": r[1],
            "role": r[2],
            "authority_level": r[3],
            "notes": r[4],
            "updated_at": r[5]
        })

    if args.json:
        print(json.dumps(profiles, indent=2))
        return

    print("👤 Profil Pengguna Terdaftar (Aina Profiling Memory)")
    print("=" * 80)
    if not profiles:
        print("(Belum ada profil pengguna tersimpan)")
    else:
        for p in profiles:
            name = p["name"] or "-"
            role = p["role"] or "-"
            auth = p["authority_level"].upper()
            notes = p["notes"] or "-"
            print(f"• {name} ({p['sender_jid']})")
            print(f"  Peran: {role} | Otoritas: {auth}")
            print(f"  Catatan / Izin: {notes}")
            print("-" * 80)

def cmd_set(args):
    jid = args.jid if "@" in args.jid else f"{args.jid}@s.whatsapp.net"
    db_path = get_db_path()
    conn = sqlite3.connect(db_path)
    init_db(conn)
    c = conn.cursor()

    c.execute("SELECT name, role, authority_level, notes FROM user_profiles WHERE sender_jid = ?", (jid,))
    existing = c.fetchone()

    name = args.name or (existing[0] if existing else None)
    role = args.role or (existing[1] if existing else None)
    auth = (args.authority.lower() if args.authority else (existing[2] if existing else "guest"))
    notes = args.notes or (existing[3] if existing else None)

    c.execute("""
        INSERT INTO user_profiles (sender_jid, name, role, authority_level, notes, updated_at)
        VALUES (?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
        ON CONFLICT(sender_jid) DO UPDATE SET
            name = COALESCE(?, user_profiles.name),
            role = COALESCE(?, user_profiles.role),
            authority_level = COALESCE(?, user_profiles.authority_level),
            notes = COALESCE(?, user_profiles.notes),
            updated_at = CURRENT_TIMESTAMP
    """, (jid, name, role, auth, notes, args.name, args.role, args.authority, args.notes))

    conn.commit()
    conn.close()
    print(f"✅ Profil pengguna `{jid}` berhasil diperbarui (Nama: '{name}', Peran: '{role}', Otoritas: {auth.upper()}).")

def cmd_extract(args):
    res = extract_entities_from_text(args.text)
    print(json.dumps(res, indent=2))

def cmd_auto_profile(args):
    jid = args.jid if "@" in args.jid else f"{args.jid}@s.whatsapp.net"
    extracted = extract_entities_from_text(args.text)
    if not extracted["name"] and not extracted["role"]:
        print(f"ℹ️ Tidak ada entitas profil baru yang terdeteksi dari teks untuk `{jid}`.")
        return

    db_path = get_db_path()
    conn = sqlite3.connect(db_path)
    init_db(conn)
    c = conn.cursor()

    c.execute("SELECT name, role, authority_level, notes FROM user_profiles WHERE sender_jid = ?", (jid,))
    row = c.fetchone()

    new_name = extracted["name"] or (row[0] if row else None)
    new_role = extracted["role"] or (row[1] if row else None)
    authority = row[2] if row else "guest"
    
    old_notes = row[3] if row and row[3] else ""
    extra_notes = extracted["notes"] or ""
    if old_notes and extra_notes:
        combined_notes = f"{old_notes}; {extra_notes}"
    else:
        combined_notes = old_notes or extra_notes

    c.execute("""
        INSERT INTO user_profiles (sender_jid, name, role, authority_level, notes, updated_at)
        VALUES (?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
        ON CONFLICT(sender_jid) DO UPDATE SET
            name = COALESCE(?, user_profiles.name),
            role = COALESCE(?, user_profiles.role),
            notes = COALESCE(?, user_profiles.notes),
            updated_at = CURRENT_TIMESTAMP
    """, (jid, new_name, new_role, authority, combined_notes, extracted["name"], extracted["role"], combined_notes))

    conn.commit()
    conn.close()
    print(f"✅ Auto-Profile berhasil memperbarui `{jid}`: Nama='{new_name}', Peran='{new_role}', Otoritas='{authority}'.")

def main():
    parser = argparse.ArgumentParser(description="Aina User Profiling & Progressive Trust Manager")
    subparsers = parser.add_subparsers(dest="subcmd", help="Subcommand")

    # get
    p_get = subparsers.add_parser("get", help="Get user profile by JID or name")
    p_get.add_argument("target", help="Sender JID or name substring")
    p_get.add_argument("--json", action="store_true", help="Output as JSON")

    # list
    p_list = subparsers.add_parser("list", help="List all registered profiles")
    p_list.add_argument("--json", action="store_true", help="Output as JSON")

    # set
    p_set = subparsers.add_parser("set", help="Set or update a user profile")
    p_set.add_argument("jid", help="Sender WhatsApp JID")
    p_set.add_argument("--name", "-n", help="Preferred display name / callsign")
    p_set.add_argument("--role", "-r", help="Role / team title")
    p_set.add_argument("--authority", "-a", choices=["admin", "staff", "guest"], help="Authority level")
    p_set.add_argument("--notes", "-m", help="Notes, preferences, permissions")

    # extract
    p_ext = subparsers.add_parser("extract", help="Extract profile entities from text")
    p_ext.add_argument("text", help="Conversational message text")

    # auto-profile
    p_auto = subparsers.add_parser("auto-profile", help="Extract and update profile from message text")
    p_auto.add_argument("jid", help="Sender WhatsApp JID")
    p_auto.add_argument("text", help="Incoming conversational text")

    args = parser.parse_args()
    if not args.subcmd:
        parser.print_help()
        sys.exit(0)

    if args.subcmd == "get":
        cmd_get(args)
    elif args.subcmd == "list":
        cmd_list(args)
    elif args.subcmd == "set":
        cmd_set(args)
    elif args.subcmd == "extract":
        cmd_extract(args)
    elif args.subcmd == "auto-profile":
        cmd_auto_profile(args)

if __name__ == "__main__":
    main()
