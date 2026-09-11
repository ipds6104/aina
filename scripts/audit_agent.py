#!/usr/bin/env python3
"""
Antigravity CLI Audit Trail & Inspection Tool
=============================================
Memungkinkan sysadmin & pengembang mengaudit seluruh tindakan dan eksekusi alat (tool calls)
yang dilakukan oleh Antigravity CLI / Aina secara deterministik:
1. Pencarian kata kunci teks (query filter pada perintah shell, path berkas, dsb.)
2. Filter berdasarkan jenis alat (run_command, replace_file_content, write_to_file, dll.)
3. Filter rentang waktu (--since 1h, 24h, 7d atau --date YYYY-MM-DD)
4. Deteksi kegagalan / kesalahan (--errors-only)
5. Urutan kronologis (--sort asc / desc)
"""

import sys
import os
import re
import json
import argparse
import datetime
from pathlib import Path

# ─── LOG LOCATIONS ───────────────────────────────────────────────────────────

AGY_APP_DIR = Path("/root/.gemini/antigravity-cli")
BRAIN_DIR = AGY_APP_DIR / "brain"
HISTORY_FILE = AGY_APP_DIR / "history.jsonl"

def parse_iso_timestamp(ts_str: str) -> datetime.datetime:
    """Parse ISO 8601 string e.g. 2026-09-11T14:43:09Z ke timezone-aware UTC datetime."""
    # Strip Z or +00:00
    if ts_str.endswith("Z"):
        ts_str = ts_str[:-1] + "+00:00"
    return datetime.datetime.fromisoformat(ts_str)

def format_local_time(dt: datetime.datetime) -> str:
    # Convert UTC to local WIB (+7)
    wib_tz = datetime.timezone(datetime.timedelta(hours=7))
    local_dt = dt.astimezone(wib_tz)
    return local_dt.strftime("%Y-%m-%d %H:%M:%S WIB")

def parse_since_duration(since_str: str) -> datetime.timedelta:
    since_str = since_str.strip().lower()
    m = re.match(r'^(\d+)([smhd])$', since_str)
    if not m:
        raise ValueError(f"Format durasi tidak valid: '{since_str}'. Gunakan format: 30m, 2h, 1d")
    val, unit = int(m.group(1)), m.group(2)
    if unit == 's':
        return datetime.timedelta(seconds=val)
    elif unit == 'm':
        return datetime.timedelta(minutes=val)
    elif unit == 'h':
        return datetime.timedelta(hours=val)
    elif unit == 'd':
        return datetime.timedelta(days=val)
    return datetime.timedelta(hours=1)

# ─── LOG SCANNER ─────────────────────────────────────────────────────────────

def scan_all_transcripts(conversation_id: str = None) -> list[dict]:
    records = []
    
    if conversation_id:
        target_transcripts = [BRAIN_DIR / conversation_id / ".system_generated" / "logs" / "transcript.jsonl"]
    else:
        target_transcripts = list(BRAIN_DIR.glob("*/.system_generated/logs/transcript.jsonl"))

    for tr_file in target_transcripts:
        if not tr_file.exists():
            continue
        conv_id = tr_file.parents[2].name

        try:
            with open(tr_file, "r", encoding="utf-8") as f:
                for line in f:
                    if not line.strip():
                        continue
                    try:
                        d = json.loads(line)
                    except json.JSONDecodeError:
                        continue

                    created_at_str = d.get("created_at")
                    if not created_at_str:
                        continue
                    try:
                        dt = parse_iso_timestamp(created_at_str)
                    except Exception:
                        continue

                    step_idx = d.get("step_index", 0)
                    step_type = d.get("type", "")
                    status = d.get("status", "DONE")
                    tool_calls = d.get("tool_calls", [])
                    content = d.get("content", "")

                    if tool_calls:
                        for tc in tool_calls:
                            t_name = tc.get("name", "")
                            t_args = tc.get("args", {})
                            summary = t_args.get("toolSummary", "") or t_args.get("toolAction", "")
                            detail = ""
                            if t_name == "run_command":
                                detail = t_args.get("CommandLine", "")
                            elif t_name in ["replace_file_content", "write_to_file", "view_file"]:
                                detail = t_args.get("TargetFile", "") or t_args.get("AbsolutePath", "")
                            elif t_name == "search_web":
                                detail = t_args.get("query", "")
                            elif t_name == "read_url_content":
                                detail = t_args.get("Url", "")

                            records.append({
                                "conversation_id": conv_id,
                                "timestamp": dt,
                                "step_index": step_idx,
                                "type": t_name,
                                "summary": summary,
                                "detail": detail,
                                "status": status,
                                "raw_args": t_args,
                            })
                    elif step_type == "USER_INPUT":
                        # Record user prompt
                        clean_content = content.replace("<USER_REQUEST>", "").replace("</USER_REQUEST>", "").strip()
                        records.append({
                            "conversation_id": conv_id,
                            "timestamp": dt,
                            "step_index": step_idx,
                            "type": "USER_INPUT",
                            "summary": "User Request",
                            "detail": clean_content[:150],
                            "status": status,
                            "raw_args": {},
                        })
        except Exception:
            continue

    return records

