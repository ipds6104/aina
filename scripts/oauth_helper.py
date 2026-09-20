#!/usr/bin/env python3
import os
import pty
import shutil
import subprocess
import sys
import time

def main():
    if len(sys.argv) < 2:
        print("Usage: oauth_helper.py <session_id>", file=sys.stderr)
        sys.exit(1)

    session_id = sys.argv[1]
    base_dir = f"/tmp/aina_oauth_{session_id}"
    os.makedirs(base_dir, exist_ok=True)

    agy_binary = os.environ.get("AGY_BINARY_PATH", "/root/.local/bin/agy")
    if not os.path.exists(agy_binary):
        agy_binary = "/usr/local/bin/agy"

    master, slave = pty.openpty()
    env = os.environ.copy()
    env["HOME"] = base_dir

    proc = subprocess.Popen(
        [agy_binary, "-p", "hi"],
        env=env,
        stdin=slave,
        stdout=slave,
        stderr=slave,
        close_fds=True,
    )
    os.close(slave)

    # 1. Read until prompt or auth URL is printed
    buf = b""
    start = time.time()
    url = None
    while time.time() - start < 15:
        try:
            chunk = os.read(master, 1024)
            if not chunk:
                break
            buf += chunk
            if b"press Enter:" in buf or b"accounts.google.com" in buf:
                for line in buf.decode("utf-8", errors="ignore").splitlines():
                    if "https://accounts.google.com" in line:
                        url = line.strip()
                        break
                if url:
                    break
        except Exception:
            break

    if not url:
        with open(os.path.join(base_dir, "status.txt"), "w") as f:
            f.write("FAILED")
        with open(os.path.join(base_dir, "error.txt"), "w") as f:
            f.write("Gagal mendapatkan URL otorisasi Google dari CLI Antigravity.")
        os.close(master)
        proc.terminate()
        sys.exit(1)

    # Write auth URL for backend to read
    with open(os.path.join(base_dir, "auth_url.txt"), "w") as f:
        f.write(url)

    # 2. Wait for code.txt to be written by backend (timeout 300 seconds)
    code_file = os.path.join(base_dir, "code.txt")
    code_start = time.time()
    code = None
    while time.time() - code_start < 300:
        if os.path.exists(code_file):
            try:
                with open(code_file, "r") as f:
                    code = f.read().strip()
                if code:
                    break
            except Exception:
                pass
        time.sleep(0.1)

    if not code:
        with open(os.path.join(base_dir, "status.txt"), "w") as f:
            f.write("TIMEOUT")
        os.close(master)
        proc.terminate()
        sys.exit(1)

    # 3. Write authorization code to PTY master
    os.write(master, (code + "\n").encode("utf-8"))

    # 4. Wait for agy to complete token exchange
    rem = b""
    wait_start = time.time()
    while time.time() - wait_start < 25:
        try:
            chunk = os.read(master, 1024)
            if not chunk:
                break
            rem += chunk
        except Exception:
            break

    os.close(master)
    try:
        proc.wait(timeout=5)
    except Exception:
        proc.terminate()

    token_path = os.path.join(base_dir, ".gemini/antigravity-cli/antigravity-oauth-token")
    if os.path.exists(token_path):
        with open(token_path, "r") as f:
            tok_content = f.read().strip()
        with open(os.path.join(base_dir, "token.json"), "w") as f:
            f.write(tok_content)
        with open(os.path.join(base_dir, "status.txt"), "w") as f:
            f.write("SUCCESS")
    else:
        err_msg = rem.decode("utf-8", errors="ignore").strip()
        # Clean up any ANSI escape codes
        import re
        ansi_clean = re.sub(r'\x1b\[[0-9;]*[mGKH]', '', err_msg)
        with open(os.path.join(base_dir, "error.txt"), "w") as f:
            f.write(ansi_clean if ansi_clean else "Verifikasi kode otorisasi gagal atau ditolak oleh Google.")
        with open(os.path.join(base_dir, "status.txt"), "w") as f:
            f.write("FAILED")

if __name__ == "__main__":
    main()
