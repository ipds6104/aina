#!/usr/bin/env python3
"""
Model Controller Script for Aina Agent
Memungkinkan Aina memeriksa atau mengubah model AI aktif secara mandiri di runtime.
"""

import sys
import os
import json
import urllib.request
import urllib.error

def get_base_url():
    port = os.environ.get("SERVER_PORT") or os.environ.get("PORT") or "8090"
    return f"http://127.0.0.1:{port}"

def get_admin_key():
    return os.environ.get("ADMIN_KEY") or os.environ.get("AINA_ADMIN_KEY") or ""

def make_request(path, method="GET", data=None):
    url = f"{get_base_url()}{path}"
    headers = {"Content-Type": "application/json"}
    admin_key = get_admin_key()
    if admin_key:
        headers["X-Admin-Key"] = admin_key
        headers["Authorization"] = f"Bearer {admin_key}"

    body = json.dumps(data).encode("utf-8") if data else None
    req = urllib.request.Request(url, data=body, headers=headers, method=method)

    try:
        with urllib.request.urlopen(req, timeout=10) as resp:
            content = resp.read().decode("utf-8")
            return json.loads(content)
    except urllib.error.HTTPError as e:
        error_body = e.read().decode("utf-8")
        try:
            err_json = json.loads(error_body)
            print(f"Error ({e.code}): {err_json.get('error') or err_json.get('message') or error_body}", file=sys.stderr)
        except Exception:
            print(f"Error ({e.code}): {error_body}", file=sys.stderr)
        sys.exit(1)
    except Exception as e:
        print(f"Connection error to {url}: {e}", file=sys.stderr)
        sys.exit(1)

def cmd_get():
    res = make_request("/api/models")
    print(f"Model aktif saat ini: {res.get('current')}")

def cmd_list():
    res = make_request("/api/models")
    print(f"Model Aktif: {res.get('current')}\n")
    print("Daftar Model yang Didukung:")
    for item in res.get("available", []):
        marker = " -> [AKTIF]" if item["id"] == res.get("current") else ""
        print(f"- {item['id']}: {item['name']}{marker}")

def cmd_set(model_name):
    payload = {
        "model": model_name,
        "admin_key": get_admin_key()
    }
    res = make_request("/api/model", method="POST", data=payload)
    print(f"Berhasil! Model aktif sekarang: {res.get('model')}")

def main():
    if len(sys.argv) < 2:
        print("Penggunaan: python3 scripts/model_control.py [get|list|set <model_name>]")
        sys.exit(1)

    action = sys.argv[1].lower()
    if action == "get":
        cmd_get()
    elif action == "list":
        cmd_list()
    elif action == "set":
        if len(sys.argv) < 3:
            print("Error: Harap sebutkan nama model. Contoh: python3 scripts/model_control.py set gemini-3.8-flash-high")
            sys.exit(1)
        cmd_set(sys.argv[2])
    else:
        print(f"Perintah tidak dikenal: '{action}'. Pilihan: get, list, set <model_name>")
        sys.exit(1)

if __name__ == "__main__":
    main()
