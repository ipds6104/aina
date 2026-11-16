#!/usr/bin/env python3
"""
wa_tool.py - Lightweight CLI for Aina to interact with Whatsmeow Gateway REST API.
Zero external dependencies (uses Python standard library and system curl).

Features:
- Deterministic exit codes: 0 = success, non-zero (1) = typed failure
- Automatic JID normalization (Indonesian 08xx -> 628xx@s.whatsapp.net, +62xx, etc.)
- Auto-retry with jittered exponential backoff for cold-contact handshake / transient gateway errors
- Response unwrapping: message_id, status, and recipient are always flattened at root level
- Native broadcast command with humanized delays, anti-ban jitter, and comprehensive audit reporting
- Native verify command to confirm message delivery and presence in gateway store
"""

import os
import sys
import json
import re
import time
import random
import argparse
import subprocess
import urllib.request
import urllib.error
import urllib.parse
import csv
from typing import Optional, Dict, Any, List, Union

DEFAULT_BASE_URL = "http://localhost:3000"
DEFAULT_TIMEOUT_SECS = 30
CURRENT_ARGS = None

def normalize_jid(target: str) -> str:
    """
    Normalize various phone/JID representations into a canonical WhatsApp JID:
    - 0896-9403-0497 -> 6289694030497@s.whatsapp.net
    - +62 896 9403 0497 -> 6289694030497@s.whatsapp.net
    - 6289694030497 -> 6289694030497@s.whatsapp.net
    - 089694030497@s.whatsapp.net -> 6289694030497@s.whatsapp.net (fixes invalid 08 prefix)
    - 120363xxx@g.us -> 120363xxx@g.us (group JID preserved)
    - 87097809592405@lid -> 87097809592405@lid (LID preserved)
    - status@broadcast -> status@broadcast (broadcast preserved)
    """
    cleaned = target.strip()
    if not cleaned:
        return ""
    if "@g.us" in cleaned or "@lid" in cleaned or cleaned == "status@broadcast":
        return cleaned

    suffix = "@s.whatsapp.net"
    if cleaned.endswith(suffix):
        cleaned = cleaned[:-len(suffix)]

    # Strip non-digit characters
    digits = re.sub(r"[^0-9]", "", cleaned)
    if not digits:
        return target.strip()

    if digits.startswith("08"):
        digits = "628" + digits[2:]
    elif digits.startswith("8"):
        digits = "62" + digits
    elif digits.startswith("0") and len(digits) > 1:
        digits = "62" + digits[1:]

    return f"{digits}{suffix}"

def get_config(args=None) -> tuple[str, str]:
    if args is None:
        args = CURRENT_ARGS

    is_companion = getattr(args, "companion", False) if args else False
    cli_base = getattr(args, "base_url", None) if args else None
    cli_key = getattr(args, "api_key", None) if args else None

    if is_companion:
        base_url = (
            cli_base
            or os.getenv("WHATSMEOW_COMPANION_BASE_URL")
            or os.getenv("WHATSMEOW_COMPANION_URL")
        )
        api_key = (
            cli_key
            or os.getenv("WHATSMEOW_COMPANION_API_KEY")
            or os.getenv("COMPANION_API_KEY")
        )
    else:
        base_url = (
            cli_base
            or os.getenv("WHATSMEOW_BASE_URL")
            or os.getenv("WHATSMEOW_URL")
        )
        api_key = (
            cli_key
            or os.getenv("WHATSMEOW_API_KEY")
            or os.getenv("API_KEY")
        )

    if not base_url or not api_key:
        candidates = [
            "/app/config/config.yaml",
            "config/config.yaml",
            "../config/config.yaml",
            "../../config/config.yaml",
        ]
        for c in candidates:
            if os.path.isfile(c):
                try:
                    with open(c, "r", encoding="utf-8") as f:
                        lines = f.readlines()
                    in_wa = False
                    for line in lines:
                        clean = line.strip()
                        if clean.startswith("whatsmeow:"):
                            in_wa = True
                        elif in_wa and clean and not clean.startswith("#"):
                            if not line.startswith(" ") and not line.startswith("\t"):
                                in_wa = False
                            elif is_companion:
                                if "companion_base_url:" in clean and not base_url:
                                    base_url = clean.split("companion_base_url:", 1)[1].strip().strip('"').strip("'")
                                elif "companion_api_key:" in clean and not api_key:
                                    api_key = clean.split("companion_api_key:", 1)[1].strip().strip('"').strip("'")
                            else:
                                if "base_url:" in clean and not base_url:
                                    base_url = clean.split("base_url:", 1)[1].strip().strip('"').strip("'")
                                elif "api_key:" in clean and not api_key:
                                    api_key = clean.split("api_key:", 1)[1].strip().strip('"').strip("'")
                except Exception:
                    pass

    return (base_url or ("" if is_companion else DEFAULT_BASE_URL)).rstrip("/"), (api_key or "")

