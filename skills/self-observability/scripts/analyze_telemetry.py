#!/usr/bin/env python3
"""
Autonomous Telemetry and Tool Usage Analyzer for Aina.
Extracts empirical usage statistics from SQLite action audits and AGY brain transcripts,
evaluates performance bottlenecks, and provides Built-in vs Custom Skill recommendations.
"""

import os
import sys
import glob
import json
import sqlite3
import argparse
from collections import Counter, defaultdict
from typing import Dict, List, Any, Optional

DEFAULT_DB_PATH = os.environ.get("DATABASE_PATH", "/root/projects/aina/data/aina.db")
DEFAULT_BRAIN_DIR = os.environ.get("APP_DATA_DIR", "/root/.gemini/antigravity-cli")
if not DEFAULT_BRAIN_DIR.endswith("brain"):
    DEFAULT_BRAIN_DIR = os.path.join(DEFAULT_BRAIN_DIR, "brain")


def get_sqlite_telemetry(db_path: str = DEFAULT_DB_PATH) -> Dict[str, Any]:
    """Extracts high-level interaction metrics and tool usage from whatsapp_action_audits."""
    if not os.path.isfile(db_path):
        return {"error": f"Database not found at {db_path}", "total_actions": 0}

    try:
        conn = sqlite3.connect(db_path)
        conn.row_factory = sqlite3.Row
        cur = conn.cursor()

        # Check if table exists
        cur.execute("SELECT name FROM sqlite_master WHERE type='table' AND name='whatsapp_action_audits'")
        if not cur.fetchone():
            return {"error": "whatsapp_action_audits table does not exist", "total_actions": 0}

        cur.execute("SELECT COUNT(*) FROM whatsapp_action_audits")
        total_actions = cur.fetchone()[0]

        cur.execute("SELECT COUNT(*) FROM whatsapp_action_audits WHERE status='success'")
        successful_actions = cur.fetchone()[0]

        cur.execute("SELECT COUNT(*) FROM whatsapp_action_audits WHERE status='failed' OR error_message IS NOT NULL")
        failed_actions = cur.fetchone()[0]

        cur.execute("SELECT AVG(duration_seconds) FROM whatsapp_action_audits WHERE duration_seconds IS NOT NULL")
        avg_duration = cur.fetchone()[0] or 0.0

        # Distinct senders / chats
        cur.execute("SELECT sender_jid, COUNT(*) as cnt FROM whatsapp_action_audits GROUP BY sender_jid ORDER BY cnt DESC LIMIT 5")
        top_senders = [{"sender": r["sender_jid"], "count": r["cnt"]} for r in cur.fetchall()]

        # Tool calls recorded in audits
        cur.execute("SELECT tools_invoked FROM whatsapp_action_audits WHERE tools_invoked != '[]' AND tools_invoked IS NOT NULL")
        tool_counts = Counter()
        for r in cur.fetchall():
            try:
                tools = json.loads(r["tools_invoked"])
                if isinstance(tools, list):
                    for t in tools:
                        tool_counts[t] += 1
            except Exception:
                pass

        # Usecases
        cur.execute("SELECT usecase, COUNT(*) as cnt FROM whatsapp_action_audits WHERE usecase IS NOT NULL GROUP BY usecase ORDER BY cnt DESC")
        usecases = [{"usecase": r["usecase"], "count": r["cnt"]} for r in cur.fetchall()]

        conn.close()

        return {
            "total_actions": total_actions,
            "successful_actions": successful_actions,
            "failed_actions": failed_actions,
            "avg_duration_seconds": round(avg_duration, 2),
            "top_senders": top_senders,
            "tool_frequencies_in_audit": dict(tool_counts.most_common(15)),
            "usecases": usecases,
        }
    except Exception as e:
        return {"error": str(e), "total_actions": 0}


