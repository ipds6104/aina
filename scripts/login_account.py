#!/usr/bin/env python3
"""
Aina Multi-Account Google OAuth CLI Helper
Memudahkan login interaktif ke beberapa akun Google (OAuth 2.0 PKCE)
dan langsung mendaftarkannya ke Pool Akun Antigravity Aina.
"""

import fcntl
import json
import os
import pty
import re
import shutil
import subprocess
import sys
import time
import urllib.parse

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

def extract_email_from_token(token_str: str) -> str:
    try:
        data = json.loads(token_str)
        id_token = data.get("id_token", "")
        if id_token and "." in id_token:
            parts = id_token.split(".")
            if len(parts) >= 2:
                payload_b64 = parts[1]
                # Pad base64
                payload_b64 += "=" * ((4 - len(payload_b64) % 4) % 4)
                import base64
                payload_json = base64.urlsafe_b64decode(payload_b64).decode("utf-8", errors="ignore")
                payload = json.loads(payload_json)
                if "email" in payload:
                    return payload["email"]
    except Exception:
        pass
    return "unknown_account"

def run_single_oauth_session(admin_key=None, server_url="https://aina.dvlpid.my.id"):
    session_id = f"cli_{int(time.time())}_{os.urandom(4).hex()}"
    base_dir = f"/tmp/aina_cli_oauth_{session_id}"
    os.makedirs(base_dir, exist_ok=True)

    agy_binary = os.environ.get("AGY_BINARY_PATH", "/root/.local/bin/agy")
    if not os.path.exists(agy_binary):
        agy_binary = "/usr/local/bin/agy"
    if not os.path.exists(agy_binary):
        agy_binary = shutil.which("agy") or "agy"

    master, slave = pty.openpty()
    env = os.environ.copy()
    env["HOME"] = base_dir

    print("\n⏳ Memulai Google Antigravity CLI untuk membuat URL otorisasi...")
    proc = subprocess.Popen(
        [agy_binary, "-p", "hi"],
        env=env,
        stdin=slave,
        stdout=slave,
        stderr=slave,
        close_fds=True,
    )
    os.close(slave)

    # 1. Read URL from agy output
    buf = b""
    start = time.time()
    url = None
    while time.time() - start < 15:
        try:
            chunk = os.read(master, 1024)
            if not chunk:
                break
            buf += chunk
            if b"accounts.google.com" in buf:
                for line in buf.decode("utf-8", errors="ignore").splitlines():
                    if "https://accounts.google.com" in line:
                        url = line.strip()
                        break
                if url:
                    break
        except Exception:
            break

    if not url:
        print("❌ Gagal mendapatkan URL otorisasi dari CLI Antigravity.")
        proc.terminate()
        shutil.rmtree(base_dir, ignore_errors=True)
        return False

    print("\n" + "=" * 70)
    print("👉 BUKA LINK LOGIN GOOGLE INI DI BROWSER ANDA:")
    print(url)
    print("=" * 70)
    print("Langkah:")
    print(" 1. Buka link di atas pada tab browser baru.")
    print(" 2. Pilih salah satu dari akun Google Anda dan klik 'Izinkan' (Allow).")
    print(" 3. Pada halaman Google, klik tombol 'Copy to Clipboard'.")
    print(" 4. Tempel kodenya di bawah ini:\n")

    try:
        user_input = input("Tempel Kode Otorisasi Google (4/0A...): ").strip()
    except (KeyboardInterrupt, EOFError):
        print("\n\nOtorisasi dibatalkan.")
        proc.terminate()
        shutil.rmtree(base_dir, ignore_errors=True)
        return False

    if not user_input:
        print("❌ Kode otorisasi kosong.")
        proc.terminate()
        shutil.rmtree(base_dir, ignore_errors=True)
        return False

    clean_code = sanitize_code(user_input)
    print(f"Mengirim kode ke CLI (panjang: {len(clean_code)} karakter)...")
    os.write(master, (clean_code + "\n").encode("utf-8"))

    # 2. Polling for token file
    token_path = os.path.join(base_dir, ".gemini/antigravity-cli/antigravity-oauth-token")
    flags = fcntl.fcntl(master, fcntl.F_GETFL)
    fcntl.fcntl(master, fcntl.F_SETFL, flags | os.O_NONBLOCK)

    wait_start = time.time()
    success = False
    rem = b""

    while time.time() - wait_start < 40:
        if os.path.exists(token_path) and os.path.getsize(token_path) > 50:
            success = True
            break
        try:
            chunk = os.read(master, 1024)
            if chunk:
                rem += chunk
        except (BlockingIOError, OSError):
            pass

        if proc.poll() is not None:
            time.sleep(0.3)
            if os.path.exists(token_path) and os.path.getsize(token_path) > 50:
                success = True
            break
        time.sleep(0.1)

    try:
        os.close(master)
    except Exception:
        pass
    try:
        proc.terminate()
        proc.wait(timeout=2)
    except Exception:
        pass

    if success and os.path.exists(token_path):
        with open(token_path, "r") as f:
            token_content = f.read().strip()
        email = extract_email_from_token(token_content)
        print(f"\n🎉 BERHASIL! Token untuk akun Google '{email}' berhasil diperoleh!")

        # Save copy locally
        os.makedirs("data/saved_tokens", exist_ok=True)
        safe_email = re.sub(r'[^a-zA-Z0-9_\-\.]', '_', email)
        save_file = f"data/saved_tokens/{safe_email}.json"
        with open(save_file, "w") as f:
            f.write(token_content)
        print(f"📁 Cadangan token disimpan di: {save_file}")

        # Send to Aina server if admin_key available
        if server_url and admin_key:
            print(f"🚀 Mendaftarkan ke Pool Akun Aina ({server_url})...")
            try:
                import urllib.request
                req_url = f"{server_url.rstrip('/')}/api/auth/token?key={urllib.parse.quote(admin_key)}"
                payload = json.dumps({"token": token_content, "setup_code": admin_key}).encode("utf-8")
                req = urllib.request.Request(
                    req_url,
                    data=payload,
                    headers={"Content-Type": "application/json"}
                )
                with urllib.request.urlopen(req, timeout=10) as response:
                    res_body = json.loads(response.read().decode("utf-8"))
                    if res_body.get("success"):
                        print(f"✅ Akun '{email}' SUKSES dimasukkan ke Pool Akun Aina (Total: {res_body.get('total_accounts')} akun).")
                    else:
                        print(f"⚠️ Respon server: {res_body.get('error', 'Gagal mendaftar')}")
            except Exception as e:
                print(f"⚠️ Gagal mengirim ke server API: {e}")
                print(f"   (Jangan khawatir, Anda bisa menempelkan isi file '{save_file}' secara manual di dashboard)")

        shutil.rmtree(base_dir, ignore_errors=True)
        return True
    else:
        err = rem.decode("utf-8", errors="ignore").strip()
        import re
        ansi_clean = re.sub(r'\x1b\[[0-9;]*[mGKH]', '', err)
        print("\n❌ Gagal menukar kode otorisasi:")
        print(ansi_clean if ansi_clean else "Kode otorisasi tidak valid atau telah kadaluarsa.")
        shutil.rmtree(base_dir, ignore_errors=True)
        return False

