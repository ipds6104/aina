#!/usr/bin/env python3
"""
Aina High-Performance WhatsApp Chat Archive & Retrieval Engine
=============================================================
Memproses berkas ekspor chat WhatsApp (.zip / .txt) dari ponsel (bahkan arsip 3+ tahun):
1. Streaming Regex Parser (mendukung format Android & iOS berbagai locale/bahasa)
2. Database SQLite Lokal dengan Full-Text Search (FTS5) untuk pencarian sub-milidetik (<10ms)
3. Ekstraksi otomatis katalog tautan (Google Sheets, Drive, Docs, dsb.)
4. Agregasi statistik kontribusi pengirim dan timeline bulanan
5. Zero Token LLM Waste: Pencarian deterministik instan tanpa membaca puluhan ribu chat ke prompt
"""

import sys
import os
import re
import json
import zipfile
import sqlite3
import argparse
import datetime
from pathlib import Path

# ─── REPO & WORKSPACE RESOLVER ───────────────────────────────────────────────

def find_repo_root() -> Path:
    current = Path.cwd()
    for parent in [current] + list(current.parents):
        if (parent / "workspaces").exists() or (parent / ".git").exists():
            return parent
    return current

REPO_ROOT = find_repo_root()

def get_workspaces_root() -> Path:
    """
    Menemukan direktori induk kumpulan workspace dengan hierarki:
    1. AINA_WORKSPACES_DIR (variabel environment eksplisit)
    2. Parent directory dari AGENT_WORKSPACE jika diset
    3. REPO_ROOT / 'workspaces'
    4. CWD / 'workspaces'
    """
    if os.environ.get("AINA_WORKSPACES_DIR"):
        p = Path(os.environ["AINA_WORKSPACES_DIR"]).resolve()
        if p.exists():
            return p

    if os.environ.get("AGENT_WORKSPACE"):
        p = Path(os.environ["AGENT_WORKSPACE"]).resolve()
        if p.is_dir() and (p / "knowledge").exists():
            return p.parent
        elif p.exists() and p.is_dir():
            return p

    repo_ws = REPO_ROOT / "workspaces"
    if repo_ws.exists():
        return repo_ws

    return Path.cwd() / "workspaces"

def resolve_workspace_dir(ws_input: str | None = None) -> Path:
    if ws_input and ws_input.strip():
        val = ws_input.strip()
        raw_path = Path(val)
        if raw_path.is_dir():
            return raw_path.resolve()

        cwd_cand = (Path.cwd() / val).resolve()
        if cwd_cand.is_dir():
            return cwd_cand

        root = get_workspaces_root()
        slug_cand = root / val
        if slug_cand.is_dir():
            return slug_cand.resolve()

        lower_cand = root / val.lower()
        if lower_cand.is_dir():
            return lower_cand.resolve()

        return slug_cand

    if os.environ.get("AGENT_WORKSPACE"):
        p = Path(os.environ["AGENT_WORKSPACE"]).resolve()
        if p.is_dir():
            return p

    cwd = Path.cwd()
    if (cwd / "knowledge").is_dir():
        return cwd.resolve()

    return (get_workspaces_root() / "default").resolve()

WORKSPACES_ROOT = get_workspaces_root()

# ─── REGEX PATTERNS FOR WHATSAPP EXPORT ───────────────────────────────────────

# Matches:
# 1) [15/08/23, 09:21:45] Sender: Msg (iOS)
# 2) 15/08/23, 09:21 - Sender: Msg (Android comma)
# 3) 15/08/23 09.21 - Sender: Msg (Android dot)
# 4) 15/08/2023, 09:21 - Sender: Msg
# 5) 8/15/23, 9:21 AM - Sender: Msg
LINE_REGEXES = [
    # Bracketed: [DD/MM/YY(YY), HH:MM(:SS)( AM/PM)?]
    re.compile(r'^\[(?P<date>\d{1,4}[/\-\.]\d{1,2}[/\-\.]\d{2,4})[,\s]+(?P<time>\d{1,2}[:\.]\d{2}(?:[:\.]\d{2})?(?:\s*[AaPp][Mm])?)\]\s+(?P<sender>[^:]+?):\s+(?P<text>.*)$'),
    # Standard: DD/MM/YY(YY), HH:MM( AM/PM)? - Sender: Msg
    re.compile(r'^(?P<date>\d{1,4}[/\-\.]\d{1,2}[/\-\.]\d{2,4})[,\s]+(?P<time>\d{1,2}[:\.]\d{2}(?:[:\.]\d{2})?(?:\s*[AaPp][Mm])?)\s+-\s+(?P<sender>[^:]+?):\s+(?P<text>.*)$'),
]

