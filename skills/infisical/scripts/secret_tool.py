#!/usr/bin/env python3
"""
secret_tool.py - Official Infisical Secret & Vault Management CLI for Aina

Provides seamless, zero-knowledge secret management for Aina:
- Set, retrieve, list, and verify secrets in Infisical centralized vault.
- Execute tools and commands with secrets injected just-in-time into the process environment (infisical run).
- Automatic Universal Auth login caching.
- Masked outputs to prevent accidental secret leakage in chat and logs.
"""

import argparse
import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

DEFAULT_DOMAIN = "https://secrets.dvlpid.my.id/api"
DEFAULT_PROJECT_ID = "f13379e0-9661-4f8e-81ef-0e81d1502da1"
DEFAULT_ENV = "dev"
CACHE_FILE = Path("/root/.infisical/cached_token")


def get_infisical_bin() -> str:
    """Find the infisical binary."""
    # Check wrapper or system binary
    candidates = [
        "/usr/local/bin/infisical",
        "/usr/bin/infisical",
        shutil.which("infisical"),
    ]
    for cand in candidates:
        if cand and os.path.isfile(cand) and os.access(cand, os.X_OK):
            return cand
    return "infisical"


def mask_secret(value: str) -> str:
    """Mask secret value showing only first and last few characters."""
    if not value:
        return "<empty>"
    if len(value) <= 6:
        return "******"
    return f"{value[:3]}...{value[-3:]}"


def ensure_authenticated(domain: str, client_id: str, client_secret: str) -> bool:
    """Ensure we have a valid token or perform universal-auth login."""
    if os.environ.get("INFISICAL_TOKEN"):
        return True

    if CACHE_FILE.is_file() and CACHE_FILE.stat().st_size > 0:
        token = CACHE_FILE.read_text().strip()
        if token:
            os.environ["INFISICAL_TOKEN"] = token
            return True

    if client_id and client_secret:
        bin_path = get_infisical_bin()
        cmd = [
            bin_path,
            "login",
            "--domain",
            domain,
            "--method=universal-auth",
            f"--client-id={client_id}",
            f"--client-secret={client_secret}",
            "--plain",
            "--silent",
        ]
        try:
            res = subprocess.run(cmd, capture_output=True, text=True, check=True)
            token = res.stdout.strip()
            if token:
                os.environ["INFISICAL_TOKEN"] = token
                CACHE_FILE.parent.mkdir(parents=True, exist_ok=True)
                CACHE_FILE.write_text(token)
                CACHE_FILE.chmod(0o600)
                return True
        except Exception as e:
            sys.stderr.write(f"[WARN] Universal auth login attempt failed: {e}\n")
    return False


def cmd_set(args, domain: str, project_id: str, env: str):
    """Set a secret in Infisical."""
    bin_path = get_infisical_bin()
    key = args.key.strip().upper()
    val = args.value.strip()

    if not key or not val:
        print(json.dumps({"status": "error", "message": "Key and value must not be empty"}))
        sys.exit(1)

    cmd = [
        bin_path,
        "secrets",
        "set",
        f"{key}={val}",
        "--projectId",
        project_id,
        "--env",
        env,
        "--domain",
        domain,
        "--silent",
    ]
    try:
        res = subprocess.run(cmd, capture_output=True, text=True)
        if res.returncode == 0:
            out = {
                "status": "success",
                "message": f"Secret '{key}' successfully saved in Infisical vault.",
                "key": key,
                "masked_value": mask_secret(val),
                "environment": env,
                "project_id": project_id,
            }
            print(json.dumps(out, indent=2))
        else:
            err = res.stderr.strip() or res.stdout.strip()
            print(json.dumps({"status": "error", "message": err}))
            sys.exit(res.returncode)
    except Exception as e:
        print(json.dumps({"status": "error", "message": str(e)}))
        sys.exit(1)


def cmd_get(args, domain: str, project_id: str, env: str):
    """Retrieve a secret from Infisical."""
    bin_path = get_infisical_bin()
    key = args.key.strip().upper()

    cmd = [
        bin_path,
        "secrets",
        "get",
        key,
        "--projectId",
        project_id,
        "--env",
        env,
        "--domain",
        domain,
        "--plain",
        "--silent",
    ]
    try:
        res = subprocess.run(cmd, capture_output=True, text=True)
        if res.returncode == 0:
            val = res.stdout.strip()
            if args.reveal or args.plain:
                if args.plain:
                    print(val)
                else:
                    print(json.dumps({
                        "status": "success",
                        "key": key,
                        "value": val,
                        "environment": env
                    }, indent=2))
            else:
                print(json.dumps({
                    "status": "success",
                    "key": key,
                    "masked_value": mask_secret(val),
                    "environment": env
                }, indent=2))
        else:
            err = res.stderr.strip() or res.stdout.strip()
            print(json.dumps({"status": "error", "message": err, "key": key}))
            sys.exit(res.returncode)
    except Exception as e:
        print(json.dumps({"status": "error", "message": str(e)}))
        sys.exit(1)