def make_request(
    method: str,
    endpoint: str,
    data: Optional[Dict[str, Any]] = None,
    args: Any = None,
    timeout: int = DEFAULT_TIMEOUT_SECS
) -> Dict[str, Any]:
    if args is None:
        args = CURRENT_ARGS
    base_url, api_key = get_config(args)
    if not base_url:
        is_comp = getattr(args, "companion", False) if args else False
        gateway_type = "Companion" if is_comp else "Primary"
        env_var = "WHATSMEOW_COMPANION_BASE_URL" if is_comp else "WHATSMEOW_BASE_URL"
        return {
            "success": False,
            "error": True,
            "message": f"{gateway_type} WhatsApp Gateway URL is not configured. Please set {env_var} in environment or config."
        }
    url = f"{base_url}{endpoint}"
    
    headers = {
        "Accept": "application/json",
        "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36 Aina-Agent/1.0"
    }
    if api_key:
        headers["X-API-Key"] = api_key
        headers["Authorization"] = f"Bearer {api_key}"

    body = None
    if data is not None:
        body = json.dumps(data).encode("utf-8")
        headers["Content-Type"] = "application/json"

    req = urllib.request.Request(url, data=body, headers=headers, method=method)
    
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            content = resp.read().decode("utf-8")
            if not content.strip():
                return {"success": True, "error": False, "status": "ok", "code": resp.status}
            parsed = json.loads(content)
            if isinstance(parsed, dict):
                # Promote nested data fields to root for ergonomic caller access
                if "data" in parsed and isinstance(parsed["data"], dict):
                    for k, v in parsed["data"].items():
                        if k not in parsed:
                            parsed[k] = v
                if "success" not in parsed:
                    parsed["success"] = True
                parsed["error"] = False
            return parsed
    except urllib.error.HTTPError as e:
        err_msg = e.read().decode("utf-8", errors="replace")
        detail = err_msg
        try:
            err_json = json.loads(err_msg)
            if isinstance(err_json, dict):
                detail = err_json.get("error") or err_json.get("message") or err_msg
        except Exception:
            pass
        return {
            "success": False,
            "error": True,
            "status_code": e.code,
            "message": f"HTTP {e.code}: {detail}"
        }
    except Exception as e:
        return {
            "success": False,
            "error": True,
            "message": f"Connection error to {url}: {str(e)}"
        }

def output_result(res: Dict[str, Any], allow_error: bool = False):
    """
    Print formatted JSON result to stdout and enforce deterministic exit codes.
    0 = success, 1 = failure (unless allow_error is set).
    """
    print(json.dumps(res, indent=2, ensure_ascii=False))
    if not allow_error:
        if res.get("error") or res.get("success") is False:
            sys.exit(1)

def send_text_with_retry(
    recipient: str,
    text: str,
    retries: int = 3,
    retry_delay: float = 3.0,
    timeout: int = DEFAULT_TIMEOUT_SECS,
    args: Any = None
) -> Dict[str, Any]:
    """
    Send text message with automatic retry and exponential jittered backoff.
    Crucial for cold-contact handshake negotiation and transient rate limits.
    """
    norm_recipient = normalize_jid(recipient)
    payload = {
        "recipient": norm_recipient,
        "content": text,
        "to": norm_recipient,
        "message": text
    }

    last_res = None
    for attempt in range(1, retries + 1):
        res = make_request("POST", "/api/v1/messages/send-text", payload, args=args, timeout=timeout)
        if res.get("error") and res.get("status_code") == 404:
            # Fallback to legacy endpoint /send/message
            res = make_request("POST", "/send/message", payload, args=args, timeout=timeout)

        # Success condition
        if not res.get("error") and res.get("success", True):
            res["success"] = True
            res["error"] = False
            res["recipient"] = norm_recipient
            res["attempts"] = attempt
            return res

        last_res = res
        if attempt < retries:
            backoff = retry_delay * (1.2 ** (attempt - 1)) + random.uniform(0.3, 0.9)
            err_msg = res.get("message") or res.get("error") or "Unknown error"
            sys.stderr.write(
                f"[wa_tool:retry] Attempt {attempt}/{retries} for {norm_recipient} failed ({err_msg}). Retrying in {backoff:.1f}s...\n"
            )
            sys.stderr.flush()
            time.sleep(backoff)

    if last_res is None:
        last_res = {"success": False, "error": True, "message": "Failed with zero attempts"}
    last_res["attempts"] = retries
    last_res["recipient"] = norm_recipient
    return last_res

def render_safe_template(template_str: str, data: Dict[str, Any]) -> str:
    """Render template placeholders safely without crashing on missing keys."""
    class SafeDict(dict):
        def __missing__(self, key):
            return "{" + key + "}"
    return template_str.format_map(SafeDict(data))

def cmd_recent(args):
    jid = normalize_jid(args.jid)
    limit = args.limit
    endpoint = f"/api/v1/chats/{urllib.parse.quote(jid)}/messages?limit={limit}"
    res = make_request("GET", endpoint)
    output_result(res, getattr(args, "allow_error", False))

def cmd_groups(args):
    res = make_request("GET", "/api/v1/groups")
    output_result(res, getattr(args, "allow_error", False))

def cmd_group_info(args):
    jid = normalize_jid(args.jid)
    endpoint = f"/api/v1/groups/{urllib.parse.quote(jid)}"
    res = make_request("GET", endpoint)
    output_result(res, getattr(args, "allow_error", False))

