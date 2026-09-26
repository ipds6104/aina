#!/usr/bin/env python3
"""
wa_tool.py - Lightweight CLI for Aina to interact with Whatsmeow Gateway REST API.
Zero external dependencies (uses Python standard library and system curl).
"""

import os
import sys
import json
import argparse
import subprocess
import urllib.request
import urllib.error
import urllib.parse
from typing import Optional, Dict, Any

DEFAULT_BASE_URL = "http://localhost:3000"
CURRENT_ARGS = None

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

def make_request(method: str, endpoint: str, data: Optional[Dict[str, Any]] = None, args: Any = None) -> Dict[str, Any]:
    if args is None:
        args = CURRENT_ARGS
    base_url, api_key = get_config(args)
    if not base_url:
        is_comp = getattr(args, "companion", False) if args else False
        gateway_type = "Companion" if is_comp else "Primary"
        env_var = "WHATSMEOW_COMPANION_BASE_URL" if is_comp else "WHATSMEOW_BASE_URL"
        return {
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
        with urllib.request.urlopen(req, timeout=10) as resp:
            content = resp.read().decode("utf-8")
            if not content.strip():
                return {"status": "ok", "code": resp.status}
            return json.loads(content)
    except urllib.error.HTTPError as e:
        err_msg = e.read().decode("utf-8", errors="replace")
        return {
            "error": True,
            "status_code": e.code,
            "message": f"HTTP {e.code}: {err_msg}"
        }
    except Exception as e:
        return {
            "error": True,
            "message": f"Connection error to {url}: {str(e)}"
        }

def cmd_recent(args):
    jid = args.jid.strip()
    limit = args.limit
    endpoint = f"/api/v1/chats/{urllib.parse.quote(jid)}/messages?limit={limit}"
    res = make_request("GET", endpoint)
    print(json.dumps(res, indent=2, ensure_ascii=False))

def cmd_groups(_args):
    res = make_request("GET", "/api/v1/groups")
    print(json.dumps(res, indent=2, ensure_ascii=False))

def cmd_group_info(args):
    jid = args.jid.strip()
    endpoint = f"/api/v1/groups/{urllib.parse.quote(jid)}"
    res = make_request("GET", endpoint)
    print(json.dumps(res, indent=2, ensure_ascii=False))

def cmd_export_backup(args):
    jid = args.jid.strip()
    endpoint = f"/api/v1/groups/{urllib.parse.quote(jid)}/backup"
    payload = {
        "limit": args.limit,
        "include_media": args.include_media,
        "format": "json"
    }
    res = make_request("POST", endpoint, payload)
    
    # If backup created and output path specified, attempt to save
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

    print(json.dumps(res, indent=2, ensure_ascii=False))

def cmd_send_text(args):
    recipient = args.to.strip()
    text = args.text
    payload = {
        "recipient": recipient,
        "content": text,
        "to": recipient,
        "message": text
    }
    res = make_request("POST", "/api/v1/messages/send-text", payload)
    if res.get("error") and res.get("status_code") == 404:
        # Fallback to legacy endpoint /send/message
        res = make_request("POST", "/send/message", payload)
    print(json.dumps(res, indent=2, ensure_ascii=False))

def cmd_reaction(args):
    recipient = args.to.strip()
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
    print(json.dumps(res, indent=2, ensure_ascii=False))

def cmd_search(args):
    jid = args.jid.strip()
    limit = args.limit or 100
    query = (args.query or "").lower().strip()
    sender = (args.sender or "").strip()
    has_media = args.has_media

    endpoint = f"/api/v1/chats/{urllib.parse.quote(jid)}/messages?limit={limit}"
    raw = make_request("GET", endpoint)

    if raw.get("error") or not isinstance(raw.get("data"), list):
        print(json.dumps(raw, indent=2, ensure_ascii=False))
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
        "chat_jid": jid,
        "query": query,
        "total_scanned": len(messages),
        "total_matched": len(filtered),
        "matches": filtered
    }
    print(json.dumps(result, indent=2, ensure_ascii=False))

def cmd_stats(_args):
    session = make_request("GET", "/api/v1/session/status")
    antiban = make_request("GET", "/api/v1/antiban/stats")
    res = {
        "session": session.get("data") if session.get("success") else session,
        "antiban": antiban.get("data") if antiban.get("success") else antiban
    }
    print(json.dumps(res, indent=2, ensure_ascii=False))

def cmd_download_media(args):
    base_url, api_key = get_config()
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
    print(json.dumps(res, indent=2, ensure_ascii=False))