def cmd_list(args, domain: str, project_id: str, env: str):
    """List secret names in Infisical."""
    bin_path = get_infisical_bin()
    cmd = [
        bin_path,
        "secrets",
        "--projectId",
        project_id,
        "--env",
        env,
        "--domain",
        domain,
        "-o",
        "json",
        "--silent",
    ]
    try:
        res = subprocess.run(cmd, capture_output=True, text=True)
        if res.returncode == 0:
            raw = res.stdout.strip()
            try:
                data = json.loads(raw)
                keys = [item.get("secretKey") or item.get("key") for item in data if isinstance(item, dict)]
                keys = [k for k in keys if k]
                print(json.dumps({
                    "status": "success",
                    "total": len(keys),
                    "secret_keys": keys,
                    "environment": env,
                    "project_id": project_id
                }, indent=2))
            except Exception:
                # If json format flag is not supported or returns table, extract names
                lines = [line.strip() for line in raw.splitlines() if line.strip() and "SECRET NAME" not in line]
                print(json.dumps({
                    "status": "success",
                    "raw_output": lines[:30],
                    "environment": env
                }, indent=2))
        else:
            err = res.stderr.strip() or res.stdout.strip()
            print(json.dumps({"status": "error", "message": err}))
            sys.exit(res.returncode)
    except Exception as e:
        print(json.dumps({"status": "error", "message": str(e)}))
        sys.exit(1)


def cmd_run(args, domain: str, project_id: str, env: str):
    """Execute command with secrets injected into environment."""
    bin_path = get_infisical_bin()
    command = args.command
    if not command:
        print("[ERROR] No command specified to run.")
        sys.exit(1)

    cmd = [
        bin_path,
        "run",
        "--projectId",
        project_id,
        "--env",
        env,
        "--domain",
        domain,
        "--",
    ] + command

    try:
        p = subprocess.run(cmd)
        sys.exit(p.returncode)
    except Exception as e:
        sys.stderr.write(f"[ERROR] Failed to run command via infisical: {e}\n")
        sys.exit(1)


def cmd_status(args, domain: str, project_id: str, env: str):
    """Check connectivity and credentials status."""
    bin_path = get_infisical_bin()
    has_token = bool(os.environ.get("INFISICAL_TOKEN") or (CACHE_FILE.is_file() and CACHE_FILE.stat().st_size > 0))
    info = {
        "status": "online" if has_token else "configured",
        "domain": domain,
        "project_id": project_id,
        "environment": env,
        "binary_path": bin_path,
        "has_cached_token": has_token,
    }
    print(json.dumps(info, indent=2))


def main():
    parser = argparse.ArgumentParser(
        description="Aina Infisical Secret Manager - Safe, zero-knowledge credential vault tool."
    )
    parser.add_argument("--domain", default=os.getenv("INFISICAL_DOMAIN", DEFAULT_DOMAIN), help="Infisical instance URL")
    parser.add_argument("--project-id", default=os.getenv("INFISICAL_PROJECT_ID", DEFAULT_PROJECT_ID), help="Infisical Project ID")
    parser.add_argument("--env", default=os.getenv("INFISICAL_ENV", DEFAULT_ENV), help="Infisical Environment (dev/prod)")

    subparsers = parser.add_subparsers(dest="subcommand", required=True)

    # Subcommand: set
    set_parser = subparsers.add_parser("set", help="Save a secret to Infisical vault")
    set_parser.add_argument("key", help="Secret variable name (e.g. TALLY_API_KEY)")
    set_parser.add_argument("value", help="Secret variable value")

    # Subcommand: get
    get_parser = subparsers.add_parser("get", help="Retrieve a secret from Infisical vault")
    get_parser.add_argument("key", help="Secret variable name (e.g. TALLY_API_KEY)")
    get_parser.add_argument("--reveal", action="store_true", help="Reveal unmasked value in JSON")
    get_parser.add_argument("--plain", action="store_true", help="Print raw unmasked value only")

    # Subcommand: list
    subparsers.add_parser("list", help="List secret keys stored in vault")

    # Subcommand: status
    subparsers.add_parser("status", help="Check Infisical vault connectivity")

    # Subcommand: run
    run_parser = subparsers.add_parser("run", help="Execute command with secrets injected into environment")
    run_parser.add_argument("command", nargs=argparse.REMAINDER, help="Command to execute (e.g. python3 script.py)")

    args = parser.parse_args()

    client_id = os.getenv("INFISICAL_CLIENT_ID", "d972328c-3c0c-4617-8c63-1cb1f7391f0f")
    client_secret = os.getenv("INFISICAL_CLIENT_SECRET", "1a73548b812589b6edf888ab64d8b0e52d615611feb7425ec75f5ea46017ae11")
    ensure_authenticated(args.domain, client_id, client_secret)

    if args.subcommand == "set":
        cmd_set(args, args.domain, args.project_id, args.env)
    elif args.subcommand == "get":
        cmd_get(args, args.domain, args.project_id, args.env)
    elif args.subcommand == "list":
        cmd_list(args, args.domain, args.project_id, args.env)
    elif args.subcommand == "status":
        cmd_status(args, args.domain, args.project_id, args.env)
    elif args.subcommand == "run":
        cmd_run(args, args.domain, args.project_id, args.env)


if __name__ == "__main__":
    main()