def cmd_sync_members(args):
    jid = normalize_jid(args.jid)
    endpoint = f"/api/v1/groups/{urllib.parse.quote(jid)}"
    res = make_request("GET", endpoint)
    if res.get("error") or res.get("success") is False:
        output_result(res, getattr(args, "allow_error", False))
        return

    data = res.get("data") or res
    participants = data.get("participants") or []
    group_name = data.get("name") or data.get("subject") or jid

    db_path = os.environ.get("DATABASE_PATH")
    if not db_path:
        for cand in ["data/aina.db", "/app/data/aina.db", "../data/aina.db", "../../data/aina.db"]:
            if os.path.exists(cand):
                db_path = cand
                break
    if not db_path:
        db_path = "data/aina.db"

    synced_count = 0
    try:
        import sqlite3
        conn = sqlite3.connect(db_path)
        c = conn.cursor()
        c.execute("""
            CREATE TABLE IF NOT EXISTS group_memberships (
                group_jid TEXT NOT NULL,
                user_jid TEXT NOT NULL,
                user_name TEXT,
                role_in_group TEXT NOT NULL DEFAULT 'member',
                last_synced_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                PRIMARY KEY (group_jid, user_jid)
            )
        """)
        for p in participants:
            user_jid = p.get("jid") or p.get("id")
            if not user_jid:
                continue
            user_name = p.get("name") or p.get("display_name")
            role = "admin" if p.get("is_admin") or p.get("is_super_admin") else "member"
            c.execute("""
                INSERT INTO group_memberships (group_jid, user_jid, user_name, role_in_group, last_synced_at)
                VALUES (?, ?, ?, ?, CURRENT_TIMESTAMP)
                ON CONFLICT(group_jid, user_jid) DO UPDATE SET
                    user_name = COALESCE(excluded.user_name, group_memberships.user_name),
                    role_in_group = excluded.role_in_group,
                    last_synced_at = CURRENT_TIMESTAMP
            """, (jid, user_jid, user_name, role))
            synced_count += 1
        conn.commit()
        conn.close()
    except Exception as e:
        err_res = {"success": False, "error": True, "message": f"Database sync error: {e}"}
        output_result(err_res, getattr(args, "allow_error", False))
        return

    out = {
        "success": True,
        "error": False,
        "group_jid": jid,
        "group_name": group_name,
        "synced_participants": synced_count,
        "database": db_path
    }
    output_result(out, getattr(args, "allow_error", False))

def cmd_export_backup(args):
    jid = normalize_jid(args.jid)
    endpoint = f"/api/v1/groups/{urllib.parse.quote(jid)}/backup"
    payload = {
        "limit": args.limit,
        "include_media": args.include_media,
        "format": "json"
    }
    res = make_request("POST", endpoint, payload)

    if args.out and not res.get("error") and res.get("backup_id"):
        backup_id = res["backup_id"]
        base_url, api_key = get_config()
        download_url = f"{base_url}/api/v1/backups/{backup_id}"
        req = urllib.request.Request(download_url, headers={"X-API-Key": api_key, "Authorization": f"Bearer {api_key}"})
        try:
            with urllib.request.urlopen(req, timeout=60) as dl:
                os.makedirs(os.path.dirname(os.path.abspath(args.out)), exist_ok=True)
                with open(args.out, "wb") as f:
                    f.write(dl.read())
            res["saved_to"] = os.path.abspath(args.out)
        except Exception as e:
            res["download_warning"] = f"Backup created as ID {backup_id}, but failed to auto-download: {e}"

    output_result(res, getattr(args, "allow_error", False))

def cmd_send_text(args):
    recipient = normalize_jid(args.to)
    text = args.text
    retries = getattr(args, "retry", 3)
    retry_delay = getattr(args, "retry_delay", 3.0)
    allow_error = getattr(args, "allow_error", False)

    res = send_text_with_retry(
        recipient=recipient,
        text=text,
        retries=retries,
        retry_delay=retry_delay,
        timeout=DEFAULT_TIMEOUT_SECS,
        args=args
    )
    output_result(res, allow_error)

def cmd_broadcast(args):
    """
    Send messages to multiple recipients with humanized jitter, per-contact retries,
    anti-ban protections, and complete verification reporting.
    """
    recipients_input = args.recipients.strip()
    raw_recipients = []

    # Parse recipients from JSON file, inline JSON, or CSV
    if os.path.isfile(recipients_input):
        if recipients_input.endswith(".csv"):
            with open(recipients_input, "r", encoding="utf-8") as f:
                reader = csv.DictReader(f)
                raw_recipients = list(reader)
        else:
            with open(recipients_input, "r", encoding="utf-8") as f:
                raw_recipients = json.load(f)
    elif recipients_input.startswith("[") or recipients_input.startswith("{"):
        raw_recipients = json.loads(recipients_input)
    else:
        # Comma-separated list of phones
        raw_recipients = [r.strip() for r in recipients_input.split(",") if r.strip()]

    if isinstance(raw_recipients, dict) and "recipients" in raw_recipients:
        raw_recipients = raw_recipients["recipients"]

    if not isinstance(raw_recipients, list) or not raw_recipients:
        err_res = {"success": False, "error": True, "message": "Recipient list is empty or invalid format."}
        output_result(err_res, getattr(args, "allow_partial", False))
        return

    # Parse template
    template_str = args.template
    if os.path.isfile(template_str):
        with open(template_str, "r", encoding="utf-8") as f:
            template_str = f.read()

    # Dry-run validation
    if getattr(args, "dry_run", False):
        preview_list = []
        for i, item in enumerate(raw_recipients, 1):
            item_dict = item if isinstance(item, dict) else {"phone": str(item)}
            phone = (
                item_dict.get("phone")
                or item_dict.get("nohp")
                or item_dict.get("no_hp")
                or item_dict.get("jid")
                or item_dict.get("recipient")
                or item_dict.get("to")
                or item_dict.get("nomor")
                or ""
            )
            target_jid = normalize_jid(str(phone))
            item_dict["jid"] = target_jid
            item_dict["no"] = i
            msg = render_safe_template(template_str, item_dict)
            preview_list.append({
                "no": i,
                "recipient": target_jid,
                "data": item_dict,
                "preview": msg[:120] + ("..." if len(msg) > 120 else "")
            })
        out = {
            "success": True,
            "dry_run": True,
            "total": len(preview_list),
            "previews": preview_list
        }
        output_result(out, allow_error=True)
        return

    delay_min = max(1.0, getattr(args, "delay_min", 3.0))
    delay_max = max(delay_min, getattr(args, "delay_max", 6.0))
    retries = getattr(args, "retry", 3)
    retry_delay = getattr(args, "retry_delay", 3.0)
    allow_partial = getattr(args, "allow_partial", False)

    total = len(raw_recipients)
    results = []
    sent_count = 0
    failed_count = 0

    sys.stderr.write(f"[wa_tool:broadcast] Starting broadcast to {total} recipient(s)...\n")
    sys.stderr.flush()

    for i, item in enumerate(raw_recipients, 1):
        item_dict = item if isinstance(item, dict) else {"phone": str(item)}
        phone = (
            item_dict.get("phone")
            or item_dict.get("nohp")
            or item_dict.get("no_hp")
            or item_dict.get("jid")
            or item_dict.get("recipient")
            or item_dict.get("to")
            or item_dict.get("nomor")
            or ""
        )
        target_jid = normalize_jid(str(phone))
        name = item_dict.get("name") or item_dict.get("nama") or target_jid
        item_dict["jid"] = target_jid
        item_dict["no"] = i

        rendered_msg = render_safe_template(template_str, item_dict)

        res = send_text_with_retry(
            recipient=target_jid,
            text=rendered_msg,
            retries=retries,
            retry_delay=retry_delay,
            timeout=DEFAULT_TIMEOUT_SECS,
            args=args
        )

        msg_id = res.get("message_id") or res.get("id") or (res.get("data", {}).get("message_id") if isinstance(res.get("data"), dict) else None)
        is_ok = not res.get("error") and res.get("success", True) and bool(msg_id)

        record = {
            "no": i,
            "name": name,
            "recipient": target_jid,
            "status": "sent" if is_ok else "failed",
            "message_id": msg_id if is_ok else None,
            "attempts": res.get("attempts", 1),
            "error": res.get("message") if not is_ok else None
        }
        results.append(record)

        if is_ok:
            sent_count += 1
            sys.stderr.write(f"[{i}/{total}] SENT -> {name} ({target_jid}) [ID: {msg_id}]\n")
        else:
            failed_count += 1
            sys.stderr.write(f"[{i}/{total}] FAILED -> {name} ({target_jid}): {res.get('message')}\n")
        sys.stderr.flush()

        # Humanized jitter delay between recipients (skips after last one)
        if i < total:
            sleep_duration = random.uniform(delay_min, delay_max)
            time.sleep(sleep_duration)

    summary = {
        "success": (failed_count == 0),
        "total": total,
        "sent_count": sent_count,
        "failed_count": failed_count,
        "recipients": results
    }

    if getattr(args, "out", None):
        try:
            out_file = os.path.abspath(args.out)
            os.makedirs(os.path.dirname(out_file), exist_ok=True)
            with open(out_file, "w", encoding="utf-8") as f:
                json.dump(summary, f, indent=2, ensure_ascii=False)
            summary["report_saved_to"] = out_file
        except Exception as e:
            summary["report_save_warning"] = str(e)

    output_result(summary, allow_partial)

