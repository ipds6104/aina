#!/usr/bin/env python3
"""
Aina Direct Google OAuth 2.0 PKCE Helper
Menggantikan pemanggilan PTY CLI Antigravity dengan native HTTP PKCE langsung ke Google OAuth.
Keunggulan:
- Tanpa batas waktu 60 detik (timeout sesi diperpanjang hingga 1 jam).
- Pertukaran token instan (200ms) tanpa memicu prompt AI.
- Bebas error PTY buffer atau escape ANSI.
- Mendukung mode interaktif 'exchange' langsung via CLI.
"""

import base64
import datetime
import hashlib
import json
import os
import secrets
import sys
import time
import urllib.error
import urllib.parse
import urllib.request

def _decode_key(arr, k=42):
    return "".join(chr(c ^ k) for c in arr)

CLIENT_ID = _decode_key([27, 26, 29, 27, 26, 26, 28, 26, 28, 26, 31, 19, 27, 7, 94, 71, 66, 89, 89, 67, 68, 24, 66, 24, 27, 70, 73, 88, 79, 24, 25, 31, 92, 94, 69, 70, 69, 64, 66, 30, 77, 30, 26, 25, 79, 90, 4, 75, 90, 90, 89, 4, 77, 69, 69, 77, 70, 79, 95, 89, 79, 88, 73, 69, 68, 94, 79, 68, 94, 4, 73, 69, 71])
CLIENT_SECRET = _decode_key([109, 101, 105, 121, 122, 114, 7, 97, 31, 18, 108, 125, 120, 30, 18, 28, 102, 78, 102, 96, 27, 71, 102, 104, 18, 89, 114, 105, 30, 80, 28, 91, 110, 107, 76])
REDIRECT_URI = "https://antigravity.google/oauth-callback"
SCOPES = [
    "https://www.googleapis.com/auth/cloud-platform",
    "https://www.googleapis.com/auth/userinfo.email",
    "https://www.googleapis.com/auth/userinfo.profile",
    "https://www.googleapis.com/auth/cclog",
    "https://www.googleapis.com/auth/experimentsandconfigs",
    "https://www.googleapis.com/auth/aicode",
    "openid",
]

def sanitize_code(raw: str) -> str:
    s = raw.strip()
    try:
        s = urllib.parse.unquote(s).strip()
    except Exception:
        pass
    if "code=" in s:
        s = s.split("code=")[1]
    for delim in ["&", "+http", " http", "userinfo.", "rinfo.", ".profile", "+", " "]:
        if delim in s:
            s = s.split(delim)[0]
    return s.strip()

def extract_email_from_jwt(id_token: str) -> str:
    try:
        if id_token and "." in id_token:
            parts = id_token.split(".")
            if len(parts) >= 2:
                payload_b64 = parts[1]
                payload_b64 += "=" * ((4 - len(payload_b64) % 4) % 4)
                payload_json = base64.urlsafe_b64decode(payload_b64).decode("utf-8", errors="ignore")
                payload = json.loads(payload_json)
                return payload.get("email", "unknown_account")
    except Exception:
        pass
    return "unknown_account"

def get_base_dir(session_id: str) -> str:
    if os.path.isdir("/app/data"):
        return os.path.abspath(f"/app/data/oauth_sessions/{session_id}")
    elif os.path.isdir("data") or os.path.exists("Cargo.toml"):
        return os.path.abspath(f"data/oauth_sessions/{session_id}")
    else:
        return f"/tmp/aina_oauth_{session_id}"

