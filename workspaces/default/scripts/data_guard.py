#!/usr/bin/env python3
"""
Aina Agnostic Data Guard & Policy Engine
========================================
Sistem keamanan deterministik level data & tool yang agnostik terhadap jenis dataset:
1. Menjaga dataset dari akses tidak sah tanpa bergantung pada self-policing LLM (Anti-Jailbreak).
2. Mendukung 4 tingkatan sensitivitas: PUBLIC, INTERNAL, RESTRICTED, CONFIDENTIAL.
3. Verifikasi subjek berbasis identitas nyata WhatsApp (sender_jid dan chat_jid).
4. Mekanisme permohonan tiket akses (Human-in-the-Loop) dengan otorisasi Admin.
"""

import sys
import os
import json
import argparse
import time
from pathlib import Path

def find_repo_root() -> Path:
    current = Path.cwd()
    for parent in [current] + list(current.parents):
        if (parent / "shared_data").exists() or (parent / "workspaces").exists() or (parent / ".git").exists():
            return parent
    return current

REPO_ROOT = find_repo_root()
POLICIES_DIR = REPO_ROOT / "shared_data" / "policies"
TICKETS_FILE = POLICIES_DIR / "access_tickets.json"

CLASSIFICATIONS = ["PUBLIC", "INTERNAL", "RESTRICTED", "CONFIDENTIAL"]

def ensure_dirs():
    POLICIES_DIR.mkdir(parents=True, exist_ok=True)
    if not TICKETS_FILE.exists():
        with open(TICKETS_FILE, "w", encoding="utf-8") as f:
            json.dump({}, f, indent=2)

def load_policy(dataset_slug: str) -> dict | None:
    ensure_dirs()
    policy_path = POLICIES_DIR / f"{dataset_slug}.json"
    if not policy_path.exists():
        return None
    try:
        with open(policy_path, "r", encoding="utf-8") as f:
            return json.load(f)
    except Exception:
        return None

def save_policy(dataset_slug: str, data: dict):
    ensure_dirs()
    policy_path = POLICIES_DIR / f"{dataset_slug}.json"
    with open(policy_path, "w", encoding="utf-8") as f:
        json.dump(data, f, indent=2)

def load_tickets() -> dict:
    ensure_dirs()
    try:
        with open(TICKETS_FILE, "r", encoding="utf-8") as f:
            return json.load(f)
    except Exception:
        return {}

def save_tickets(tickets: dict):
    ensure_dirs()
    with open(TICKETS_FILE, "w", encoding="utf-8") as f:
        json.dump(tickets, f, indent=2)

def clean_jid(jid: str | None) -> str:
    if not jid:
        return ""
    j = jid.strip().lower()
    return j

# ─── SUBCOMMANDS ─────────────────────────────────────────────────────────────

def cmd_register(args):
    dataset_slug = args.dataset.strip().lower()
    classification = args.classification.strip().upper()
    if classification not in CLASSIFICATIONS:
        print(f"❌ Klasifikasi '{classification}' tidak valid. Pilihan: {', '.join(CLASSIFICATIONS)}")
        sys.exit(1)

    existing = load_policy(dataset_slug) or {}

    allowed_groups = set(existing.get("allowed_groups", []))
    if args.allow_group:
        for g in args.allow_group:
            allowed_groups.add(clean_jid(g))

    allowed_senders = set(existing.get("allowed_senders", []))
    if args.allow_sender:
        for s in args.allow_sender:
            allowed_senders.add(clean_jid(s))

    policy = {
        "dataset_slug": dataset_slug,
        "title": args.title or existing.get("title", dataset_slug),
        "classification": classification,
        "description": args.description or existing.get("description", ""),
        "allowed_groups": sorted(list(allowed_groups)),
        "allowed_senders": sorted(list(allowed_senders)),
        "approver_jid": clean_jid(args.approver) or existing.get("approver_jid", ""),
        "updated_at": int(time.time()),
    }

    save_policy(dataset_slug, policy)
    print(f"✅ Kebijakan akses dataset '{dataset_slug}' berhasil didaftarkan/diperbarui:")
    print(json.dumps(policy, indent=2))

