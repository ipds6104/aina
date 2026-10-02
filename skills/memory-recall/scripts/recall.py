#!/usr/bin/env python3
"""
Aina Episodic Memory Recall Utility
Searches and retrieves past conversations, user statements, past decisions, and context from SQLite message history.
"""

import sys
import os
import sqlite3
import json
import argparse
from typing import Optional, List, Dict, Any

def get_db_path() -> str:
    db_env = os.environ.get("DATABASE_PATH")
    if db_env and os.path.exists(db_env):
        return db_env
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

STOPWORDS = {
    "apa", "ya", "yaa", "sih", "soal", "tentang", "yang", "dan", "di", "ke", "dari",
    "ini", "itu", "ada", "gak", "tidak", "bisa", "tolong", "kah", "dong", "kemarin",
    "tadi", "kita", "kamu", "saya", "aku", "the", "is", "and", "what", "which", "how"
}

def search_memory(query: str, chat_jid: Optional[str] = None, sender_jid: Optional[str] = None, limit: int = 5) -> List[Dict[str, Any]]:
    db_path = get_db_path()
    if not os.path.exists(db_path):
        return []

    conn = sqlite3.connect(f"file:{db_path}?mode=ro", uri=True)
    conn.row_factory = sqlite3.Row
    cursor = conn.cursor()

    clean_query = query.strip()
    if not clean_query:
        return []

    keywords = [
        w.strip(".,?!:;\"'").lower()
        for w in clean_query.split()
        if len(w) >= 3 and w.lower() not in STOPWORDS
    ]

    results = []
    scope_clauses = []
    scope_params = []
    if chat_jid and sender_jid:
        scope_clauses.append("(chat_jid = ? OR sender_jid = ? OR chat_jid IN (SELECT DISTINCT chat_jid FROM message_history WHERE sender_jid = ?))")
        scope_params.extend([chat_jid, sender_jid, sender_jid])
    elif chat_jid:
        scope_clauses.append("chat_jid = ?")
        scope_params.append(chat_jid)
    elif sender_jid:
        scope_clauses.append("sender_jid = ?")
        scope_params.append(sender_jid)

    if keywords:
        kw_clauses = ["text LIKE ?" for _ in keywords]
        sql_kw = " OR ".join(kw_clauses)
        params_kw = [f"%{kw}%" for kw in keywords]

        if scope_clauses:
            sql = f"""
                SELECT id, chat_jid, sender_jid, text, is_from_me, created_at
                FROM message_history
                WHERE {scope_clauses[0]} AND ({sql_kw})
                ORDER BY id DESC
                LIMIT ?
            """
            full_params = scope_params + params_kw + [limit]
        else:
            sql = f"""
                SELECT id, chat_jid, sender_jid, text, is_from_me, created_at
                FROM message_history
                WHERE ({sql_kw})
                ORDER BY id DESC
                LIMIT ?
            """
            full_params = params_kw + [limit]

        cursor.execute(sql, full_params)
        rows = cursor.fetchall()
        for row in rows:
            results.append(dict(row))
    else:
        param = f"%{clean_query}%"
        if scope_clauses:
            sql = f"""
                SELECT id, chat_jid, sender_jid, text, is_from_me, created_at
                FROM message_history
                WHERE {scope_clauses[0]} AND text LIKE ?
                ORDER BY id DESC
                LIMIT ?
            """
            cursor.execute(sql, tuple(scope_params + [param, limit]))
        else:
            sql = """
                SELECT id, chat_jid, sender_jid, text, is_from_me, created_at
                FROM message_history
                WHERE text LIKE ?
                ORDER BY id DESC
                LIMIT ?
            """
            cursor.execute(sql, (param, limit))
        rows = cursor.fetchall()
        for row in rows:
            results.append(dict(row))

    conn.close()
    return results