def cmd_send_media(args):
    base_url, api_key = get_config(args)
    endpoint = f"{base_url}/api/v1/messages/send-media"
    file_path = os.path.abspath(args.file)
    
    if not os.path.exists(file_path):
        print(json.dumps({"error": True, "message": f"File not found: {file_path}"}))
        sys.exit(1)

    # Auto-detect media type if set to auto
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

    # Use curl for robust multipart form-data handling without extra python dependencies
    curl_cmd = [
        "curl", "-s", "-X", "POST", endpoint,
        "-H", f"X-API-Key: {api_key}",
        "-H", f"Authorization: Bearer {api_key}",
        "-F", f"recipient={args.to}",
        "-F", f"type={media_type}",
        "-F", f"file=@{file_path}"
    ]
    if args.caption:
        curl_cmd.extend(["-F", f"caption={args.caption}"])
    if getattr(args, "reply_to", None):
        curl_cmd.extend(["-F", f"reply_to_id={args.reply_to}"])

    try:
        proc = subprocess.run(curl_cmd, capture_output=True, text=True, timeout=60)
        if proc.returncode == 0:
            try:
                out_json = json.loads(proc.stdout)
                print(json.dumps(out_json, indent=2, ensure_ascii=False))
            except json.JSONDecodeError:
                print(json.dumps({"status": "ok", "raw_output": proc.stdout}))
        else:
            print(json.dumps({"error": True, "message": proc.stderr or proc.stdout}))
    except Exception as e:
        print(json.dumps({"error": True, "message": str(e)}))

def cmd_profile_picture_get(args):
    params = []
    if args.jid:
        params.append(f"jid={urllib.parse.quote(args.jid)}")
    if args.preview:
        params.append("preview=true")
    qs = f"?{'&'.join(params)}" if params else ""
    res = make_request("GET", f"/api/v1/user/profile-picture{qs}")
    print(json.dumps(res, indent=2, ensure_ascii=False))

def cmd_profile_picture_set(args):
    base_url, api_key = get_config(args)
    endpoint = f"{base_url}/api/v1/user/profile-picture"
    file_path = os.path.abspath(args.file)
    if not os.path.exists(file_path):
        print(json.dumps({"error": True, "message": f"File not found: {file_path}"}))
        sys.exit(1)

    curl_cmd = [
        "curl", "-s", "-X", "POST", endpoint,
        "-H", f"X-API-Key: {api_key}",
        "-H", f"Authorization: Bearer {api_key}",
        "-F", f"file=@{file_path}"
    ]
    if args.jid:
        curl_cmd.extend(["-F", f"jid={args.jid}"])

    try:
        proc = subprocess.run(curl_cmd, capture_output=True, text=True, timeout=60)
        if proc.returncode == 0:
            try:
                out_json = json.loads(proc.stdout)
                print(json.dumps(out_json, indent=2, ensure_ascii=False))
            except json.JSONDecodeError:
                print(json.dumps({"status": "ok", "raw_output": proc.stdout}))
        else:
            print(json.dumps({"error": True, "message": proc.stderr or proc.stdout}))
    except Exception as e:
        print(json.dumps({"error": True, "message": str(e)}))

def cmd_profile_picture_remove(args):
    endpoint = "/api/v1/user/profile-picture"
    if args.jid:
        endpoint += f"?jid={urllib.parse.quote(args.jid)}"
    res = make_request("DELETE", endpoint)
    print(json.dumps(res, indent=2, ensure_ascii=False))

def cmd_about_set(args):
    res = make_request("POST", "/api/v1/user/about", {"status": args.status})
    print(json.dumps(res, indent=2, ensure_ascii=False))

# Import StatusSafetyGuard dari scripts/persona/safety jika tersedia
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
        # Fallback minimal jika modul safety tidak dapat dimuat
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
        print(json.dumps({
            "error": True,
            "safety_violation": True,
            "message": f"DITOLAK STATUS SAFETY GUARD: {reason}"
        }, indent=2, ensure_ascii=False))
        sys.exit(1)

    payload = {
        "type": "text",
        "text": clean_text
    }
    if args.background:
        payload["background_color"] = args.background
    if args.font is not None:
        payload["font"] = args.font
    res = make_request("POST", "/api/v1/status/send-story", payload)
    print(json.dumps(res, indent=2, ensure_ascii=False))