def cmd_verify(args):
    """
    Verify whether a message ID has been recorded and confirmed in gateway store.
    """
    msg_id = args.id.strip()
    chat_jid = normalize_jid(args.jid) if getattr(args, "jid", None) else ""
    allow_error = getattr(args, "allow_error", False)

    endpoint = f"/api/v1/chats/{urllib.parse.quote(chat_jid)}/messages?limit=100" if chat_jid else "/api/v1/messages/recent?limit=100"
    raw = make_request("GET", endpoint)

    messages = raw.get("data") if isinstance(raw.get("data"), list) else (raw if isinstance(raw, list) else [])
    matched = None
    for m in messages:
        if isinstance(m, dict) and m.get("id") == msg_id:
            matched = m
            break

    if matched:
        out = {
            "success": True,
            "error": False,
            "verified": True,
            "found": True,
            "message_id": msg_id,
            "chat_jid": matched.get("chat_jid") or matched.get("chat") or chat_jid,
            "sender_jid": matched.get("sender_jid") or matched.get("sender"),
            "timestamp": matched.get("timestamp"),
            "is_from_me": matched.get("is_from_me"),
            "text": matched.get("text")
        }
        output_result(out, allow_error=False)
    else:
        out = {
            "success": False,
            "error": True,
            "verified": False,
            "found": False,
            "message_id": msg_id,
            "message": f"Message ID {msg_id} was not found in gateway recent messages store."
        }
        output_result(out, allow_error=allow_error)

def cmd_reaction(args):
    recipient = normalize_jid(args.to)
    msg_id = args.id.strip()
    emoji = args.emoji.strip()
    payload = {
        "recipient": recipient,
        "to": recipient,
        "chat_jid": recipient,
        "message_id": msg_id,
        "reaction": emoji,
        "emoji": emoji,
    }
    res = make_request("POST", "/api/v1/messages/reaction", payload)
    if res.get("error") and res.get("status_code") == 404:
        # Fallback to legacy endpoint /send/reaction
        res = make_request("POST", "/send/reaction", payload)
    output_result(res, getattr(args, "allow_error", False))

def cmd_search(args):
    jid = normalize_jid(args.jid)
    limit = args.limit or 100
    query = (args.query or "").lower().strip()
    sender = (args.sender or "").strip()
    has_media = args.has_media

    endpoint = f"/api/v1/chats/{urllib.parse.quote(jid)}/messages?limit={limit}"
    raw = make_request("GET", endpoint)

    if raw.get("error") or not isinstance(raw.get("data"), list):
        output_result(raw, getattr(args, "allow_error", False))
        return

    messages = raw["data"]
    filtered = []
    for m in messages:
        text = (m.get("text") or "").lower()
        if query and query not in text:
            continue
        if sender and sender not in (m.get("sender_jid") or ""):
            continue
        if has_media is not None:
            if m.get("has_media") != has_media:
                continue
        filtered.append(m)

    result = {
        "success": True,
        "error": False,
        "chat_jid": jid,
        "query": query,
        "total_scanned": len(messages),
        "total_matched": len(filtered),
        "matches": filtered
    }
    output_result(result, getattr(args, "allow_error", False))