def get_recent(chat_jid: Optional[str] = None, sender_jid: Optional[str] = None, limit: int = 5) -> List[Dict[str, Any]]:
    db_path = get_db_path()
    if not os.path.exists(db_path):
        return []

    conn = sqlite3.connect(f"file:{db_path}?mode=ro", uri=True)
    conn.row_factory = sqlite3.Row
    cursor = conn.cursor()

    if chat_jid and sender_jid:
        sql = """
            SELECT id, chat_jid, sender_jid, text, is_from_me, created_at
            FROM message_history
            WHERE chat_jid = ? OR sender_jid = ? OR chat_jid IN (SELECT DISTINCT chat_jid FROM message_history WHERE sender_jid = ?)
            ORDER BY id DESC
            LIMIT ?
        """
        cursor.execute(sql, (chat_jid, sender_jid, sender_jid, limit))
    elif chat_jid:
        sql = """
            SELECT id, chat_jid, sender_jid, text, is_from_me, created_at
            FROM message_history
            WHERE chat_jid = ?
            ORDER BY id DESC
            LIMIT ?
        """
        cursor.execute(sql, (chat_jid, limit))
    elif sender_jid:
        sql = """
            SELECT id, chat_jid, sender_jid, text, is_from_me, created_at
            FROM message_history
            WHERE sender_jid = ?
            ORDER BY id DESC
            LIMIT ?
        """
        cursor.execute(sql, (sender_jid, limit))
    else:
        sql = """
            SELECT id, chat_jid, sender_jid, text, is_from_me, created_at
            FROM message_history
            ORDER BY id DESC
            LIMIT ?
        """
        cursor.execute(sql, (limit,))

    rows = cursor.fetchall()
    results = [dict(row) for row in rows]
    results.reverse()
    conn.close()
    return results


def format_markdown(messages: List[Dict[str, Any]], title: str) -> str:
    if not messages:
        return f"### {title}\nTidak ditemukan catatan riwayat percakapan yang cocok.\n"

    out = [f"### {title} ({len(messages)} pesan ditemukan)"]
    for m in messages:
        role = "Aina (Bot)" if m.get("is_from_me") else f"User ({m.get('sender_jid', 'Pengirim')})"
        time_str = m.get("created_at", "")
        text = m.get("text", "").strip()
        out.append(f"- **[{time_str}] {role}**:\n  > {text}")
    return "\n\n".join(out)

def main():
    parser = argparse.ArgumentParser(description="Aina Episodic Memory Recall CLI")
    subparsers = parser.add_subparsers(dest="subcommand", required=True)

    search_parser = subparsers.add_parser("search", help="Search past messages by query")
    search_parser.add_argument("query", type=str, help="Search query or keywords")
    search_parser.add_argument("--chat", type=str, default=None, help="Filter by WhatsApp Chat JID")
    search_parser.add_argument("--limit", type=int, default=5, help="Max results (default: 5)")
    search_parser.add_argument("--json", action="store_true", help="Output as JSON")

    recent_parser = subparsers.add_parser("recent", help="Retrieve most recent messages")
    recent_parser.add_argument("--chat", type=str, default=None, help="Filter by WhatsApp Chat JID")
    recent_parser.add_argument("--limit", type=int, default=5, help="Max results (default: 5)")
    recent_parser.add_argument("--json", action="store_true", help="Output as JSON")

    args = parser.parse_args()

    if args.subcommand == "search":
        results = search_memory(args.query, chat_jid=args.chat, limit=args.limit)
        if args.json:
            print(json.dumps(results, indent=2))
        else:
            print(format_markdown(results, f"Hasil Pencarian Memori: \"{args.query}\""))
    elif args.subcommand == "recent":
        results = get_recent(chat_jid=args.chat, limit=args.limit)
        if args.json:
            print(json.dumps(results, indent=2))
        else:
            print(format_markdown(results, "Riwayat Percakapan Terakhir"))

if __name__ == "__main__":
    main()