URL_REGEX = re.compile(r'https?://[^\s<>"\')]+')
ATTACHMENT_PATTERNS = [
    re.compile(r'<attached:\s*([^>]+)>', re.IGNORECASE),
    re.compile(r'([\w\-\.]+\.(?:jpg|jpeg|png|webp|opus|mp3|mp4|pdf|xlsx|csv|docx|zip))\s+\(file terlampir\)', re.IGNORECASE),
    re.compile(r'([\w\-\.]+\.(?:jpg|jpeg|png|webp|opus|mp3|mp4|pdf|xlsx|csv|docx|zip))\s+<attached>', re.IGNORECASE),
]

def slugify(text: str) -> str:
    text = text.strip().lower()
    text = re.sub(r'[\s_]+', '-', text)
    text = re.sub(r'[^a-z0-9-]', '', text)
    return text.strip('-')

def parse_date_time(date_str: str, time_str: str) -> str:
    """Normalisasi berbagai format tanggal teks ke ISO format: YYYY-MM-DD HH:MM:SS."""
    # Clean time
    time_str = time_str.replace('.', ':').strip()
    is_pm = 'pm' in time_str.lower()
    is_am = 'am' in time_str.lower()
    time_clean = re.sub(r'\s*[AaPp][Mm]', '', time_str).strip()

    time_parts = [int(p) for p in time_clean.split(':') if p.isdigit()]
    hh = time_parts[0] if len(time_parts) > 0 else 0
    mm = time_parts[1] if len(time_parts) > 1 else 0
    ss = time_parts[2] if len(time_parts) > 2 else 0

    if is_pm and hh < 12:
        hh += 12
    elif is_am and hh == 12:
        hh = 0

    # Clean date
    delims = ['/', '-', '.']
    parts = []
    for d in delims:
        if d in date_str:
            parts = [int(p) for p in date_str.split(d) if p.isdigit()]
            break

    if len(parts) != 3:
        return f"1970-01-01 {hh:02d}:{mm:02d}:{ss:02d}"

    # Disambiguate DD/MM/YY vs YYYY/MM/DD
    if parts[0] > 1000: # YYYY/MM/DD
        year, month, day = parts[0], parts[1], parts[2]
    elif parts[2] > 1000: # DD/MM/YYYY
        day, month, year = parts[0], parts[1], parts[2]
    else: # DD/MM/YY
        day, month = parts[0], parts[1]
        year = 2000 + parts[2] if parts[2] < 100 else parts[2]

    # Swap if month > 12 (means MM/DD/YY was used)
    if month > 12 and day <= 12:
        day, month = month, day

    return f"{year:04d}-{month:02d}-{day:02d} {hh:02d}:{mm:02d}:{ss:02d}"

# ─── DATABASE INITIALIZER (WITH FTS5 FULL TEXT SEARCH) ───────────────────────

def init_chat_db(db_path: Path):
    conn = sqlite3.connect(db_path)
    conn.execute("PRAGMA journal_mode=WAL;")
    conn.execute("PRAGMA synchronous=NORMAL;")
    
    conn.execute("""
    CREATE TABLE IF NOT EXISTS messages (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        timestamp TEXT NOT NULL,
        year_month TEXT NOT NULL,
        sender TEXT NOT NULL,
        text TEXT NOT NULL,
        has_media INTEGER DEFAULT 0,
        media_name TEXT DEFAULT ''
    );
    """)
    conn.execute("CREATE INDEX IF NOT EXISTS idx_msg_ts ON messages(timestamp);")
    conn.execute("CREATE INDEX IF NOT EXISTS idx_msg_ym ON messages(year_month);")
    conn.execute("CREATE INDEX IF NOT EXISTS idx_msg_sender ON messages(sender);")

    # Virtual FTS5 table for lightning-fast search
    try:
        conn.execute("""
        CREATE VIRTUAL TABLE IF NOT EXISTS messages_fts USING fts5(
            sender,
            text,
            content='messages',
            content_rowid='id'
        );
        """)
    except sqlite3.OperationalError:
        # Fallback if fts5 is disabled in build
        pass

    conn.commit()
    return conn

# ─── IMPORT ENGINE ───────────────────────────────────────────────────────────