def cmd_stats(args):
    session = make_request("GET", "/api/v1/session/status")
    antiban = make_request("GET", "/api/v1/antiban/stats")
    res = {
        "success": True,
        "error": False,
        "session": session.get("data") if session.get("success") else session,
        "antiban": antiban.get("data") if antiban.get("success") else antiban
    }
    output_result(res, getattr(args, "allow_error", False))

def cmd_download_media(args):
    direct_path = args.direct_path.strip()
    out_file = os.path.abspath(args.out)

    payload = {"direct_path": direct_path}
    if args.media_key:
        payload["media_key"] = args.media_key
    if args.type:
        payload["type"] = args.type

    res = make_request("POST", "/api/v1/media/download", payload)
    if not res.get("error") and res.get("data"):
        data = res["data"]
        os.makedirs(os.path.dirname(out_file), exist_ok=True)
        if isinstance(data, str) and len(data) > 20:
            try:
                import base64
                with open(out_file, "wb") as f:
                    f.write(base64.b64decode(data))
                res["saved_to"] = out_file
            except Exception as e:
                res["save_error"] = str(e)
    output_result(res, getattr(args, "allow_error", False))

def cmd_send_media(args):
    recipient = normalize_jid(args.to)
    base_url, api_key = get_config(args)
    endpoint = f"{base_url}/api/v1/messages/send-media"
    file_path = os.path.abspath(args.file)

    if not os.path.exists(file_path):
        err_res = {"success": False, "error": True, "message": f"File not found: {file_path}"}
        output_result(err_res, getattr(args, "allow_error", False))
        return

    media_type = args.type
    if not media_type or media_type == "auto":
        ext = os.path.splitext(file_path)[1].lower()
        if ext in [".jpg", ".jpeg", ".png", ".webp", ".gif"]:
            media_type = "image"
        elif ext in [".mp4", ".mov", ".mkv", ".avi"]:
            media_type = "video"
        elif ext in [".mp3", ".ogg", ".opus", ".m4a", ".wav"]:
            media_type = "audio"
        else:
            media_type = "document"

    curl_cmd = [
        "curl", "-s", "-X", "POST", endpoint,
        "-H", f"X-API-Key: {api_key}",
        "-H", f"Authorization: Bearer {api_key}",
        "-F", f"recipient={recipient}",
        "-F", f"type={media_type}",
        "-F", f"file=@{file_path}"
    ]
    if args.caption:
        curl_cmd.extend(["-F", f"caption={args.caption}"])
    if getattr(args, "reply_to", None):
        curl_cmd.extend(["-F", f"reply_to_id={args.reply_to}"])

    retries = getattr(args, "retry", 3)
    retry_delay = getattr(args, "retry_delay", 3.0)
    last_res = None

    for attempt in range(1, retries + 1):
        try:
            proc = subprocess.run(curl_cmd, capture_output=True, text=True, timeout=60)
            if proc.returncode == 0:
                try:
                    out_json = json.loads(proc.stdout)
                    if isinstance(out_json, dict):
                        if "data" in out_json and isinstance(out_json["data"], dict):
                            for k, v in out_json["data"].items():
                                if k not in out_json:
                                    out_json[k] = v
                        out_json["success"] = True
                        out_json["error"] = False
                        out_json["recipient"] = recipient
                        out_json["attempts"] = attempt
                    output_result(out_json, getattr(args, "allow_error", False))
                    return
                except json.JSONDecodeError:
                    success_res = {"success": True, "error": False, "status": "ok", "raw_output": proc.stdout, "recipient": recipient}
                    output_result(success_res, getattr(args, "allow_error", False))
                    return
            else:
                last_res = {"success": False, "error": True, "message": proc.stderr or proc.stdout}
        except Exception as e:
            last_res = {"success": False, "error": True, "message": str(e)}

        if attempt < retries:
            backoff = retry_delay * (1.2 ** (attempt - 1)) + random.uniform(0.3, 0.9)
            sys.stderr.write(f"[wa_tool:retry] Media attempt {attempt}/{retries} failed. Retrying in {backoff:.1f}s...\n")
            sys.stderr.flush()
            time.sleep(backoff)

    if last_res is None:
        last_res = {"success": False, "error": True, "message": "Media dispatch failed with zero attempts"}
    output_result(last_res, getattr(args, "allow_error", False))

def cmd_profile_picture_get(args):
    params = []
    if args.jid:
        params.append(f"jid={urllib.parse.quote(normalize_jid(args.jid))}")
    if args.preview:
        params.append("preview=true")
    qs = f"?{'&'.join(params)}" if params else ""
    res = make_request("GET", f"/api/v1/user/profile-picture{qs}")
    output_result(res, getattr(args, "allow_error", False))

def cmd_profile_picture_set(args):
    base_url, api_key = get_config(args)
    endpoint = f"{base_url}/api/v1/user/profile-picture"
    file_path = os.path.abspath(args.file)
    if not os.path.exists(file_path):
        err_res = {"success": False, "error": True, "message": f"File not found: {file_path}"}
        output_result(err_res, getattr(args, "allow_error", False))
        return

    curl_cmd = [
        "curl", "-s", "-X", "POST", endpoint,
        "-H", f"X-API-Key: {api_key}",
        "-H", f"Authorization: Bearer {api_key}",
        "-F", f"file=@{file_path}"
    ]
    if args.jid:
        curl_cmd.extend(["-F", f"jid={normalize_jid(args.jid)}"])

    try:
        proc = subprocess.run(curl_cmd, capture_output=True, text=True, timeout=60)
        if proc.returncode == 0:
            try:
                out_json = json.loads(proc.stdout)
                output_result(out_json, getattr(args, "allow_error", False))
            except json.JSONDecodeError:
                output_result({"success": True, "error": False, "status": "ok", "raw_output": proc.stdout}, getattr(args, "allow_error", False))
        else:
            output_result({"success": False, "error": True, "message": proc.stderr or proc.stdout}, getattr(args, "allow_error", False))
    except Exception as e:
        output_result({"success": False, "error": True, "message": str(e)}, getattr(args, "allow_error", False))