def get_transcript_telemetry(brain_dir: str = DEFAULT_BRAIN_DIR, max_files: int = 100) -> Dict[str, Any]:
    """Parses Antigravity CLI transcript logs to extract deep sub-step tool invocations."""
    transcripts = glob.glob(os.path.join(brain_dir, "*", ".system_generated", "logs", "transcript.jsonl"))
    
    total_transcripts = len(transcripts)
    total_steps = 0
    tool_counter = Counter()
    command_prefix_counter = Counter()
    viewed_files_counter = Counter()
    error_counter = Counter()

    for t_path in transcripts[:max_files]:
        try:
            with open(t_path, "r", encoding="utf-8") as f:
                for line in f:
                    line = line.strip()
                    if not line:
                        continue
                    total_steps += 1
                    data = json.loads(line)
                    status = data.get("status")
                    tool_calls = data.get("tool_calls", [])

                    for tc in tool_calls:
                        tname = tc.get("name")
                        if not tname:
                            continue
                        tool_counter[tname] += 1

                        if status == "ERROR":
                            error_counter[tname] += 1

                        args = tc.get("args") or tc.get("arguments") or tc.get("parameters") or {}
                        if isinstance(args, dict):
                            # Analyze run_command patterns
                            if tname == "run_command":
                                cmd = args.get("CommandLine", "").strip()
                                if cmd:
                                    # Group by first binary/command token
                                    parts = [p.strip("\"'") for p in cmd.split() if p.strip("\"'")]
                                    prefix = parts[0] if parts else "unknown"
                                    if prefix in ("python", "python3", "bun", "cargo", "git", "bash", "sh") and len(parts) > 1:
                                        prefix = f"{parts[0]} {parts[1]}"
                                    command_prefix_counter[prefix] += 1

                            # Analyze view_file patterns
                            elif tname == "view_file":
                                path = args.get("AbsolutePath", "").strip()
                                if path:
                                    filename = os.path.basename(path)
                                    viewed_files_counter[filename] += 1

        except Exception:
            continue

    return {
        "total_transcripts_scanned": total_transcripts,
        "total_steps_scanned": total_steps,
        "top_tools": dict(tool_counter.most_common(15)),
        "frequent_command_prefixes": dict(command_prefix_counter.most_common(12)),
        "frequently_viewed_files": dict(viewed_files_counter.most_common(10)),
        "error_rates_per_tool": dict(error_counter.most_common(5)),
    }


def generate_recommendations(sql_data: Dict[str, Any], trans_data: Dict[str, Any]) -> List[Dict[str, Any]]:
    """Synthesizes recommendations for shortcuts and evaluates Built-in vs Custom Skill."""
    recs = []
    top_tools = trans_data.get("top_tools", {})
    cmd_prefixes = trans_data.get("frequent_command_prefixes", {})

    # 1. Repetitive CLI / Script executions
    for prefix, count in cmd_prefixes.items():
        if count >= 10:
            is_git = "git" in prefix
            is_cargo = "cargo" in prefix
            is_python = "python" in prefix or "python3" in prefix

            category = "Custom Skill"
            rationale = "Workflow spesifik / otomasi script berulang. Sangat cocok dibuatkan skill modular agar tidak perlu merestart core daemon dan mudah dikustomisasi."
            action_hint = f"Jalankan: python3 skills/skill-builder/scripts/skill_scaffolder.py init <nama_skill>"

            if is_cargo or is_git:
                category = "Built-in (Rust Core) or CLI Shortcut"
                rationale = "Operasi dasar repo/workspace. Jika frekuensinya sangat masif dan memerlukan kecepatan mikrodetik, dapat dijadikan subcommand built-in di CLI `aina`."
                action_hint = "Tambahkan command handler di `src/adapters/driving/cli/commands/`"

            recs.append({
                "target": prefix,
                "usage_frequency": count,
                "recommended_architecture": category,
                "rationale": rationale,
                "action": action_hint
            })

    # 2. General Tool Bottlenecks
    if top_tools.get("view_file", 0) > 1000 and top_tools.get("replace_file_content", 0) > 500:
        recs.append({
            "target": "Code Editing & File Inspection Loop",
            "usage_frequency": f"view_file ({top_tools.get('view_file')}), edit ({top_tools.get('replace_file_content')})",
            "recommended_architecture": "Built-in Architecture (Rust)",
            "rationale": "Siklus pembacaan dan penyuntingan kode sangat masif. Menggunakan caching representasi AST atau in-memory file index pada core Rust akan menghemat ratusan round-trip latency.",
            "action": "Integrasikan ripgrep / FTS5 indexed file cache di `src/core/domain/`"
        })

    # 3. Third-party or Integration Skills
    for tool_name, count in top_tools.items():
        if "secret" in tool_name or "event" in tool_name or "search" in tool_name:
            recs.append({
                "target": f"Tool {tool_name}",
                "usage_frequency": count,
                "recommended_architecture": "Custom Skill (`data/custom-skills`)",
                "rationale": "Integrasi eksternal memerlukan fleksibilitas vendor dan decoupled failure domain.",
                "action": "Kelola via folder `skills/` dan sinkronkan dengan `USER_SKILLS_REPO`."
            })

    return recs