def cmd_check(args):
    dataset_slug = args.dataset.strip().lower()
    sender_jid = clean_jid(args.sender)
    chat_jid = clean_jid(args.chat)

    policy = load_policy(dataset_slug)
    if not policy:
        # Jika belum ada kebijakan terdaftar, default aman: izinkan hanya jika PUBLIC
        res = {
            "allowed": False,
            "dataset": dataset_slug,
            "classification": "UNREGISTERED",
            "reason": f"Dataset '{dataset_slug}' belum memiliki kebijakan akses di shared_data/policies/. Akses diblokir secara default (Zero Trust).",
            "approver": "Admin"
        }
        print(json.dumps(res, indent=2))
        sys.exit(1)

    classification = policy.get("classification", "RESTRICTED").upper()
    allowed_groups = [clean_jid(g) for g in policy.get("allowed_groups", [])]
    allowed_senders = [clean_jid(s) for s in policy.get("allowed_senders", [])]
    approver = policy.get("approver_jid", "Admin")

    # 1. Klasifikasi PUBLIC -> Selalu izinkan
    if classification == "PUBLIC":
        res = {
            "allowed": True,
            "dataset": dataset_slug,
            "classification": "PUBLIC",
            "reason": "Dataset berstatus publik untuk seluruh pengguna."
        }
        print(json.dumps(res, indent=2))
        sys.exit(0)

    # 2. Cek apakah ada tiket akses sementara yang aktif (Approved Ticket)
    tickets = load_tickets()
    now = int(time.time())
    for t_id, t_info in tickets.items():
        if t_info.get("dataset") == dataset_slug and clean_jid(t_info.get("sender")) == sender_jid:
            if t_info.get("status") == "APPROVED" and t_info.get("expires_at", 0) > now:
                res = {
                    "allowed": True,
                    "dataset": dataset_slug,
                    "classification": classification,
                    "reason": f"Akses diberikan via tiket resmi disetujui: {t_id} (berlaku s.d. {time.ctime(t_info['expires_at'])})."
                }
                print(json.dumps(res, indent=2))
                sys.exit(0)

    # 3. Cek apakah pengirim adalah Approver / Admin Utama
    if approver and sender_jid == approver:
        res = {
            "allowed": True,
            "dataset": dataset_slug,
            "classification": classification,
            "reason": "Akses diberikan karena pemohon adalah Administrator/Approver resmi dataset ini."
        }
        print(json.dumps(res, indent=2))
        sys.exit(0)

    # 4. Evaluasi RESTRICTED
    if classification == "RESTRICTED":
        # Cocokkan grup chat
        if chat_jid and chat_jid in allowed_groups:
            res = {
                "allowed": True,
                "dataset": dataset_slug,
                "classification": "RESTRICTED",
                "reason": f"Akses diizinkan di dalam grup terdaftar: {chat_jid}."
            }
            print(json.dumps(res, indent=2))
            sys.exit(0)

        # Cocokkan nomor pengirim individu
        if sender_jid and sender_jid in allowed_senders:
            res = {
                "allowed": True,
                "dataset": dataset_slug,
                "classification": "RESTRICTED",
                "reason": f"Akses diizinkan untuk nomor terdaftar: {sender_jid}."
            }
            print(json.dumps(res, indent=2))
            sys.exit(0)

        # Jika tidak cocok
        res = {
            "allowed": False,
            "dataset": dataset_slug,
            "classification": "RESTRICTED",
            "reason": f"Nomor {sender_jid} atau ruang obrolan {chat_jid} bukan anggota yang berwenang untuk dataset {dataset_slug}.",
            "approver": approver,
            "action_required": f"Minta izin atau konfirmasi langsung ke {approver}."
        }
        print(json.dumps(res, indent=2))
        sys.exit(1)

    # 5. Evaluasi CONFIDENTIAL
    if classification == "CONFIDENTIAL":
        res = {
            "allowed": False,
            "dataset": dataset_slug,
            "classification": "CONFIDENTIAL",
            "reason": f"Dataset {dataset_slug} berstatus Sangat Rahasia (Confidential). Hanya dapat diakses langsung oleh {approver}.",
            "approver": approver
        }
        print(json.dumps(res, indent=2))
        sys.exit(1)

    # Fallback DENY
    res = {
        "allowed": False,
        "dataset": dataset_slug,
        "classification": classification,
        "reason": "Akses ditolak oleh kebijakan keamanan data.",
        "approver": approver
    }
    print(json.dumps(res, indent=2))
    sys.exit(1)

def cmd_request_access(args):
    dataset_slug = args.dataset.strip().lower()
    sender_jid = clean_jid(args.sender)
    reason = args.reason.strip() if args.reason else "Kebutuhan analisis dinas"

    policy = load_policy(dataset_slug)
    approver = policy.get("approver_jid", "Admin") if policy else "Admin"

    import random
    ticket_id = f"REQ-{int(time.time())%100000:05d}-{random.randint(10,99)}"

    tickets = load_tickets()
    tickets[ticket_id] = {
        "ticket_id": ticket_id,
        "dataset": dataset_slug,
        "sender": sender_jid,
        "reason": reason,
        "created_at": int(time.time()),
        "status": "PENDING",
        "approver": approver,
        "expires_at": 0
    }
    save_tickets(tickets)

    out = {
        "ticket_id": ticket_id,
        "dataset": dataset_slug,
        "sender": sender_jid,
        "approver": approver,
        "status": "PENDING",
        "notification_for_admin": f"🔔 Permohonan Akses Data: Nomor {sender_jid} meminta akses dataset '{dataset_slug}'. Alasan: {reason}. Untuk menyetujui, balas: /approve {ticket_id}"
    }
    print(json.dumps(out, indent=2))

