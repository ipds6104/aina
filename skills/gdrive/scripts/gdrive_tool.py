#!/usr/bin/env python3
"""
gdrive_tool.py - Official Google Drive & Google Sheets CLI for Aina

Provides seamless Google Drive & Google Sheets operations for Aina:
- Permanent OAuth 2.0 flow (refresh token persistence, no 7-day expiry when published).
- Spreadsheet creation, reading, writing, and appending (Google Sheets API v4).
- File upload, download/export, and shareable link management (Google Drive API v3).
- Zero external heavy dependencies (uses standard library + requests).
"""

import argparse
import csv
import io
import json
import mimetypes
import os
import re
import sys
import time
import urllib.parse
from http.server import BaseHTTPRequestHandler, HTTPServer
import requests

DEFAULT_SCOPES = [
    "https://www.googleapis.com/auth/drive",
    "https://www.googleapis.com/auth/spreadsheets",
    "https://www.googleapis.com/auth/userinfo.email"
]

TOKEN_ENDPOINT = "https://oauth2.googleapis.com/token"
AUTH_ENDPOINT = "https://accounts.google.com/o/oauth2/v2/auth"
USERINFO_ENDPOINT = "https://www.googleapis.com/oauth2/v3/userinfo"
DRIVE_API_BASE = "https://www.googleapis.com/drive/v3"
DRIVE_UPLOAD_BASE = "https://www.googleapis.com/upload/drive/v3"
SHEETS_API_BASE = "https://sheets.googleapis.com/v4/spreadsheets"

EXPORT_MIMES = {
    "xlsx": "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
    "csv": "text/csv",
    "pdf": "application/pdf",
    "docx": "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
    "txt": "text/plain",
}


def resolve_path(provided_path, default_candidates):
    if provided_path:
        return os.path.abspath(provided_path)
    for c in default_candidates:
        if os.path.exists(c):
            return os.path.abspath(c)
    for c in default_candidates:
        parent = os.path.dirname(os.path.abspath(c))
        if os.path.exists(parent):
            return os.path.abspath(c)
    return os.path.abspath(default_candidates[0])


def get_credentials_path(cli_path=None):
    env_path = os.environ.get("GOOGLE_CLIENT_SECRETS_FILE")
    if cli_path:
        return resolve_path(cli_path, [])
    if env_path:
        return resolve_path(env_path, [])
    return resolve_path(None, [
        "/app/config/client_secrets.json",
        "config/client_secrets.json",
        "/root/projects/aina/config/client_secrets.json",
        "client_secrets.json"
    ])


def get_token_path(cli_path=None):
    env_path = os.environ.get("GOOGLE_TOKEN_FILE")
    if cli_path:
        return resolve_path(cli_path, [])
    if env_path:
        return resolve_path(env_path, [])
    return resolve_path(None, [
        "/app/data/google_token.json",
        "data/google_token.json",
        "/root/projects/aina/data/google_token.json",
        "google_token.json"
    ])


def load_client_secrets(secrets_file):
    if not os.path.exists(secrets_file):
        raise FileNotFoundError(
            f"File client secrets tidak ditemukan di: {secrets_file}\n"
            "Silakan unduh file client_secrets.json dari Google Cloud Console (OAuth 2.0 Client ID) "
            "dan simpan ke config/client_secrets.json atau set env GOOGLE_CLIENT_SECRETS_FILE."
        )
    with open(secrets_file, "r", encoding="utf-8") as f:
        data = json.load(f)
    if "installed" in data:
        cfg = data["installed"]
    elif "web" in data:
        cfg = data["web"]
    else:
        cfg = data
    client_id = cfg.get("client_id")
    client_secret = cfg.get("client_secret")
    if not client_id or not client_secret:
        raise ValueError(f"client_id atau client_secret tidak ditemukan dalam {secrets_file}")
    return client_id, client_secret


def load_token_data(token_file):
    if not os.path.exists(token_file):
        raise FileNotFoundError(
            f"File token tidak ditemukan di: {token_file}\n"
            "Silakan jalankan 'gdrive_tool auth' terlebih dahulu untuk otorisasi Google."
        )
    with open(token_file, "r", encoding="utf-8") as f:
        return json.load(f)