def cmd_profile_picture_remove(args):
    endpoint = "/api/v1/user/profile-picture"
    if args.jid:
        endpoint += f"?jid={urllib.parse.quote(normalize_jid(args.jid))}"
    res = make_request("DELETE", endpoint)
    output_result(res, getattr(args, "allow_error", False))

def cmd_about_set(args):
    res = make_request("POST", "/api/v1/user/about", {"status": args.status})
    output_result(res, getattr(args, "allow_error", False))

# Import StatusSafetyGuard from scripts/persona/safety if available
try:
    from scripts.persona.safety import StatusSafetyGuard
except Exception:
    _aina_root = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", ".."))
    if _aina_root not in sys.path:
        sys.path.insert(0, _aina_root)
    try:
        from scripts.persona.safety import StatusSafetyGuard
    except Exception:
        StatusSafetyGuard = None

def validate_safe_status(text=None, file_path=None):
    if StatusSafetyGuard:
        if text is not None:
            is_safe, reason = StatusSafetyGuard.validate_status_text(text)
            if not is_safe:
                return False, f"Teks status tidak aman / mengandung error: {reason}"
        if file_path is not None:
            is_safe, reason = StatusSafetyGuard.validate_status_media(file_path)
            if not is_safe:
                return False, f"Media status tidak valid / rusak: {reason}"
        return True, ""
    else:
        if text is not None:
            clean = str(text).strip().lower()
            if not clean or any(e in clean for e in ["error", "exception", "failed", "traceback", "500", "502", "503", "quota"]):
                return False, "Teks status mengandung indikasi pesan error."
        if file_path is not None:
            if not os.path.exists(file_path) or os.path.getsize(file_path) < 1024:
                return False, "Berkas media tidak ditemukan atau berukuran 0/rusak."
        return True, ""

def cmd_status_send_text(args):
    clean_text = args.text
    if StatusSafetyGuard:
        clean_text = StatusSafetyGuard.sanitize_caption(args.text)
    is_safe, reason = validate_safe_status(text=clean_text)
    if not is_safe:
        err_res = {
            "success": False,
            "error": True,
            "safety_violation": True,
            "message": f"Ditolak Status Safety Guard: {reason}"
        }
        output_result(err_res, getattr(args, "allow_error", False))
        return

    payload = {
        "type": "text",
        "text": clean_text
    }
    if args.background:
        payload["background_color"] = args.background
    if args.font is not None:
        payload["font"] = args.font
    res = make_request("POST", "/api/v1/status/send-story", payload)
    output_result(res, getattr(args, "allow_error", False))

def cmd_status_send_media(args):
    base_url, api_key = get_config(args)
    endpoint = f"{base_url}/api/v1/status/send-story"
    file_path = os.path.abspath(args.file)

    is_safe_media, media_err = validate_safe_status(file_path=file_path)
    if not is_safe_media:
        err_res = {
            "success": False,
            "error": True,
            "safety_violation": True,
            "message": f"Ditolak Status Safety Guard: {media_err}"
        }
        output_result(err_res, getattr(args, "allow_error", False))
        return

    if args.caption:
        if StatusSafetyGuard:
            args.caption = StatusSafetyGuard.sanitize_caption(args.caption)
        is_safe_caption, cap_err = validate_safe_status(text=args.caption)
        if not is_safe_caption:
            err_res = {
                "success": False,
                "error": True,
                "safety_violation": True,
                "message": f"Ditolak Status Safety Guard (Caption tidak aman): {cap_err}"
            }
            output_result(err_res, getattr(args, "allow_error", False))
            return

    media_type = args.type
    if not media_type or media_type == "auto":
        ext = os.path.splitext(file_path)[1].lower()
        if ext in [".mp4", ".mov", ".mkv", ".avi"]:
            media_type = "video"
        else:
            media_type = "image"

    curl_cmd = [
        "curl", "-s", "-X", "POST", endpoint,
        "-H", f"X-API-Key: {api_key}",
        "-H", f"Authorization: Bearer {api_key}",
        "-F", f"type={media_type}",
        "-F", f"file=@{file_path}"
    ]
    if args.caption:
        curl_cmd.extend(["-F", f"caption={args.caption}", "-F", f"text={args.caption}"])

    try:
        proc = subprocess.run(curl_cmd, capture_output=True, text=True, timeout=60)
        if proc.returncode == 0:
            try:
                out_json = json.loads(proc.stdout)
                output_result(out_json, getattr(args, "allow_error", False))
            except json.JSONDecodeError:
                output_result({"success": True, "error": False, "status": "ok", "raw_output": proc.stdout}, getattr(args, "allow_error", False))
        else:
            output_result({"success": False, "error": True, "message": proc.stderr or proc.stdout}, getattr(args, "allow_error", False))
    except Exception as e:
        output_result({"success": False, "error": True, "message": str(e)}, getattr(args, "allow_error", False))

def cmd_status_list(args):
    params = []
    if args.limit:
        params.append(f"limit={args.limit}")
    if args.active_only:
        params.append("active_only=true")
    if args.contacts:
        params.append("self=false")
    qs = f"?{'&'.join(params)}" if params else ""
    res = make_request("GET", f"/api/v1/status/list{qs}")
    output_result(res, getattr(args, "allow_error", False))