# ─── FILTER & RENDER ENGINE ──────────────────────────────────────────────────

def main():
    parser = argparse.ArgumentParser(description="Antigravity CLI Action Audit Trail")
    parser.add_argument("-q", "--query", help="Cari kata kunci di dalam ringkasan, perintah, atau path berkas")
    parser.add_argument("-t", "--type", help="Filter berdasarkan jenis tool (run_command, replace_file_content, USER_INPUT, dll)")
    parser.add_argument("--conversation", help="Filter ID percakapan tertentu")
    parser.add_argument("--since", help="Filter aksi dalam rentang waktu terakhir (misal: 30m, 2h, 24h, 7d)")
    parser.add_argument("--date", help="Filter tanggal tertentu (format: YYYY-MM-DD)")
    parser.add_argument("--errors-only", action="store_true", help="Hanya tampilkan aksi yang berstatus ERROR atau gagal")
    parser.add_argument("--sort", choices=["asc", "desc"], default="desc", help="Urutan waktu (default: desc / terbaru)")
    parser.add_argument("--limit", type=int, default=30, help="Jumlah rekaman maksimal (default: 30, 0 untuk semua)")
    parser.add_argument("--json", action="store_true", help="Cetak hasil dalam format JSON")

    args = parser.parse_args()

    records = scan_all_transcripts(args.conversation)
    if not records:
        print("ℹ️ Tidak ada jejak transcript yang ditemukan.")
        sys.exit(0)

    # Filter Time Range
    now_utc = datetime.datetime.now(datetime.timezone.utc)
    if args.since:
        delta = parse_since_duration(args.since)
        cutoff = now_utc - delta
        records = [r for r in records if r["timestamp"] >= cutoff]

    if args.date:
        records = [r for r in records if r["timestamp"].strftime("%Y-%m-%d") == args.date]

    # Filter Type
    if args.type:
        type_needle = args.type.strip().lower()
        records = [r for r in records if type_needle in r["type"].lower()]

    # Filter Errors
    if args.errors_only:
        records = [r for r in records if r["status"].upper() == "ERROR"]

    # Filter Keyword Search
    if args.query:
        q = args.query.strip().lower()
        records = [
            r for r in records
            if q in r["type"].lower() or q in r["summary"].lower() or q in r["detail"].lower()
        ]

    # Sort
    is_desc = args.sort == "desc"
    records.sort(key=lambda x: x["timestamp"], reverse=is_desc)

    # Limit
    if args.limit > 0:
        records = records[:args.limit]

    # Output JSON
    if args.json:
        serializable = []
        for r in records:
            item = dict(r)
            item["timestamp"] = r["timestamp"].isoformat()
            serializable.append(item)
        print(json.dumps(serializable, indent=2))
        sys.exit(0)

    # Output Terminal Table
    print(f"\n🛡️ === ANTIGRAVITY CLI AGENT AUDIT TRAIL ===")
    print(f"Total Ditemukan: {len(records)} aksi | Filter: Query='{args.query or '*'}' Type='{args.type or '*'}' Since='{args.since or '*'}'")
    divider = "=" * 110
    print(divider)
    print(f"| {'Waktu (WIB)':<20} | {'Tipe Aksi':<18} | {'Status':<8} | {'Ringkasan & Detail Perintah/Berkas':<55} |")
    print(divider)

    for r in records:
        time_str = format_local_time(r["timestamp"])
        t_type = r["type"]
        status = r["status"]
        status_badge = "✓ OK" if status == "DONE" else "🚨 ERR"
        
        detail_snippet = f"{r['summary']}: {r['detail']}" if r['detail'] else r['summary']
        detail_snippet = detail_snippet.replace('\n', ' ')
        if len(detail_snippet) > 55:
            detail_snippet = detail_snippet[:52] + "..."

        print(f"| {time_str:<20} | {t_type[:18]:<18} | {status_badge:<8} | {detail_snippet:<55} |")

    print(divider)
    print("")

if __name__ == "__main__":
    main()
