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

def get_config() -> tuple[str, str]:
    base_url = (
        os.getenv("WHATSMEOW_BASE_URL")
        or os.getenv("WHATSMEOW_URL")
        or DEFAULT_BASE_URL
    ).rstrip("/")
    api_key = (
        os.getenv("WHATSMEOW_API_KEY")
        or os.getenv("API_KEY")
        or ""
    )
    return base_url, api_key

def make_request(method: str, endpoint: str, data: Optional[Dict[str, Any]] = None) -> Dict[str, Any]:
    base_url, api_key = get_config()
    url = f"{base_url}{endpoint}"
    
    headers = {
        "Accept": "application/json",
        "User-Agent": "Aina-Agentic-Tool/1.0"
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
        with urllib.request.urlopen(req, timeout=30) as resp:
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

def cmd_send_media(args):
    base_url, api_key = get_config()
    endpoint = f"{base_url}/api/v1/messages/send-media"
    file_path = os.path.abspath(args.file)
    
    if not os.path.exists(file_path):
        print(json.dumps({"error": True, "message": f"File not found: {file_path}"}))
        sys.exit(1)

    # Use curl for robust multipart form-data handling without extra python dependencies
    curl_cmd = [
        "curl", "-s", "-X", "POST", endpoint,
        "-H", f"X-API-Key: {api_key}",
        "-H", f"Authorization: Bearer {api_key}",
        "-F", f"recipient={args.to}",
        "-F", f"type={args.type}",
        "-F", f"file=@{file_path}"
    ]
    if args.caption:
        curl_cmd.extend(["-F", f"caption={args.caption}"])

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

def main():
    parser = argparse.ArgumentParser(description="Whatsmeow CLI helper tool for Aina agent")
    subparsers = parser.add_subparsers(dest="subcommand", required=True)

    # recent
    p_recent = subparsers.add_parser("recent", help="Fetch recent messages from a chat")
    p_recent.add_argument("--jid", required=True, help="Chat or Group JID")
    p_recent.add_argument("--limit", type=int, default=20, help="Number of messages to retrieve (default: 20)")
    p_recent.set_defaults(func=cmd_recent)

    # groups
    p_groups = subparsers.add_parser("groups", help="List all joined groups")
    p_groups.set_defaults(func=cmd_groups)

    # group-info
    p_ginfo = subparsers.add_parser("group-info", help="Get metadata and participants of a group")
    p_ginfo.add_argument("--jid", required=True, help="Group JID (ends with @g.us)")
    p_ginfo.set_defaults(func=cmd_group_info)

    # export-backup
    p_backup = subparsers.add_parser("export-backup", help="Export chat backup of a group")
    p_backup.add_argument("--jid", required=True, help="Group JID")
    p_backup.add_argument("--limit", type=int, default=1000, help="Max messages to backup")
    p_backup.add_argument("--include-media", action="store_true", help="Include media metadata")
    p_backup.add_argument("--out", help="Optional output filepath to save JSON backup")
    p_backup.set_defaults(func=cmd_export_backup)

    # send-text
    p_send_text = subparsers.add_parser("send-text", help="Send a WhatsApp text message")
    p_send_text.add_argument("--to", required=True, help="Recipient JID (e.g. 628xxx@s.whatsapp.net or 120363xxx@g.us)")
    p_send_text.add_argument("--text", required=True, help="Message text content")
    p_send_text.set_defaults(func=cmd_send_text)

    # send-media
    p_media = subparsers.add_parser("send-media", help="Send media or document file")
    p_media.add_argument("--to", required=True, help="Recipient JID")
    p_media.add_argument("--file", required=True, help="Path to file on disk")
    p_media.add_argument("--type", default="document", choices=["image", "video", "audio", "document"], help="Media type")
    p_media.add_argument("--caption", help="Optional caption text")
    p_media.set_defaults(func=cmd_send_media)

    args = parser.parse_args()
    args.func(args)

if __name__ == "__main__":
    main()