def cmd_revoke(args):
    payload = {"message_id": args.id}
    if args.chat_jid:
        payload["chat_jid"] = normalize_jid(args.chat_jid)
    res = make_request("POST", "/api/v1/messages/revoke", payload)
    output_result(res, getattr(args, "allow_error", False))

def main():
    global CURRENT_ARGS

    common_parser = argparse.ArgumentParser(add_help=False)
    common_parser.add_argument("--base-url", help="Override Whatsmeow Gateway Base URL")
    common_parser.add_argument("--api-key", help="Override Whatsmeow API Key")
    common_parser.add_argument("--companion", action="store_true", help="Route request to the Companion WhatsApp gateway (personal account)")
    common_parser.add_argument("--allow-error", action="store_true", help="Do not exit with non-zero exit code on failure")

    parser = argparse.ArgumentParser(
        description="Whatsmeow CLI helper tool for Aina agent",
        parents=[common_parser]
    )
    subparsers = parser.add_subparsers(dest="subcommand", required=True)

    # recent
    p_recent = subparsers.add_parser("recent", help="Fetch recent messages from a chat", parents=[common_parser])
    p_recent.add_argument("--jid", required=True, help="Chat or Group JID")
    p_recent.add_argument("--limit", type=int, default=20, help="Number of messages to retrieve (default: 20)")
    p_recent.set_defaults(func=cmd_recent)

    # groups
    p_groups = subparsers.add_parser("groups", help="List all joined groups", parents=[common_parser])
    p_groups.set_defaults(func=cmd_groups)

    # group-info
    p_ginfo = subparsers.add_parser("group-info", help="Get metadata and participants of a group", parents=[common_parser])
    p_ginfo.add_argument("--jid", required=True, help="Group JID (ends with @g.us)")
    p_ginfo.set_defaults(func=cmd_group_info)

    # sync-members
    p_sync = subparsers.add_parser("sync-members", help="Synchronize participants of a group into local SQLite group_memberships", parents=[common_parser])
    p_sync.add_argument("--jid", required=True, help="Group JID (ends with @g.us)")
    p_sync.set_defaults(func=cmd_sync_members)

    # export-backup
    p_backup = subparsers.add_parser("export-backup", help="Export chat backup of a group", parents=[common_parser])
    p_backup.add_argument("--jid", required=True, help="Group JID")
    p_backup.add_argument("--limit", type=int, default=1000, help="Max messages to backup")
    p_backup.add_argument("--include-media", action="store_true", help="Include media metadata")
    p_backup.add_argument("--out", help="Optional output filepath to save JSON backup")
    p_backup.set_defaults(func=cmd_export_backup)

    # send-text
    p_send_text = subparsers.add_parser("send-text", help="Send a WhatsApp text message", parents=[common_parser])
    p_send_text.add_argument("--to", required=True, help="Recipient phone or JID (e.g. 0896-xxx or 628xxx@s.whatsapp.net)")
    p_send_text.add_argument("--text", required=True, help="Message text content")
    p_send_text.add_argument("--retry", type=int, default=3, help="Max retry attempts on transient handshake/network error (default: 3)")
    p_send_text.add_argument("--retry-delay", type=float, default=3.0, help="Base retry delay in seconds (default: 3.0)")
    p_send_text.set_defaults(func=cmd_send_text)

    # broadcast
    p_broadcast = subparsers.add_parser(
        "broadcast",
        help="Send personalized text messages to multiple recipients with anti-ban delay, retries, and verification",
        parents=[common_parser]
    )
    p_broadcast.add_argument("--recipients", required=True, help="Filepath (JSON or CSV) or inline JSON string with recipients list")
    p_broadcast.add_argument("--template", required=True, help="Message template string with {name}, {phone}, etc., or path to template file")
    p_broadcast.add_argument("--delay-min", type=float, default=3.0, help="Minimum humanized jitter delay between recipients (default: 3.0s)")
    p_broadcast.add_argument("--delay-max", type=float, default=6.0, help="Maximum humanized jitter delay between recipients (default: 6.0s)")
    p_broadcast.add_argument("--retry", type=int, default=3, help="Max retry attempts per recipient (default: 3)")
    p_broadcast.add_argument("--retry-delay", type=float, default=3.0, help="Base retry delay in seconds (default: 3.0)")
    p_broadcast.add_argument("--dry-run", action="store_true", help="Validate and preview rendered messages without sending")
    p_broadcast.add_argument("--out", help="Optional filepath to save JSON broadcast execution summary")
    p_broadcast.add_argument("--allow-partial", action="store_true", help="Exit with 0 even if some recipients failed")
    p_broadcast.set_defaults(func=cmd_broadcast)

    # verify
    p_verify = subparsers.add_parser("verify", help="Verify whether a message ID exists and is recorded in gateway store", parents=[common_parser])
    p_verify.add_argument("--id", required=True, help="Message ID to verify (e.g. 3EB0XXXXX)")
    p_verify.add_argument("--jid", default="", help="Optional chat JID to accelerate search scope")
    p_verify.set_defaults(func=cmd_verify)

    # reaction
    p_reaction = subparsers.add_parser("reaction", help="Send a reaction to a WhatsApp message", parents=[common_parser])
    p_reaction.add_argument("--to", required=True, help="Recipient or chat JID")
    p_reaction.add_argument("--id", required=True, help="Message ID to react to")
    p_reaction.add_argument("--emoji", default="\U0001F64F", help="Emoji character (default: folded hands)")
    p_reaction.set_defaults(func=cmd_reaction)

    # send-media
    p_media = subparsers.add_parser("send-media", help="Send media or document file", parents=[common_parser])
    p_media.add_argument("--to", required=True, help="Recipient phone or JID")
    p_media.add_argument("--file", required=True, help="Path to file on disk")
    p_media.add_argument("--type", default="auto", choices=["auto", "image", "video", "audio", "document"], help="Media type (default: auto)")
    p_media.add_argument("--caption", help="Optional caption text")
    p_media.add_argument("--reply-to", help="Optional message ID being quoted")
    p_media.add_argument("--retry", type=int, default=3, help="Max retry attempts (default: 3)")
    p_media.add_argument("--retry-delay", type=float, default=3.0, help="Retry delay in seconds (default: 3.0)")
    p_media.set_defaults(func=cmd_send_media)

    # search
    p_search = subparsers.add_parser("search", help="Search and filter messages in a chat", parents=[common_parser])
    p_search.add_argument("--jid", required=True, help="Chat or Group JID")
    p_search.add_argument("--query", default="", help="Keyword text to search for (case-insensitive)")
    p_search.add_argument("--sender", default="", help="Filter by sender phone or JID")
    p_search.add_argument("--has-media", type=lambda v: v.lower() in ["true", "1", "yes"], default=None, help="Filter only messages with media (true/false)")
    p_search.add_argument("--limit", type=int, default=100, help="Max messages to scan (default: 100)")
    p_search.set_defaults(func=cmd_search)

    # stats
    p_stats = subparsers.add_parser("stats", help="Get gateway connection and antiban rate-limit stats", parents=[common_parser])
    p_stats.set_defaults(func=cmd_stats)

    # download-media
    p_dl = subparsers.add_parser("download-media", help="Download media from WhatsApp CDN", parents=[common_parser])
    p_dl.add_argument("--direct-path", required=True, help="Media direct_path starting with slash")
    p_dl.add_argument("--media-key", help="Optional media decryption key")
    p_dl.add_argument("--type", choices=["image", "video", "audio", "document"], help="Optional media type")
    p_dl.add_argument("--out", required=True, help="Destination filepath on disk")
    p_dl.set_defaults(func=cmd_download_media)

    # profile-picture-get
    p_pp_get = subparsers.add_parser("profile-picture-get", help="Get profile picture URL for a user, group, or self", parents=[common_parser])
    p_pp_get.add_argument("--jid", default="", help="Target JID (defaults to self if empty)")
    p_pp_get.add_argument("--preview", action="store_true", help="Fetch low-res preview thumbnail instead of full image")
    p_pp_get.set_defaults(func=cmd_profile_picture_get)

    # profile-picture-set
    p_pp_set = subparsers.add_parser("profile-picture-set", help="Update profile picture for self or group", parents=[common_parser])
    p_pp_set.add_argument("--file", required=True, help="Path to avatar image file (JPG/PNG)")
    p_pp_set.add_argument("--jid", default="", help="Target JID (leave empty for self, or specify group JID)")
    p_pp_set.set_defaults(func=cmd_profile_picture_set)

    # profile-picture-remove
    p_pp_del = subparsers.add_parser("profile-picture-remove", help="Remove profile picture for self or group", parents=[common_parser])
    p_pp_del.add_argument("--jid", default="", help="Target JID (defaults to self if empty)")
    p_pp_del.set_defaults(func=cmd_profile_picture_remove)

    # about-set
    p_about = subparsers.add_parser("about-set", help="Update WhatsApp About / Bio status text", parents=[common_parser])
    p_about.add_argument("--status", required=True, help="New About status text")
    p_about.set_defaults(func=cmd_about_set)

    # status-send-text
    p_st_text = subparsers.add_parser("status-send-text", help="Post an ephemeral 24-hour text status story", parents=[common_parser])
    p_st_text.add_argument("--text", required=True, help="Status story text")
    p_st_text.add_argument("--background", help="Optional ARGB background color (hex, e.g. 0xFF25D366)")
    p_st_text.add_argument("--font", type=int, choices=[1, 2, 3, 4, 5], help="Optional font style (1 to 5)")
    p_st_text.set_defaults(func=cmd_status_send_text)

    # status-send-media
    p_st_media = subparsers.add_parser("status-send-media", help="Post an ephemeral 24-hour media status story", parents=[common_parser])
    p_st_media.add_argument("--file", required=True, help="Path to image or video file")
    p_st_media.add_argument("--caption", help="Optional status caption")
    p_st_media.add_argument("--type", choices=["auto", "image", "video"], default="auto", help="Media type (default: auto)")
    p_st_media.set_defaults(func=cmd_status_send_media)

    # status-list
    p_st_list = subparsers.add_parser("status-list", help="List status stories created by bot (or contacts)", parents=[common_parser])
    p_st_list.add_argument("--limit", type=int, default=20, help="Max status stories to list (default: 20)")
    p_st_list.add_argument("--active-only", action="store_true", help="Only show active stories (< 24 hours)")
    p_st_list.add_argument("--contacts", action="store_true", help="List status stories from contacts instead of own")
    p_st_list.set_defaults(func=cmd_status_list)

    # revoke / status-revoke
    p_revoke = subparsers.add_parser("revoke", help="Revoke/delete a sent message or status story for everyone", parents=[common_parser])
    p_revoke.add_argument("--id", required=True, help="Message ID or Status Story ID to revoke")
    p_revoke.add_argument("--chat-jid", default="", help="Chat JID (leave empty or status@broadcast for stories)")
    p_revoke.set_defaults(func=cmd_revoke)

    p_st_revoke = subparsers.add_parser("status-revoke", help="Revoke/delete a posted status story", parents=[common_parser])
    p_st_revoke.add_argument("--id", required=True, help="Status Story ID to revoke")
    p_st_revoke.set_defaults(func=lambda args: cmd_revoke(argparse.Namespace(id=args.id, chat_jid="status@broadcast")))

    args = parser.parse_args()
    CURRENT_ARGS = args
    args.func(args)

if __name__ == "__main__":
    main()