def main():
    print("=" * 70)
    print("🌟 AINA GOOGLE MULTI-ACCOUNT OAUTH HELPER (CLI)")
    print("=" * 70)

    admin_key = os.environ.get("WHATSMEOW_API_KEY") or os.environ.get("ADMIN_KEY")
    if not admin_key:
        try:
            admin_key = input("Masukkan Admin Key / WHATSMEOW_API_KEY Aina Anda: ").strip()
        except (KeyboardInterrupt, EOFError):
            print("\nBatal.")
            sys.exit(0)

    server_url = os.environ.get("AINA_SERVER_URL", "https://aina.dvlpid.my.id")

    added_count = 0
    while True:
        print(f"\n--- [ Sesi Tambah Akun #{added_count + 1} ] ---")
        ok = run_single_oauth_session(admin_key=admin_key, server_url=server_url)
        if ok:
            added_count += 1

        print("\n" + "-" * 50)
        try:
            choice = input(f"Ingin menambahkan akun Google berikutnya ({added_count} berhasil)? (y/n): ").strip().lower()
            if choice not in ["y", "yes", "ya"]:
                print(f"\nSelesai! {added_count} akun telah diproses. Terima kasih! 🙏")
                break
        except (KeyboardInterrupt, EOFError):
            print("\nSelesai.")
            break

if __name__ == "__main__":
    main()