def save_token_data(token_file, data):
    os.makedirs(os.path.dirname(os.path.abspath(token_file)), exist_ok=True)
    with open(token_file, "w", encoding="utf-8") as f:
        json.dump(data, f, indent=2)


def get_valid_access_token(token_file=None, secrets_file=None):
    token_file = get_token_path(token_file)
    token_data = load_token_data(token_file)

    now = int(time.time())
    expires_at = token_data.get("expires_at", 0)
    access_token = token_data.get("access_token")
    refresh_token = token_data.get("refresh_token")

    if access_token and (expires_at - now > 60):
        return access_token

    if not refresh_token:
        raise ValueError(
            "Refresh token tidak ditemukan dalam file token. "
            "Silakan jalankan ulang 'gdrive_tool auth' untuk mendapatkan refresh token baru."
        )

    client_id = token_data.get("client_id")
    client_secret = token_data.get("client_secret")

    if not client_id or not client_secret:
        secrets_path = get_credentials_path(secrets_file)
        client_id, client_secret = load_client_secrets(secrets_path)

    payload = {
        "client_id": client_id,
        "client_secret": client_secret,
        "refresh_token": refresh_token,
        "grant_type": "refresh_token",
    }
    resp = requests.post(TOKEN_ENDPOINT, data=payload, timeout=30)
    if resp.status_code != 200:
        raise RuntimeError(
            f"Gagal memperbarui access token (HTTP {resp.status_code}): {resp.text}\n"
            "Periksa apakah kredensial OAuth masih aktif atau jalankan 'gdrive_tool auth' kembali."
        )

    new_token = resp.json()
    new_access_token = new_token.get("access_token")
    expires_in = new_token.get("expires_in", 3600)

    token_data["access_token"] = new_access_token
    token_data["expires_at"] = now + int(expires_in)
    token_data["client_id"] = client_id
    token_data["client_secret"] = client_secret
    save_token_data(token_file, token_data)

    return new_access_token


def extract_resource_id(input_str):
    if not input_str:
        return ""
    input_str = input_str.strip()
    m = re.search(r"/d/([a-zA-Z0-9-_]+)", input_str)
    if m:
        return m.group(1)
    m = re.search(r"id=([a-zA-Z0-9-_]+)", input_str)
    if m:
        return m.group(1)
    if "/" not in input_str and "?" not in input_str:
        return input_str
    return input_str


class OAuthCallbackHandler(BaseHTTPRequestHandler):
    auth_code = None

    def do_GET(self):
        parsed = urllib.parse.urlparse(self.path)
        params = urllib.parse.parse_qs(parsed.query)
        if "code" in params:
            OAuthCallbackHandler.auth_code = params["code"][0]
            self.send_response(200)
            self.send_header("Content-Type", "text/html; charset=utf-8")
            self.end_headers()
            html = """
            <!DOCTYPE html>
            <html>
            <head><title>Otorisasi Berhasil - Aina</title></head>
            <body style="font-family: sans-serif; text-align: center; padding-top: 50px; background: #f0fdf4;">
                <h1 style="color: #15803d;">🎉 Otorisasi Google Drive Berhasil!</h1>
                <p style="color: #374151; font-size: 16px;">Token telah berhasil diterima oleh Aina. Anda dapat menutup tab ini sekarang.</p>
            </body>
            </html>
            """
            self.wfile.write(html.encode("utf-8"))
        else:
            self.send_response(400)
            self.send_header("Content-Type", "text/html; charset=utf-8")
            self.end_headers()
            self.wfile.write(b"<h1>Parameter 'code' tidak ditemukan.</h1>")

    def log_message(self, format, *args):
        pass