def cmd_approve(args):
    ticket_id = args.ticket.strip().upper()
    duration_hours = args.duration or 24

    tickets = load_tickets()
    if ticket_id not in tickets:
        print(f"❌ Tiket permohonan '{ticket_id}' tidak ditemukan.")
        sys.exit(1)

    t = tickets[ticket_id]
    now = int(time.time())
    expires = now + (duration_hours * 3600)
    t["status"] = "APPROVED"
    t["approved_at"] = now
    t["expires_at"] = expires

    save_tickets(tickets)
    print(f"✅ Tiket '{ticket_id}' berhasil DISETUJUI.")
    print(f"   Dataset : {t.get('dataset')}")
    print(f"   Pemohon : {t.get('sender')}")
    print(f"   Durasi  : {duration_hours} jam (berlaku s.d. {time.ctime(expires)})")

def cmd_list(args):
    ensure_dirs()
    policies = list(POLICIES_DIR.glob("*.json"))
    valid_policies = [p for p in policies if p.name != "access_tickets.json"]

    print(f"\n🛡️ DAFTAR KEBIJAKAN AKSES DATASET ({len(valid_policies)}):")
    divider = "-" * 85
    print(divider)
    print(f"| {'Dataset Slug':<18} | {'Klasifikasi':<14} | {'Grup Terdaftar':<16} | {'Approver':<26} |")
    print(divider)

    for p in sorted(valid_policies):
        try:
            data = json.loads(p.read_text(encoding="utf-8"))
            slug = data.get("dataset_slug", p.stem)
            cls_ = data.get("classification", "-")
            groups = f"{len(data.get('allowed_groups', []))} grup"
            approver = data.get("approver_jid", "-")
            print(f"| {slug:<18} | {cls_:<14} | {groups:<16} | {approver[:26]:<26} |")
        except Exception:
            continue
    print(divider)

    tickets = load_tickets()
    active_tickets = [t for t in tickets.values() if t.get("status") == "PENDING" or t.get("expires_at", 0) > int(time.time())]
    if active_tickets:
        print(f"\n🎫 TIKET AKSES AKTIF ({len(active_tickets)}):")
        for t in active_tickets:
            print(f"   • [{t.get('ticket_id')}] {t.get('dataset')} untuk {t.get('sender')} | Status: {t.get('status')}")
    print("")

def main():
    parser = argparse.ArgumentParser(description="Aina Agnostic Data Guard Policy Engine")
    subparsers = parser.add_subparsers(dest="command", required=True)

    # Register
    p_reg = subparsers.add_parser("register", help="Daftarkan/perbarui kebijakan akses dataset")
    p_reg.add_argument("--dataset", "-d", required=True, help="Slug pengenal dataset (misal: se2026, regsosek)")
    p_reg.add_argument("--classification", "-c", required=True, choices=CLASSIFICATIONS, help="Tingkat kerahasiaan data")
    p_reg.add_argument("--title", help="Judul dataset")
    p_reg.add_argument("--description", help="Deskripsi dataset")
    p_reg.add_argument("--allow-group", "-g", action="append", help="JID grup yang diizinkan (bisa multiple)")
    p_reg.add_argument("--allow-sender", "-s", action="append", help="JID nomor telepon yang diizinkan (bisa multiple)")
    p_reg.add_argument("--approver", "-a", help="JID penanggung jawab / approver izin (misal: JID Admin)")

    # Check
    p_chk = subparsers.add_parser("check", help="Verifikasi hak akses pemohon")
    p_chk.add_argument("--dataset", "-d", required=True, help="Slug dataset")
    p_chk.add_argument("--sender", "-s", required=True, help="Nomor JID pemohon")
    p_chk.add_argument("--chat", "-c", help="JID ruang obrolan (grup atau DM)")

    # Request Access
    p_req = subparsers.add_parser("request-access", help="Buat tiket permohonan akses baru")
    p_req.add_argument("--dataset", "-d", required=True, help="Slug dataset")
    p_req.add_argument("--sender", "-s", required=True, help="Nomor JID pemohon")
    p_req.add_argument("--reason", "-r", help="Alasan permohonan akses")

    # Approve
    p_app = subparsers.add_parser("approve", help="Setujui tiket permohonan akses")
    p_app.add_argument("--ticket", "-t", required=True, help="ID tiket (misal: REQ-12345-67)")
    p_app.add_argument("--duration", type=int, default=24, help="Durasi masa berlaku izin dalam jam (default: 24 jam)")

    # List
    subparsers.add_parser("list", help="Tampilkan seluruh kebijakan data & tiket")

    args = parser.parse_args()

    if args.command == "register":
        cmd_register(args)
    elif args.command == "check":
        cmd_check(args)
    elif args.command == "request-access":
        cmd_request_access(args)
    elif args.command == "approve":
        cmd_approve(args)
    elif args.command == "list":
        cmd_list(args)

if __name__ == "__main__":
    main()