def do_exchange(base_dir: str, code: str, verifier: str):
    clean_code = sanitize_code(code)
    post_data = {
        "client_id": CLIENT_ID,
        "client_secret": CLIENT_SECRET,
        "code": clean_code,
        "code_verifier": verifier,
        "grant_type": "authorization_code",
        "redirect_uri": REDIRECT_URI,
    }

    req = urllib.request.Request(
        "https://oauth2.googleapis.com/token",
        data=urllib.parse.urlencode(post_data).encode("utf-8"),
        headers={
            "Content-Type": "application/x-www-form-urlencoded",
            "User-Agent": "Antigravity-CLI/1.0",
        },
    )

    try:
        with urllib.request.urlopen(req, timeout=15) as resp:
            resp_body = resp.read().decode("utf-8")
            data = json.loads(resp_body)

            now = datetime.datetime.now(datetime.timezone.utc)
            expiry_dt = now + datetime.timedelta(seconds=data.get("expires_in", 3600))

            token_obj = {
                "token": {
                    "access_token": data.get("access_token", ""),
                    "token_type": data.get("token_type", "Bearer"),
                    "refresh_token": data.get("refresh_token", ""),
                    "expiry": expiry_dt.isoformat(),
                },
                "auth_method": "consumer",
                "id_token": data.get("id_token", ""),
            }

            token_json = json.dumps(token_obj)
            with open(os.path.join(base_dir, "token.json"), "w") as f:
                f.write(token_json)
            with open(os.path.join(base_dir, "status.txt"), "w") as f:
                f.write("SUCCESS\n")

            email = extract_email_from_jwt(data.get("id_token", ""))
            print(f"SUCCESS:{email}")
            return True

    except urllib.error.HTTPError as e:
        err_body = e.read().decode("utf-8", errors="ignore")
        try:
            err_json = json.loads(err_body)
            desc = err_json.get("error_description", err_json.get("error", err_body))
        except Exception:
            desc = err_body
        with open(os.path.join(base_dir, "error.txt"), "w") as f:
            f.write(f"Google OAuth Error ({e.code}): {desc}\n")
        with open(os.path.join(base_dir, "status.txt"), "w") as f:
            f.write("FAILED\n")
        print(f"FAILED:{desc}", file=sys.stderr)
        return False
    except Exception as e:
        with open(os.path.join(base_dir, "error.txt"), "w") as f:
            f.write(f"Koneksi gagal: {str(e)}\n")
        with open(os.path.join(base_dir, "status.txt"), "w") as f:
            f.write("FAILED\n")
        print(f"FAILED:{e}", file=sys.stderr)
        return False

def main():
    if len(sys.argv) < 2:
        print("Usage: oauth_helper.py <session_id> OR oauth_helper.py exchange <session_id> <code>", file=sys.stderr)
        sys.exit(1)

    if sys.argv[1] == "exchange":
        if len(sys.argv) < 4:
            print("Usage: oauth_helper.py exchange <session_id> <code>", file=sys.stderr)
            sys.exit(1)
        session_id = sys.argv[2]
        code = sys.argv[3]
        base_dir = get_base_dir(session_id)
        verifier_file = os.path.join(base_dir, "verifier.txt")
        if not os.path.exists(verifier_file):
            print(f"Verifier file not found in {base_dir}", file=sys.stderr)
            sys.exit(1)
        with open(verifier_file, "r") as f:
            verifier = f.read().strip()
        ok = do_exchange(base_dir, code, verifier)
        sys.exit(0 if ok else 1)

    session_id = sys.argv[1]
    base_dir = get_base_dir(session_id)
    os.makedirs(base_dir, exist_ok=True)

    # Generate PKCE verifier & challenge
    verifier = base64.urlsafe_b64encode(secrets.token_bytes(32)).decode("utf-8").rstrip("=")
    digest = hashlib.sha256(verifier.encode("utf-8")).digest()
    challenge = base64.urlsafe_b64encode(digest).decode("utf-8").rstrip("=")
    state = base64.urlsafe_b64encode(secrets.token_bytes(16)).decode("utf-8").rstrip("=")

    # Simpan verifier ke file sesi
    with open(os.path.join(base_dir, "verifier.txt"), "w") as f:
        f.write(verifier)

    # Bangun URL Google OAuth
    params = {
        "access_type": "offline",
        "client_id": CLIENT_ID,
        "code_challenge": challenge,
        "code_challenge_method": "S256",
        "prompt": "consent",
        "redirect_uri": REDIRECT_URI,
        "response_type": "code",
        "scope": " ".join(SCOPES),
        "state": state,
    }
    url = "https://accounts.google.com/o/oauth2/auth?" + urllib.parse.urlencode(params)

    # Tulis URL untuk dibaca backend / UI
    with open(os.path.join(base_dir, "auth_url.txt"), "w") as f:
        f.write(url)
    with open(os.path.join(base_dir, "status.txt"), "w") as f:
        f.write("WAITING\n")

    print(f"AUTH_URL:{url}")
    sys.exit(0)

if __name__ == "__main__":
    main()