def cmd_auth(args):
    secrets_file = get_credentials_path(args.client_secrets)
    token_file = get_token_path(args.token_file)
    client_id, client_secret = load_client_secrets(secrets_file)

    port = args.port or 8085
    redirect_uri = f"http://localhost:{port}"

    auth_params = {
        "client_id": client_id,
        "redirect_uri": redirect_uri,
        "response_type": "code",
        "scope": " ".join(DEFAULT_SCOPES),
        "access_type": "offline",
        "prompt": "consent",
    }
    auth_url = f"{AUTH_ENDPOINT}?{urllib.parse.urlencode(auth_params)}"

    print("=" * 70)
    print("🔐 INITIATING GOOGLE OAUTH 2.0 FOR AINA")
    print("=" * 70)
    print(f"Client Secrets: {secrets_file}")
    print(f"Token Target  : {token_file}")
    print(f"Redirect URI  : {redirect_uri}")
    print("\n👉 Silakan buka URL berikut di browser untuk login & memberikan izin:")
    print(f"\n{auth_url}\n")
    print("=" * 70)

    auth_code = args.code

    if not auth_code and not args.no_browser:
        print(f"Mendengarkan callback di {redirect_uri}...")
        print("Tip: Jika Anda berada di remote server / headless tanpa akses port lokal,")
        print("Anda dapat menekan Ctrl+C dan jalankan kembali dengan parameter: --code <AUTH_CODE>")
        print("Atau salin URL hasil redirect dan masukkan langsung di bawah ini.\n")

        server = None
        try:
            server = HTTPServer(("0.0.0.0", port), OAuthCallbackHandler)
            server.timeout = 180
            while not OAuthCallbackHandler.auth_code:
                server.handle_request()
                if OAuthCallbackHandler.auth_code:
                    break
            auth_code = OAuthCallbackHandler.auth_code
        except KeyboardInterrupt:
            print("\nCallback server dihentikan manual.")
        except Exception as e:
            print(f"Peringatan: Gagal menjalankan server callback ({e}). Beralih ke input manual.")
        finally:
            if server:
                server.server_close()

    if not auth_code:
        print("\nMasukkan Authorization Code atau paste URL redirect dari browser:")
        raw_input = input("Code / URL: ").strip()
        if "code=" in raw_input:
            parsed = urllib.parse.urlparse(raw_input)
            params = urllib.parse.parse_qs(parsed.query)
            if "code" in params:
                auth_code = params["code"][0]
            else:
                m = re.search(r"code=([^&]+)", raw_input)
                if m:
                    auth_code = urllib.parse.unquote(m.group(1))
        else:
            auth_code = raw_input

    if not auth_code:
        print("❌ Error: Authorization code tidak boleh kosong.")
        sys.exit(1)

    print("Menukar authorization code dengan token...")
    token_payload = {
        "client_id": client_id,
        "client_secret": client_secret,
        "code": auth_code,
        "grant_type": "authorization_code",
        "redirect_uri": redirect_uri,
    }
    resp = requests.post(TOKEN_ENDPOINT, data=token_payload, timeout=30)
    if resp.status_code != 200:
        print(f"❌ Gagal menukar token (HTTP {resp.status_code}): {resp.text}")
        sys.exit(1)

    token_resp = resp.json()
    access_token = token_resp.get("access_token")
    refresh_token = token_resp.get("refresh_token")
    expires_in = token_resp.get("expires_in", 3600)
    now = int(time.time())

    if not refresh_token:
        print("⚠️ Peringatan: Google tidak mengembalikan refresh token.")
        print("Hal ini terjadi jika izin telah diberikan sebelumnya tanpa parameter prompt=consent.")
        print("Pastikan untuk menghapus izin Aina di https://myaccount.google.com/permissions")
        print("atau pastikan prompt=consent diaktifkan.")

    token_data = {
        "client_id": client_id,
        "client_secret": client_secret,
        "access_token": access_token,
        "refresh_token": refresh_token,
        "expires_at": now + int(expires_in),
        "scopes": DEFAULT_SCOPES,
        "token_uri": TOKEN_ENDPOINT,
    }
    save_token_data(token_file, token_data)

    user_email = "Unknown"
    try:
        user_info_resp = requests.get(
            USERINFO_ENDPOINT,
            headers={"Authorization": f"Bearer {access_token}"},
            timeout=15,
        )
        if user_info_resp.status_code == 200:
            user_email = user_info_resp.json().get("email", "Unknown")
    except Exception:
        pass

    result = {
        "status": "authenticated",
        "user_email": user_email,
        "token_file": token_file,
        "has_refresh_token": bool(refresh_token),
        "expires_in_seconds": expires_in,
    }
    print("\n✅ OTORISASI BERHASIL!")
    print(json.dumps(result, indent=2))