def import_chat_archive(
    archive_path: Path,
    workspace_name: str,
    chat_slug: str = None,
    extract_media: bool = True
) -> dict:
    ws_dir = resolve_workspace_dir(workspace_name)
    if not ws_dir.exists():
        raise FileNotFoundError(f"Workspace '{workspace_name}' tidak ditemukan di {ws_dir}")

    if not chat_slug:
        chat_slug = slugify(archive_path.stem.replace("WhatsApp Chat with", "").replace("WhatsApp Chat -", ""))

    target_dir = ws_dir / "data" / "chats" / chat_slug
    docs_dir = target_dir / "documents"
    target_dir.mkdir(parents=True, exist_ok=True)
    docs_dir.mkdir(parents=True, exist_ok=True)

    db_path = target_dir / "messages.db"
    conn = init_chat_db(db_path)

    # Clean existing records
    conn.execute("DELETE FROM messages;")
    try:
        conn.execute("DELETE FROM messages_fts;")
    except Exception:
        pass

    chat_text_content = ""
    is_zip = archive_path.suffix.lower() == ".zip"

    # 1. Unpack ZIP or read direct TXT
    if is_zip:
        with zipfile.ZipFile(archive_path, 'r') as zf:
            namelist = zf.namelist()
            txt_candidates = [n for n in namelist if n.endswith('.txt') and not n.startswith('__MACOSX')]
            if not txt_candidates:
                raise ValueError("Tidak ditemukan file riwayat chat (.txt) di dalam berkas zip.")
            
            # Read primary chat file
            primary_txt = txt_candidates[0]
            chat_text_content = zf.read(primary_txt).decode('utf-8', errors='replace')

            # Extract media if requested
            if extract_media:
                for member in namelist:
                    if member == primary_txt or member.startswith('__MACOSX') or member.endswith('/'):
                        continue
                    fname = Path(member).name
                    # Save attachments to documents/
                    out_f = docs_dir / fname
                    with zf.open(member) as src, open(out_f, "wb") as dst:
                        dst.write(src.read())
    else:
        chat_text_content = archive_path.read_text(encoding='utf-8', errors='replace')

    # 2. Parse Chat Lines
    lines = chat_text_content.splitlines()
    parsed_messages = []
    current_msg = None
    all_links = []
    sender_counts = {}

    for line in lines:
        line_str = line.strip()
        if not line_str:
            continue

        matched = False
        for rx in LINE_REGEXES:
            m = rx.match(line_str)
            if m:
                matched = True
                if current_msg:
                    parsed_messages.append(current_msg)

                d_str = m.group("date")
                t_str = m.group("time")
                sender = m.group("sender").strip()
                text = m.group("text").strip()
                iso_ts = parse_date_time(d_str, t_str)
                ym = iso_ts[:7]

                # Check media attachment
                has_media = 0
                media_name = ""
                for att_rx in ATTACHMENT_PATTERNS:
                    att_m = att_rx.search(text)
                    if att_m:
                        has_media = 1
                        media_name = att_m.group(1).strip()
                        break

                current_msg = {
                    "timestamp": iso_ts,
                    "year_month": ym,
                    "sender": sender,
                    "text": text,
                    "has_media": has_media,
                    "media_name": media_name,
                }
                sender_counts[sender] = sender_counts.get(sender, 0) + 1

                # Extract links
                for url in URL_REGEX.findall(text):
                    all_links.append({
                        "url": url,
                        "timestamp": iso_ts,
                        "sender": sender
                    })
                break

        if not matched and current_msg:
            # Multi-line message continuation
            current_msg["text"] += "\n" + line_str
            for url in URL_REGEX.findall(line_str):
                all_links.append({
                    "url": url,
                    "timestamp": current_msg["timestamp"],
                    "sender": current_msg["sender"]
                })

    if current_msg:
        parsed_messages.append(current_msg)

    # 3. Batch Insert into SQLite
    cursor = conn.cursor()
    insert_sql = """
    INSERT INTO messages (timestamp, year_month, sender, text, has_media, media_name)
    VALUES (?, ?, ?, ?, ?, ?);
    """
    for msg in parsed_messages:
        cursor.execute(insert_sql, (
            msg["timestamp"],
            msg["year_month"],
            msg["sender"],
            msg["text"],
            msg["has_media"],
            msg["media_name"]
        ))
        rowid = cursor.lastrowid
        try:
            cursor.execute("INSERT INTO messages_fts(rowid, sender, text) VALUES (?, ?, ?);", (rowid, msg["sender"], msg["text"]))
        except Exception:
            pass

    conn.commit()

    # 4. Generate Metadata & Links Index
    earliest = parsed_messages[0]["timestamp"] if parsed_messages else "-"
    latest = parsed_messages[-1]["timestamp"] if parsed_messages else "-"
    total_msgs = len(parsed_messages)
    total_docs = len(list(docs_dir.iterdir())) if docs_dir.exists() else 0

    metadata = {
        "slug": chat_slug,
        "workspace": workspace_name,
        "imported_at": datetime.datetime.now().strftime("%Y-%m-%d %H:%M:%S WIB"),
        "total_messages": total_msgs,
        "earliest_message": earliest,
        "latest_message": latest,
        "total_attachments": total_docs,
        "top_senders": sorted(sender_counts.items(), key=lambda x: x[1], reverse=True)[:10],
        "total_links": len(all_links)
    }

    (target_dir / "metadata.json").write_text(json.dumps(metadata, indent=2, ensure_ascii=False), encoding="utf-8")
    (target_dir / "links.json").write_text(json.dumps(all_links, indent=2, ensure_ascii=False), encoding="utf-8")

    return metadata

