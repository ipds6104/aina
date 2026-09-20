#!/usr/bin/env python3
"""
Aina Multi-Account Google OAuth CLI Helper (Direct PKCE)
Memudahkan login interaktif ke 11 akun Google (OAuth 2.0 PKCE langsung ke Google)
dan mendaftarkannya ke Pool Akun Antigravity Aina tanpa batas waktu 60 detik.
"""

import base64
import datetime
import hashlib
import json
import os
import re
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

def run_single_oauth_session(account_idx=1, admin_key=None, server_url="https://aina.dvlpid.my.id"):
    # Generate PKCE verifier & challenge
    verifier = base64.urlsafe_b64encode(secrets.token_bytes(32)).decode("utf-8").rstrip("=")
    digest = hashlib.sha256(verifier.encode("utf-8")).digest()
    challenge = base64.urlsafe_b64encode(digest).decode("utf-8").rstrip("=")
    state = base64.urlsafe_b64encode(secrets.token_bytes(16)).decode("utf-8").rstrip("=")

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

    print("\n" + "=" * 70)
    print(f"👉 [AKUN #{account_idx}] BUKA LINK LOGIN GOOGLE INI DI BROWSER ANDA:")
    print(url)
    print("=" * 70)
    print("Langkah:")
    print(f" 1. Buka link di atas di Chrome/browser Anda.")
    print(f" 2. Pilih Akun Google #{account_idx} Anda, lalu klik 'Izinkan' (Allow).")
    print(" 3. Pada halaman Google yang terbuka, klik tombol 'Copy to Clipboard'.")
    print(" 4. Tempel (paste) kodenya di bawah ini (tidak ada batasan 60 detik!):\n")

    try:
        user_input = input("Tempel Kode Otorisasi Google (4/0A...): ").strip()
    except (KeyboardInterrupt, EOFError):
        print("\n\nOtorisasi dibatalkan.")
        return False

    if not user_input:
        print("❌ Kode otorisasi kosong.")
        return False

    clean_code = sanitize_code(user_input)
    print(f"⏳ Menghubungkan langsung ke Google OAuth untuk menukar token...")

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

            token_content = json.dumps(token_obj)
            email = extract_email_from_jwt(data.get("id_token", ""))
            print(f"\n🎉 BERHASIL! Token untuk akun Google '{email}' berhasil diperoleh!")

            # Simpan cadangan lokal
            os.makedirs("data/saved_tokens", exist_ok=True)
            safe_email = re.sub(r'[^a-zA-Z0-9_\-\.]', '_', email)
            save_file = f"data/saved_tokens/{safe_email}.json"
            with open(save_file, "w") as f:
                f.write(token_content)
            print(f"📁 Cadangan token tersimpan di: {save_file}")

            # Daftarkan ke server Aina
            if server_url and admin_key:
                print(f"🚀 Mendaftarkan ke Pool Akun Aina ({server_url})...")
                try:
                    req_url = f"{server_url.rstrip('/')}/api/auth/token?key={urllib.parse.quote(admin_key)}"
                    payload = json.dumps({"token": token_content, "setup_code": admin_key}).encode("utf-8")
                    reg_req = urllib.request.Request(
                        req_url,
                        data=payload,
                        headers={"Content-Type": "application/json"},
                    )
                    with urllib.request.urlopen(reg_req, timeout=10) as reg_resp:
                        res_body = json.loads(reg_resp.read().decode("utf-8"))
                        if res_body.get("success"):
                            print(f"✅ Akun '{email}' SUKSES dimasukkan ke Pool Akun Aina! (Total Akun Aktif: {res_body.get('total_accounts')})")
                        else:
                            print(f"⚠️ Respon server: {res_body.get('error', 'Gagal mendaftar')}")
                except Exception as e:
                    print(f"⚠️ Gagal mengirim otomatis ke server API: {e}")
                    print(f"   (Cadangan tetap aman di file '{save_file}')")

            # Update juga pool lokal jika ada
            local_pool = "data/token_pool.json"
            if os.path.exists(local_pool):
                try:
                    with open(local_pool, "r") as f:
                        pool_data = json.load(f)
                    pool_data[email] = {
                        "email": email,
                        "token_content": token_content,
                        "added_at": int(time.time()),
                        "cooldown_until": 0,
                    }
                    with open(local_pool, "w") as f:
                        json.dump(pool_data, f, indent=2)
                    print(f"💾 Pool lokal '{local_pool}' juga telah diperbarui.")
                except Exception:
                    pass

            return True

    except urllib.error.HTTPError as e:
        err_body = e.read().decode("utf-8", errors="ignore")
        try:
            err_json = json.loads(err_body)
            desc = err_json.get("error_description", err_json.get("error", err_body))
        except Exception:
            desc = err_body
        print(f"\n❌ Gagal menukar kode otorisasi dari Google ({e.code}): {desc}")
        return False
    except Exception as e:
        print(f"\n❌ Terjadi kesalahan jaringan: {e}")
        return False

def main():
    print("=" * 70)
    print("🌟 AINA GOOGLE MULTI-ACCOUNT DIRECT OAUTH (PKCE)")
    print("=" * 70)
    print("Alat ini menghubungkan langsung ke Google OAuth tanpa batas waktu 60 detik.")

    admin_key = os.environ.get("WHATSMEOW_API_KEY") or os.environ.get("ADMIN_KEY")
    if not admin_key:
        try:
            admin_key = input("\nMasukkan Admin Key / WHATSMEOW_API_KEY Aina Anda: ").strip()
        except (KeyboardInterrupt, EOFError):
            print("\nBatal.")
            sys.exit(0)

    server_url = os.environ.get("AINA_SERVER_URL", "https://aina.dvlpid.my.id")

    added_count = 0
    current_idx = 1
    while True:
        ok = run_single_oauth_session(account_idx=current_idx, admin_key=admin_key, server_url=server_url)
        if ok:
            added_count += 1
            current_idx += 1

        print("\n" + "-" * 50)
        try:
            choice = input(f"Lanjut menambahkan akun Google berikutnya ({added_count} akun tersimpan)? (y/n): ").strip().lower()
            if choice not in ["y", "yes", "ya"]:
                print(f"\n🎉 Selesai! Sebanyak {added_count} akun Google berhasil terhubung ke Aina.")
                print("Semua akun kini siap digunakan bergantian (Round-Robin) saat kuota habis! 🚀")
                break
        except (KeyboardInterrupt, EOFError):
            print("\nSelesai.")
            break

if __name__ == "__main__":
    main()