def cmd_status(args):
    token_file = get_token_path(args.token_file)
    if not os.path.exists(token_file):
        print(json.dumps({"status": "unauthenticated", "error": f"Token file {token_file} does not exist"}))
        sys.exit(1)

    try:
        access_token = get_valid_access_token(token_file=args.token_file)
    except Exception as e:
        print(json.dumps({"status": "error", "message": str(e)}))
        sys.exit(1)

    user_email = "Unknown"
    try:
        ui_resp = requests.get(
            USERINFO_ENDPOINT,
            headers={"Authorization": f"Bearer {access_token}"},
            timeout=15
        )
        if ui_resp.status_code == 200:
            user_email = ui_resp.json().get("email", "Unknown")
    except Exception:
        pass

    quota_info = {}
    try:
        about_resp = requests.get(
            f"{DRIVE_API_BASE}/about?fields=user,storageQuota",
            headers={"Authorization": f"Bearer {access_token}"},
            timeout=15
        )
        if about_resp.status_code == 200:
            quota_info = about_resp.json().get("storageQuota", {})
    except Exception:
        pass

    token_data = load_token_data(token_file)
    res = {
        "status": "authenticated",
        "user_email": user_email,
        "token_file": token_file,
        "has_refresh_token": bool(token_data.get("refresh_token")),
        "expires_at": token_data.get("expires_at"),
        "storage_quota": quota_info
    }
    print(json.dumps(res, indent=2))


def set_drive_permissions(access_token, file_id, share_type="anyone", role="reader", email=None):
    if share_type == "none":
        return None
    url = f"{DRIVE_API_BASE}/files/{file_id}/permissions"
    body = {"role": role}
    if share_type == "anyone":
        body["type"] = "anyone"
    elif share_type == "user" and email:
        body["type"] = "user"
        body["emailAddress"] = email
    else:
        body["type"] = "anyone"

    resp = requests.post(
        url,
        headers={"Authorization": f"Bearer {access_token}", "Content-Type": "application/json"},
        json=body,
        timeout=20
    )
    if resp.status_code in (200, 201):
        return resp.json()
    return None


def cmd_sheets_create(args):
    access_token = get_valid_access_token(token_file=args.token_file)
    title = args.title or f"Sheet_{int(time.time())}"

    body = {
        "properties": {
            "title": title
        }
    }
    resp = requests.post(
        SHEETS_API_BASE,
        headers={"Authorization": f"Bearer {access_token}", "Content-Type": "application/json"},
        json=body,
        timeout=30
    )
    if resp.status_code != 200:
        print(f"❌ Error creating spreadsheet (HTTP {resp.status_code}): {resp.text}")
        sys.exit(1)

    sheet_data = resp.json()
    sheet_id = sheet_data.get("spreadsheetId")
    sheet_url = sheet_data.get("spreadsheetUrl") or f"https://docs.google.com/spreadsheets/d/{sheet_id}/edit?usp=sharing"

    rows = []
    if args.data_csv and os.path.exists(args.data_csv):
        with open(args.data_csv, "r", encoding="utf-8") as f:
            reader = csv.reader(f)
            rows = [r for r in reader]
    elif args.data_json:
        try:
            if os.path.exists(args.data_json):
                with open(args.data_json, "r", encoding="utf-8") as f:
                    rows = json.load(f)
            else:
                rows = json.loads(args.data_json)
        except Exception as e:
            print(f"⚠️ Failed to parse data-json: {e}")

    if rows:
        append_url = f"{SHEETS_API_BASE}/{sheet_id}/values/Sheet1!A1:append?valueInputOption=USER_ENTERED"
        requests.post(
            append_url,
            headers={"Authorization": f"Bearer {access_token}", "Content-Type": "application/json"},
            json={"values": rows},
            timeout=30
        )

    if args.folder_id:
        folder_id = extract_resource_id(args.folder_id)
        move_url = f"{DRIVE_API_BASE}/files/{sheet_id}?addParents={folder_id}&fields=id,parents"
        requests.patch(
            move_url,
            headers={"Authorization": f"Bearer {access_token}"},
            timeout=20
        )

    if args.share != "none":
        set_drive_permissions(
            access_token,
            sheet_id,
            share_type=args.share,
            role=args.role,
            email=args.share_email
        )

    output = {
        "status": "success",
        "id": sheet_id,
        "title": title,
        "url": sheet_url,
        "rows_populated": len(rows),
        "shared": args.share != "none"
    }
    print(json.dumps(output, indent=2))