# ─── SEARCH & QUERY ENGINE ───────────────────────────────────────────────────

def search_chat(
    workspace_name: str,
    chat_slug: str,
    query: str = None,
    sender: str = None,
    since: str = None,
    until: str = None,
    limit: int = 30
) -> list[dict]:
    ws_dir = resolve_workspace_dir(workspace_name)
    db_path = ws_dir / "data" / "chats" / chat_slug / "messages.db"
    if not db_path.exists():
        print(f"❌ Database chat untuk slug '{chat_slug}' tidak ditemukan di {db_path}")
        return []

    conn = sqlite3.connect(db_path)
    conn.row_factory = sqlite3.Row

    conditions = []
    params = []

    if query:
        # Check if FTS is available
        use_fts = True
        try:
            test_cur = conn.execute("SELECT 1 FROM messages_fts LIMIT 1;")
            test_cur.fetchone()
        except Exception:
            use_fts = False

        if use_fts:
            clean_q = re.sub(r'[^a-zA-Z0-9\s]', '', query).strip()
            if clean_q:
                conditions.append("m.id IN (SELECT rowid FROM messages_fts WHERE messages_fts MATCH ?)")
                params.append(clean_q)
            else:
                conditions.append("m.text LIKE ?")
                params.append(f"%{query}%")
        else:
            conditions.append("m.text LIKE ?")
            params.append(f"%{query}%")

    if sender:
        conditions.append("m.sender LIKE ?")
        params.append(f"%{sender}%")

    if since:
        conditions.append("m.timestamp >= ?")
        params.append(since)

    if until:
        conditions.append("m.timestamp <= ?")
        params.append(until)

    where_clause = " WHERE " + " AND ".join(conditions) if conditions else ""
    sql = f"SELECT timestamp, sender, text, has_media, media_name FROM messages m{where_clause} ORDER BY timestamp ASC LIMIT ?;"
    params.append(limit)

    cursor = conn.execute(sql, params)
    rows = [dict(r) for r in cursor.fetchall()]
    return rows

# ─── CLI COMMANDS & DISPATCHER ───────────────────────────────────────────────

def cmd_import(args):
    archive_file = Path(args.archive)
    if not archive_file.exists():
        print(f"❌ File arsip '{archive_file}' tidak ditemukan.")
        sys.exit(1)

    print(f"📦 Mengimpor arsip WhatsApp: {archive_file.name} ke workspace '{args.workspace}'...")
    start_t = datetime.datetime.now()
    res = import_chat_archive(
        archive_path=archive_file,
        workspace_name=args.workspace,
        chat_slug=args.name,
        extract_media=not args.no_media
    )
    elapsed = (datetime.datetime.now() - start_t).total_seconds()

    print(f"\n🎉 SUKSES MENGIMPOR DALAM {elapsed:.2f} DETIK!")
    print(f"• Slug Chat       : {res['slug']}")
    print(f"• Total Pesan     : {res['total_messages']:,} pesan")
    print(f"• Rentang Tanggal : {res['earliest_message']} s.d. {res['latest_message']}")
    print(f"• Lampiran Media  : {res['total_attachments']} berkas tersimpan")
    print(f"• Tautan Diekstrak: {res['total_links']} link tersimpan")
    print("\n👥 5 Kontributor Teraktif:")
    for s, c in res['top_senders'][:5]:
        pct = (c / res['total_messages']) * 100 if res['total_messages'] else 0
        print(f"  - {s:<25}: {c:,} pesan ({pct:.1f}%)")