def trace_tool_context(
    query: str,
    db_path: str = DEFAULT_DB_PATH,
    brain_dir: str = DEFAULT_BRAIN_DIR,
    limit: int = 5,
    chat_filter: Optional[str] = None
) -> Dict[str, Any]:
    """
    Deep-dives into past WhatsApp interactions and AGY transcripts to uncover the exact
    conversations, user prompts, surrounding chat dialogue, and AI thinking traces
    that prompted the tool invocations.
    """
    whatsapp_matches = []
    transcript_matches = []

    # 1. Search SQLite whatsapp_action_audits & message_history
    if os.path.isfile(db_path):
        try:
            conn = sqlite3.connect(db_path)
            conn.row_factory = sqlite3.Row
            cur = conn.cursor()

            sql = """
                SELECT id, message_id, chat_jid, sender_jid, decision,
                       conversation_id, status, input_text, response_text,
                       duration_seconds, tools_invoked, usecase, created_at_epoch
                FROM whatsapp_action_audits
                WHERE (tools_invoked LIKE ? OR input_text LIKE ? OR usecase LIKE ?)
            """
            params = [f"%{query}%", f"%{query}%", f"%{query}%"]
            if chat_filter:
                sql += " AND (chat_jid = ? OR sender_jid = ?)"
                params.extend([chat_filter, chat_filter])
            sql += " ORDER BY id DESC LIMIT ?"
            params.append(limit)

            cur.execute(sql, params)
            rows = cur.fetchall()

            for r in rows:
                row_dict = dict(r)
                chat_jid = row_dict["chat_jid"]
                surrounding = []
                try:
                    cur.execute(
                        "SELECT id, sender_jid, text, is_from_me, created_at FROM message_history WHERE chat_jid = ? ORDER BY id DESC LIMIT 5",
                        [chat_jid]
                    )
                    surrounding = [dict(sr) for sr in reversed(cur.fetchall())]
                except Exception:
                    pass

                row_dict["surrounding_chat"] = surrounding
                whatsapp_matches.append(row_dict)

            conn.close()
        except Exception:
            pass

    # 2. Search AGY Brain Transcripts
    transcripts = glob.glob(os.path.join(brain_dir, "*", ".system_generated", "logs", "transcript.jsonl"))
    q_lower = query.lower()

    for t_path in transcripts:
        if len(transcript_matches) >= limit:
            break
        try:
            session_id = os.path.basename(os.path.dirname(os.path.dirname(os.path.dirname(t_path))))
            with open(t_path, "r", encoding="utf-8") as f:
                current_user_prompt = ""
                for line in f:
                    if not line.strip():
                        continue
                    data = json.loads(line)
                    if data.get("type") == "USER_INPUT":
                        current_user_prompt = data.get("content", "").strip()

                    tool_calls = data.get("tool_calls", [])
                    matched_call = None
                    for tc in tool_calls:
                        tname = tc.get("name", "")
                        args_str = json.dumps(tc.get("args", {})).lower()
                        if q_lower in tname.lower() or q_lower in args_str:
                            matched_call = tc
                            break

                    if matched_call:
                        thinking = data.get("thinking", "").strip()
                        transcript_matches.append({
                            "session_id": session_id,
                            "user_prompt": current_user_prompt,
                            "thinking_trace": thinking[:300] + ("..." if len(thinking) > 300 else ""),
                            "matched_tool": matched_call.get("name"),
                            "tool_action": (matched_call.get("args") or {}).get("toolAction"),
                            "tool_summary": (matched_call.get("args") or {}).get("toolSummary"),
                            "arguments": matched_call.get("args"),
                            "timestamp": data.get("created_at"),
                        })
                        if len(transcript_matches) >= limit:
                            break
        except Exception:
            continue

    return {
        "query": query,
        "whatsapp_audit_matches": whatsapp_matches,
        "transcript_deep_dive_matches": transcript_matches,
    }