def cmd_sheets_read(args):
    access_token = get_valid_access_token(token_file=args.token_file)
    sheet_id = extract_resource_id(args.id or args.url)
    if not sheet_id:
        print("❌ Error: Harap masukkan --id atau --url spreadsheet.")
        sys.exit(1)

    range_name = urllib.parse.quote(args.range or "Sheet1")
    url = f"{SHEETS_API_BASE}/{sheet_id}/values/{range_name}"
    resp = requests.get(
        url,
        headers={"Authorization": f"Bearer {access_token}"},
        timeout=30
    )
    if resp.status_code != 200:
        print(f"❌ Error reading spreadsheet (HTTP {resp.status_code}): {resp.text}")
        sys.exit(1)

    data = resp.json()
    values = data.get("values", [])

    if args.format == "table":
        if not values:
            print("(Tabel kosong)")
            return
        col_widths = {}
        for row in values:
            for i, cell in enumerate(row):
                col_widths[i] = max(col_widths.get(i, 0), len(str(cell)))
        for row in values:
            line = " | ".join(str(cell).ljust(col_widths.get(i, 10)) for i, cell in enumerate(row))
            print(f"| {line} |")
    elif args.format == "csv":
        out = io.StringIO()
        writer = csv.writer(out)
        for row in values:
            writer.writerow(row)
        print(out.getvalue(), end="")
    else:
        print(json.dumps({
            "status": "success",
            "range": data.get("range"),
            "values": values
        }, indent=2))


def cmd_sheets_append(args):
    access_token = get_valid_access_token(token_file=args.token_file)
    sheet_id = extract_resource_id(args.id or args.url)
    if not sheet_id:
        print("❌ Error: Harap masukkan --id atau --url spreadsheet.")
        sys.exit(1)

    rows = []
    if args.row:
        rows = [args.row.split(",")]
    elif args.data_csv and os.path.exists(args.data_csv):
        with open(args.data_csv, "r", encoding="utf-8") as f:
            reader = csv.reader(f)
            rows = [r for r in reader]
    elif args.data_json:
        try:
            if os.path.exists(args.data_json):
                with open(args.data_json, "r", encoding="utf-8") as f:
                    rows = json.load(f)
            else:
                rows = json.loads(args.data_json)
        except Exception as e:
            print(f"❌ Error parsing data-json: {e}")
            sys.exit(1)

    if not rows:
        print("❌ Error: Tidak ada baris yang diberikan untuk di-append (--row, --data-csv, atau --data-json).")
        sys.exit(1)

    range_name = urllib.parse.quote(args.range or "Sheet1")
    url = f"{SHEETS_API_BASE}/{sheet_id}/values/{range_name}:append?valueInputOption=USER_ENTERED&insertDataOption=INSERT_ROWS"
    resp = requests.post(
        url,
        headers={"Authorization": f"Bearer {access_token}", "Content-Type": "application/json"},
        json={"values": rows},
        timeout=30
    )
    if resp.status_code != 200:
        print(f"❌ Error appending to spreadsheet (HTTP {resp.status_code}): {resp.text}")
        sys.exit(1)

    res = resp.json()
    updates = res.get("updates", {})
    output = {
        "status": "success",
        "updatedRange": updates.get("updatedRange"),
        "updatedRows": updates.get("updatedRows"),
        "updatedCells": updates.get("updatedCells")
    }
    print(json.dumps(output, indent=2))