def cmd_status_send_media(args):
    base_url, api_key = get_config(args)
    endpoint = f"{base_url}/api/v1/status/send-story"
    file_path = os.path.abspath(args.file)

    # Validasi media dan caption melalui StatusSafetyGuard
    is_safe_media, media_err = validate_safe_status(file_path=file_path)
    if not is_safe_media:
        print(json.dumps({
            "error": True,
            "safety_violation": True,
            "message": f"DITOLAK STATUS SAFETY GUARD: {media_err}"
        }, indent=2, ensure_ascii=False))
        sys.exit(1)

    if args.caption:
        if StatusSafetyGuard:
            args.caption = StatusSafetyGuard.sanitize_caption(args.caption)
        is_safe_caption, cap_err = validate_safe_status(text=args.caption)
        if not is_safe_caption:
            print(json.dumps({
                "error": True,
                "safety_violation": True,
                "message": f"DITOLAK STATUS SAFETY GUARD (Caption tidak aman): {cap_err}"
            }, indent=2, ensure_ascii=False))
            sys.exit(1)

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
                print(json.dumps(out_json, indent=2, ensure_ascii=False))
            except json.JSONDecodeError:
                print(json.dumps({"status": "ok", "raw_output": proc.stdout}))
        else:
            print(json.dumps({"error": True, "message": proc.stderr or proc.stdout}))
    except Exception as e:
        print(json.dumps({"error": True, "message": str(e)}))

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
    print(json.dumps(res, indent=2, ensure_ascii=False))

def cmd_revoke(args):
    payload = {"message_id": args.id}
    if args.chat_jid:
        payload["chat_jid"] = args.chat_jid
    res = make_request("POST", "/api/v1/messages/revoke", payload)
    print(json.dumps(res, indent=2, ensure_ascii=False))

def main():
    global CURRENT_ARGS

    common_parser = argparse.ArgumentParser(add_help=False)
    common_parser.add_argument("--base-url", help="Override Whatsmeow Gateway Base URL")
    common_parser.add_argument("--api-key", help="Override Whatsmeow API Key")
    common_parser.add_argument("--companion", action="store_true", help="Route request to the Companion WhatsApp gateway (e.g. personal account)")

    parser = argparse.ArgumentParser(description="Whatsmeow CLI helper tool for Aina agent", parents=[common_parser])
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

    # export-backup
    p_backup = subparsers.add_parser("export-backup", help="Export chat backup of a group", parents=[common_parser])
    p_backup.add_argument("--jid", required=True, help="Group JID")
    p_backup.add_argument("--limit", type=int, default=1000, help="Max messages to backup")
    p_backup.add_argument("--include-media", action="store_true", help="Include media metadata")
    p_backup.add_argument("--out", help="Optional output filepath to save JSON backup")
    p_backup.set_defaults(func=cmd_export_backup)

    # send-text
    p_send_text = subparsers.add_parser("send-text", help="Send a WhatsApp text message", parents=[common_parser])
    p_send_text.add_argument("--to", required=True, help="Recipient JID (e.g. 628xxx@s.whatsapp.net or 120363xxx@g.us)")
    p_send_text.add_argument("--text", required=True, help="Message text content")
    p_send_text.set_defaults(func=cmd_send_text)

    # reaction
    p_reaction = subparsers.add_parser("reaction", help="Send an emoji reaction to a WhatsApp message", parents=[common_parser])
    p_reaction.add_argument("--to", required=True, help="Recipient or chat JID (e.g. 628xxx@s.whatsapp.net)")
    p_reaction.add_argument("--id", required=True, help="Message ID to react to")
    p_reaction.add_argument("--emoji", default="🙏", help="Emoji reaction (e.g. 🙏, 👍, ❤️)")
    p_reaction.set_defaults(func=cmd_reaction)

    # send-media
    p_media = subparsers.add_parser("send-media", help="Send media or document file", parents=[common_parser])
    p_media.add_argument("--to", required=True, help="Recipient JID")
    p_media.add_argument("--file", required=True, help="Path to file on disk")
    p_media.add_argument("--type", default="auto", choices=["auto", "image", "video", "audio", "document"], help="Media type (default: auto)")
    p_media.add_argument("--caption", help="Optional caption text")
    p_media.add_argument("--reply-to", help="Optional message ID being quoted / replied to")
    p_media.set_defaults(func=cmd_send_media)

    # search
    p_search = subparsers.add_parser("search", help="Search & filter messages in a chat", parents=[common_parser])
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
    p_st_text.add_argument("--background", help="Optional ARGB background color (hex, e.g. 0xFF5733 or #FF5733)")
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