def main():
    parser = argparse.ArgumentParser(description="Aina Telemetry & Tool Usage Analyzer")
    parser.add_argument("command", choices=["summary", "tools", "bottlenecks", "recommendations", "trace"], help="Analysis mode")
    parser.add_argument("query", nargs="?", default="", help="Query or tool name for 'trace' deep-dive")
    parser.add_argument("--limit", type=int, default=5, help="Result limit for trace")
    parser.add_argument("--chat", default=None, help="Filter WhatsApp chat JID")
    parser.add_argument("--json", action="store_true", help="Output raw JSON")
    parser.add_argument("--db", default=DEFAULT_DB_PATH, help="Path to SQLite database")
    parser.add_argument("--brain", default=DEFAULT_BRAIN_DIR, help="Path to AGY brain directory")

    args = parser.parse_args()

    sql_data = get_sqlite_telemetry(args.db)
    trans_data = get_transcript_telemetry(args.brain)

    if args.command == "summary":
        res = {
            "action_audits": sql_data,
            "agent_transcripts": {
                "total_conversations": trans_data.get("total_transcripts_scanned", 0),
                "total_steps": trans_data.get("total_steps_scanned", 0),
                "top_tools": trans_data.get("top_tools", {}),
            }
        }
        if args.json:
            print(json.dumps(res, indent=2))
        else:
            print("📊 RINGKASAN TELEMETRI & OBSERVABILITY AINA\n" + "="*45)
            print(f"• Total Aksi WhatsApp Tercatat : {sql_data.get('total_actions', 0)}")
            print(f"• Total Sesi Percakapan Brain  : {trans_data.get('total_transcripts_scanned', 0)}")
            print(f"• Total Langkah Eksekusi Alat  : {trans_data.get('total_steps_scanned', 0)}")
            print(f"• Rata-rata Durasi Aksi        : {sql_data.get('avg_duration_seconds', 0.0)}s")
            print("\n🛠️ Top Alat Paling Banyak Digunakan:")
            for t, c in list(trans_data.get("top_tools", {}).items())[:7]:
                print(f"  - {t:<22}: {c} kali")

    elif args.command == "tools":
        res = {
            "top_tools": trans_data.get("top_tools", {}),
            "command_prefixes": trans_data.get("frequent_command_prefixes", {}),
            "frequently_viewed_files": trans_data.get("frequently_viewed_files", {}),
            "error_rates": trans_data.get("error_rates_per_tool", {}),
        }
        if args.json:
            print(json.dumps(res, indent=2))
        else:
            print("🛠️ DETAIL PENGGUNAAN ALAT (TOOL USAGE VOLUME)\n" + "="*45)
            for t, c in trans_data.get("top_tools", {}).items():
                errs = trans_data.get("error_rates_per_tool", {}).get(t, 0)
                err_str = f" ({errs} errors)" if errs else ""
                print(f"  • {t:<24}: {c:>5} panggilan{err_str}")
            
            print("\n💻 Pola Prefix Command Terbanyak:")
            for p, c in trans_data.get("frequent_command_prefixes", {}).items():
                print(f"  • {p:<24}: {c:>5} eksekusi")

    elif args.command == "bottlenecks":
        res = {
            "command_prefixes": trans_data.get("frequent_command_prefixes", {}),
            "viewed_files": trans_data.get("frequently_viewed_files", {}),
            "error_rates": trans_data.get("error_rates_per_tool", {}),
        }
        if args.json:
            print(json.dumps(res, indent=2))
        else:
            print("⚠️ ANALISIS BOTTLENECK & FREKUENSI TINGGI\n" + "="*45)
            print("File yang Paling Sering Dibaca Berulang Kali:")
            for f, c in trans_data.get("frequently_viewed_files", {}).items():
                print(f"  • {f:<30}: {c} kali")
            print("\nPerintah yang Sering Dipanggil Berulang:")
            for p, c in trans_data.get("frequent_command_prefixes", {}).items():
                print(f"  • {p:<30}: {c} kali")

    elif args.command == "recommendations":
        recs = generate_recommendations(sql_data, trans_data)
        if args.json:
            print(json.dumps(recs, indent=2))
        else:
            print("💡 REKOMENDASI PENINGKATAN & KEPUTUSAN ARSITEKTUR\n" + "="*50)
            if not recs:
                print("Belum ada pola berulang signifikan yang memerlukan shortcut khusus.")
            for i, r in enumerate(recs, 1):
                print(f"{i}. Target: {r['target']} (Frekuensi: {r['usage_frequency']})")
                print(f"   Arsitektur Direkomendasikan: [{r['recommended_architecture']}]")
                print(f"   Alasan                     : {r['rationale']}")
                print(f"   Aksi Konkret               : {r['action']}\n")

    elif args.command == "trace":
        if not args.query:
            print("Error: query/tool_name wajib diberikan untuk perintah 'trace'. Contoh: analyze_telemetry.py trace curl")
            sys.exit(1)

        trace_data = trace_tool_context(args.query, args.db, args.brain, args.limit, args.chat)
        if args.json:
            print(json.dumps(trace_data, indent=2, default=str))
        else:
            print(f"🔍 DEEP DIVE: LATAR BELAKANG PERCAKAPAN UNTUK '{args.query}'\n" + "="*60)
            wa_matches = trace_data.get("whatsapp_audit_matches", [])
            trans_matches = trace_data.get("transcript_deep_dive_matches", [])

            if not wa_matches and not trans_matches:
                print(f"Tidak ditemukan jejak pemanggilan alat atau percakapan yang cocok dengan query '{args.query}'.")
                return

            if wa_matches:
                print(f"📱 Temuan di WhatsApp Action Audits ({len(wa_matches)} kejadian):")
                for i, m in enumerate(wa_matches, 1):
                    print(f"\n[{i}] ID Aksi: #{m.get('id')} | Chat: {m.get('chat_jid')}")
                    print(f"    • Pesan User : {m.get('input_text')}")
                    print(f"    • Alat       : {m.get('tools_invoked')}")
                    print(f"    • Respons AI : {(m.get('response_text') or '')[:120]}...")
                    if m.get("surrounding_chat"):
                        print("    💬 Konteks Obrolan Terkait (Message History):")
                        for sc in m["surrounding_chat"]:
                            role = "Aina" if sc.get("is_from_me") else "User"
                            print(f"       - [{role}]: {sc.get('text', '')[:100]}")

            if trans_matches:
                print(f"\n🧠 Temuan di AGY Brain Transcripts ({len(trans_matches)} sesi):")
                for i, tm in enumerate(trans_matches, 1):
                    print(f"\n[{i}] Sesi: {tm.get('session_id')[:18]}... | Waktu: {tm.get('timestamp')}")
                    clean_prompt = tm.get('user_prompt', '').replace('\n', ' ')
                    if len(clean_prompt) > 140:
                        clean_prompt = clean_prompt[:137] + "..."
                    print(f"    • User Prompt  : {clean_prompt}")
                    if tm.get('thinking_trace'):
                        clean_th = tm.get('thinking_trace', '').replace('\n', ' ')
                        print(f"    • Nalar/Alasan : {clean_th}")
                    print(f"    • Alat & Action: {tm.get('matched_tool')} ({tm.get('tool_action') or '-'})")
                    if tm.get('arguments'):
                        arg_preview = json.dumps(tm.get('arguments'))
                        if len(arg_preview) > 120:
                            arg_preview = arg_preview[:117] + "..."
                        print(f"    • Argumen Tool : {arg_preview}")


if __name__ == "__main__":
    main()