def cmd_drive_upload(args):
    access_token = get_valid_access_token(token_file=args.token_file)
    file_path = os.path.abspath(args.file)
    if not os.path.exists(file_path):
        print(f"❌ Error: File {file_path} tidak ditemukan.")
        sys.exit(1)

    file_name = args.name or os.path.basename(file_path)
    content_type, _ = mimetypes.guess_type(file_path)
    if not content_type:
        content_type = "application/octet-stream"

    metadata = {"name": file_name}
    if args.folder_id:
        metadata["parents"] = [extract_resource_id(args.folder_id)]

    if args.convert_to_sheets:
        metadata["mimeType"] = "application/vnd.google-apps.spreadsheet"

    boundary = "-------314159265358979323846"
    delimiter = f"\r\n--{boundary}\r\n"
    close_delim = f"\r\n--{boundary}--\r\n"

    with open(file_path, "rb") as f:
        file_bytes = f.read()

    multipart_body = (
        delimiter
        + 'Content-Type: application/json; charset=UTF-8\r\n\r\n'
        + json.dumps(metadata)
        + delimiter
        + f'Content-Type: {content_type}\r\n\r\n'
    ).encode("utf-8") + file_bytes + close_delim.encode("utf-8")

    upload_url = f"{DRIVE_UPLOAD_BASE}/files?uploadType=multipart&fields=id,name,mimeType,webViewLink,webContentLink"
    resp = requests.post(
        upload_url,
        headers={
            "Authorization": f"Bearer {access_token}",
            "Content-Type": f"multipart/related; boundary={boundary}",
        },
        data=multipart_body,
        timeout=120
    )
    if resp.status_code not in (200, 201):
        print(f"❌ Error uploading file (HTTP {resp.status_code}): {resp.text}")
        sys.exit(1)

    file_info = resp.json()
    file_id = file_info.get("id")

    if args.share != "none":
        set_drive_permissions(
            access_token,
            file_id,
            share_type=args.share,
            role=args.role,
            email=args.share_email
        )

    web_view_link = file_info.get("webViewLink")
    if not web_view_link:
        if file_info.get("mimeType") == "application/vnd.google-apps.spreadsheet":
            web_view_link = f"https://docs.google.com/spreadsheets/d/{file_id}/edit?usp=sharing"
        else:
            web_view_link = f"https://drive.google.com/file/d/{file_id}/view?usp=sharing"

    output = {
        "status": "success",
        "id": file_id,
        "name": file_info.get("name"),
        "mimeType": file_info.get("mimeType"),
        "web_view_link": web_view_link,
        "web_content_link": file_info.get("webContentLink"),
        "shared": args.share != "none"
    }
    print(json.dumps(output, indent=2))


def cmd_drive_download(args):
    access_token = get_valid_access_token(token_file=args.token_file)
    file_id = extract_resource_id(args.id or args.url)
    if not file_id:
        print("❌ Error: Harap masukkan --id atau --url file.")
        sys.exit(1)

    meta_url = f"{DRIVE_API_BASE}/files/{file_id}?fields=id,name,mimeType,size"
    meta_resp = requests.get(meta_url, headers={"Authorization": f"Bearer {access_token}"}, timeout=20)
    if meta_resp.status_code != 200:
        print(f"❌ Error fetching metadata (HTTP {meta_resp.status_code}): {meta_resp.text}")
        sys.exit(1)

    meta = meta_resp.json()
    mime = meta.get("mimeType", "")
    original_name = meta.get("name", "downloaded_file")

    out_path = args.out
    if not out_path:
        out_path = original_name

    out_path = os.path.abspath(out_path)
    os.makedirs(os.path.dirname(out_path), exist_ok=True)

    if mime.startswith("application/vnd.google-apps."):
        export_fmt = args.export_format or ("xlsx" if "spreadsheet" in mime else "pdf")
        target_mime = EXPORT_MIMES.get(export_fmt.lower(), "application/pdf")
        export_url = f"{DRIVE_API_BASE}/files/{file_id}/export?mimeType={urllib.parse.quote(target_mime)}"
        resp = requests.get(export_url, headers={"Authorization": f"Bearer {access_token}"}, stream=True, timeout=120)
    else:
        dl_url = f"{DRIVE_API_BASE}/files/{file_id}?alt=media"
        resp = requests.get(dl_url, headers={"Authorization": f"Bearer {access_token}"}, stream=True, timeout=120)

    if resp.status_code != 200:
        print(f"❌ Error downloading file (HTTP {resp.status_code}): {resp.text}")
        sys.exit(1)

    with open(out_path, "wb") as f:
        for chunk in resp.iter_content(chunk_size=65536):
            if chunk:
                f.write(chunk)

    output = {
        "status": "success",
        "id": file_id,
        "name": original_name,
        "downloaded_to": out_path,
        "size_bytes": os.path.getsize(out_path)
    }
    print(json.dumps(output, indent=2))