def cmd_search(args):
    results = search_chat(
        workspace_name=args.workspace,
        chat_slug=args.name,
        query=args.query,
        sender=args.sender,
        since=args.since,
        until=args.until,
        limit=args.limit
    )

    print(f"\n🔍 === HASIL PENCARIAN CHAT: '{args.name}' ===")
    print(f"Query: '{args.query or '*'}' | Ditemukan: {len(results)} pesan (Limit: {args.limit})\n")
    divider = "-" * 90
    print(divider)
    for r in results:
        print(f"[{r['timestamp']}] *{r['sender']}*:")
        for l in r['text'].splitlines():
            print(f"  {l}")
        if r['has_media']:
            print(f"  📎 [Lampiran: {r['media_name']}]")
        print(divider)

def cmd_links(args):
    ws_dir = resolve_workspace_dir(args.workspace)
    links_file = ws_dir / "data" / "chats" / args.name / "links.json"
    if not links_file.exists():
        print(f"❌ Berkas links.json untuk '{args.name}' tidak ditemukan di {links_file}.")
        return

    links = json.loads(links_file.read_text(encoding="utf-8"))
    if args.domain:
        d = args.domain.lower()
        links = [l for l in links if d in l["url"].lower()]

    print(f"\n🌐 === DAFTAR TAUTAN DIBAGIKAN: '{args.name}' ===")
    print(f"Total Tautan: {len(links)} (Filter: '{args.domain or '*' }')\n")
    for l in links[:args.limit]:
        print(f"• [{l['timestamp']}] {l['sender']}:")
        print(f"  🔗 {l['url']}")

def cmd_stats(args):
    ws_dir = resolve_workspace_dir(args.workspace)
    meta_file = ws_dir / "data" / "chats" / args.name / "metadata.json"
    if not meta_file.exists():
        print(f"❌ Berkas metadata untuk '{args.name}' tidak ditemukan di {meta_file}.")
        return

    meta = json.loads(meta_file.read_text(encoding="utf-8"))
    print(f"\n📊 === STATISTIK LENGKAP ARSIP CHAT: '{args.name}' ===")
    print(json.dumps(meta, indent=2, ensure_ascii=False))

def main():
    parser = argparse.ArgumentParser(description="Aina High-Performance WhatsApp Chat Importer")
    subparsers = parser.add_subparsers(dest="subcommand", help="Perintah chat importer")

    # import
    p_imp = subparsers.add_parser("import", help="Impor berkas ekspor chat (.zip atau .txt)")
    p_imp.add_argument("archive", help="Path berkas zip atau txt chat")
    p_imp.add_argument("--workspace", default=None, help="Nama atau path workspace target (default: aktif)")
    p_imp.add_argument("--name", help="Slug penamaan chat (contoh: grup-kantor)")
    p_imp.add_argument("--no-media", action="store_true", help="Jangan ekstrak berkas media/dokumen")

    # search
    p_srch = subparsers.add_parser("search", help="Cari pesan di arsip chat")
    p_srch.add_argument("name", help="Slug chat yang dicari")
    p_srch.add_argument("--workspace", default=None, help="Nama atau path workspace target (default: aktif)")
    p_srch.add_argument("--query", "-q", help="Kata kunci pencarian teks")
    p_srch.add_argument("--sender", help="Filter berdasarkan nama pengirim")
    p_srch.add_argument("--since", help="Filter mulai tanggal (YYYY-MM-DD)")
    p_srch.add_argument("--until", help="Filter sampai tanggal (YYYY-MM-DD)")
    p_srch.add_argument("--limit", type=int, default=20, help="Jumlah pesan maksimal")

    # links
    p_lnk = subparsers.add_parser("links", help="Tampilkan seluruh tautan yang pernah dibagikan")
    p_lnk.add_argument("name", help="Slug chat")
    p_lnk.add_argument("--workspace", default=None, help="Nama atau path workspace target (default: aktif)")
    p_lnk.add_argument("--domain", help="Filter domain (contoh: drive, docs, sheet, onedrive)")
    p_lnk.add_argument("--limit", type=int, default=50, help="Maksimal tautan ditampilkan")

    # stats
    p_stat = subparsers.add_parser("stats", help="Tampilkan metadata & statistik arsip chat")
    p_stat.add_argument("name", help="Slug chat")
    p_stat.add_argument("--workspace", default=None, help="Nama atau path workspace target (default: aktif)")

    args = parser.parse_args()

    if args.subcommand == "import":
        cmd_import(args)
    elif args.subcommand == "search":
        cmd_search(args)
    elif args.subcommand == "links":
        cmd_links(args)
    elif args.subcommand == "stats":
        cmd_stats(args)
    else:
        parser.print_help()
        sys.exit(1)

if __name__ == "__main__":
    main()