def cmd_drive_list(args):
    access_token = get_valid_access_token(token_file=args.token_file)
    query_parts = ["trashed = false"]
    if args.folder_id:
        folder_id = extract_resource_id(args.folder_id)
        query_parts.append(f"'{folder_id}' in parents")
    if args.query:
        query_parts.append(f"({args.query})")

    q = " and ".join(query_parts)
    limit = args.limit or 20
    url = f"{DRIVE_API_BASE}/files?q={urllib.parse.quote(q)}&pageSize={limit}&fields=files(id,name,mimeType,size,modifiedTime,webViewLink)"
    resp = requests.get(url, headers={"Authorization": f"Bearer {access_token}"}, timeout=30)
    if resp.status_code != 200:
        print(f"❌ Error listing files (HTTP {resp.status_code}): {resp.text}")
        sys.exit(1)

    files = resp.json().get("files", [])
    print(json.dumps({"status": "success", "count": len(files), "files": files}, indent=2))


def cmd_drive_share(args):
    access_token = get_valid_access_token(token_file=args.token_file)
    file_id = extract_resource_id(args.id or args.url)
    if not file_id:
        print("❌ Error: Harap masukkan --id atau --url file/sheet.")
        sys.exit(1)

    res = set_drive_permissions(
        access_token,
        file_id,
        share_type=args.share or "anyone",
        role=args.role or "reader",
        email=args.email
    )

    share_url = f"https://drive.google.com/file/d/{file_id}/view?usp=sharing"
    output = {
        "status": "success",
        "id": file_id,
        "share_type": args.share or "anyone",
        "role": args.role or "reader",
        "url": share_url,
        "details": res
    }
    print(json.dumps(output, indent=2))


def main():
    parser = argparse.ArgumentParser(
        description="Official Google Drive & Google Sheets CLI for Aina",
        formatter_class=argparse.RawTextHelpFormatter
    )
    subparsers = parser.add_subparsers(dest="subcommand", help="Subcommand yang ingin dijalankan")

    # auth
    p_auth = subparsers.add_parser("auth", help="Otorisasi OAuth 2.0 satu kali untuk mendapatkan token permanen")
    p_auth.add_argument("--client-secrets", help="Path ke file client_secrets.json dari Google Cloud")
    p_auth.add_argument("--token-file", help="Path penyimpanan google_token.json")
    p_auth.add_argument("--port", type=int, default=8085, help="Port untuk local callback server (default: 8085)")
    p_auth.add_argument("--code", help="Otorisasi manual via authorization code langsung")
    p_auth.add_argument("--no-browser", action="store_true", help="Nonaktifkan callback server (headless manual code input)")

    # status
    p_status = subparsers.add_parser("status", help="Cek status otorisasi, validitas token, dan akun Google")
    p_status.add_argument("--token-file", help="Path penyimpanan google_token.json")

    # sheets-create
    p_screate = subparsers.add_parser("sheets-create", help="Buat Google Spreadsheet baru")
    p_screate.add_argument("--title", required=True, help="Nama spreadsheet")
    p_screate.add_argument("--folder-id", help="Folder Google Drive tujuan")
    p_screate.add_argument("--data-csv", help="Path file CSV lokal untuk diisikan langsung")
    p_screate.add_argument("--data-json", help="Data JSON 2D array atau path file JSON")
    p_screate.add_argument("--share", choices=["anyone", "user", "none"], default="anyone", help="Mode sharing link (default: anyone)")
    p_screate.add_argument("--role", choices=["reader", "writer"], default="writer", help="Role sharing (default: writer)")
    p_screate.add_argument("--share-email", help="Email penerima jika --share user")
    p_screate.add_argument("--token-file", help="Path penyimpanan google_token.json")

    # sheets-read
    p_sread = subparsers.add_parser("sheets-read", help="Baca data dari Google Spreadsheet")
    p_sread.add_argument("--id", help="Spreadsheet ID")
    p_sread.add_argument("--url", help="URL Google Spreadsheet")
    p_sread.add_argument("--range", default="Sheet1", help="Range pembacaan (default: Sheet1)")
    p_sread.add_argument("--format", choices=["json", "table", "csv"], default="json", help="Format output")
    p_sread.add_argument("--token-file", help="Path penyimpanan google_token.json")

    # sheets-append
    p_sapp = subparsers.add_parser("sheets-append", help="Tambah baris baru ke Google Spreadsheet")
    p_sapp.add_argument("--id", help="Spreadsheet ID")
    p_sapp.add_argument("--url", help="URL Google Spreadsheet")
    p_sapp.add_argument("--range", default="Sheet1", help="Target sheet (default: Sheet1)")
    p_sapp.add_argument("--row", help="Data baris tunggal (comma-separated, misal: A,B,C)")
    p_sapp.add_argument("--data-csv", help="Path file CSV yang ingin di-append")
    p_sapp.add_argument("--data-json", help="JSON 2D array baris yang ingin di-append")
    p_sapp.add_argument("--token-file", help="Path penyimpanan google_token.json")

    # drive-upload
    p_dupload = subparsers.add_parser("drive-upload", help="Unggah file lokal ke Google Drive")
    p_dupload.add_argument("--file", required=True, help="Path file lokal yang akan diunggah")
    p_dupload.add_argument("--name", help="Nama file di Google Drive (default: nama file asli)")
    p_dupload.add_argument("--folder-id", help="Folder Google Drive tujuan")
    p_dupload.add_argument("--share", choices=["anyone", "user", "none"], default="anyone", help="Mode sharing link (default: anyone)")
    p_dupload.add_argument("--role", choices=["reader", "writer"], default="reader", help="Role sharing (default: reader)")
    p_dupload.add_argument("--share-email", help="Email penerima jika --share user")
    p_dupload.add_argument("--convert-to-sheets", action="store_true", help="Otomatis konversi CSV/XLSX ke Google Spreadsheet")
    p_dupload.add_argument("--token-file", help="Path penyimpanan google_token.json")

    # drive-download
    p_ddl = subparsers.add_parser("drive-download", help="Unduh file dari Google Drive")
    p_ddl.add_argument("--id", help="Google Drive File ID")
    p_ddl.add_argument("--url", help="Google Drive File URL")
    p_ddl.add_argument("--out", required=True, help="Path penyimpanan file lokal")
    p_ddl.add_argument("--export-format", choices=["xlsx", "csv", "pdf", "docx", "txt"], help="Format ekspor dokumen/spreadsheet")
    p_ddl.add_argument("--token-file", help="Path penyimpanan google_token.json")

    # drive-list
    p_dlist = subparsers.add_parser("drive-list", help="Cari dan daftar file di Google Drive")
    p_dlist.add_argument("--query", help="Query pencarian Drive API")
    p_dlist.add_argument("--folder-id", help="Filter berdasarkan folder ID")
    p_dlist.add_argument("--limit", type=int, default=20, help="Jumlah maksimal file (default: 20)")
    p_dlist.add_argument("--token-file", help="Path penyimpanan google_token.json")

    # drive-share
    p_dshare = subparsers.add_parser("drive-share", help="Ubah izin atau buat shareable link file/sheet")
    p_dshare.add_argument("--id", help="Google Drive File ID")
    p_dshare.add_argument("--url", help="Google Drive File URL")
    p_dshare.add_argument("--share", choices=["anyone", "user"], default="anyone", help="Tipe sharing")
    p_dshare.add_argument("--role", choices=["reader", "writer"], default="reader", help="Role akses")
    p_dshare.add_argument("--email", help="Email penerima jika --share user")
    p_dshare.add_argument("--token-file", help="Path penyimpanan google_token.json")

    args = parser.parse_args()
    if not args.subcommand:
        parser.print_help()
        sys.exit(0)

    dispatch = {
        "auth": cmd_auth,
        "status": cmd_status,
        "sheets-create": cmd_sheets_create,
        "sheets-read": cmd_sheets_read,
        "sheets-append": cmd_sheets_append,
        "drive-upload": cmd_drive_upload,
        "drive-download": cmd_drive_download,
        "drive-list": cmd_drive_list,
        "drive-share": cmd_drive_share,
    }

    cmd_fn = dispatch.get(args.subcommand)
    if cmd_fn:
        cmd_fn(args)
    else:
        parser.print_help()


if __name__ == "__main__":
    main()
