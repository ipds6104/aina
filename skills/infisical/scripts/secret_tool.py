#!/usr/bin/env python3
"""
secret_tool.py - Universal Vendor-Agnostic Secret & Vault Management CLI for Aina

Provides seamless, zero-knowledge secret management across multiple vault backends:
1. Infisical (Universal Auth / Machine Identity)
2. HashiCorp Vault (KV v1/v2 via CLI or REST API)
3. Doppler (CLI / Service Token)
4. Local Persistent Env Provider (Zero-config fallback in /app/data/config/.secrets.env)

Features:
- Standardized CLI contract: get, set, list, run, status.
- Just-in-Time (JIT) process environment injection (zero secrets written to disk in cloud mode).
- Automatic provider detection based on environment variables.
- Masked outputs to prevent secret leaks in chat and logs.
"""

import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import urllib.error
import urllib.request
from abc import ABC, abstractmethod
from pathlib import Path

# Paths & Defaults
CACHE_FILE = Path("/root/.infisical/cached_token")
DEFAULT_INFISICAL_DOMAIN = "https://secrets.dvlpid.my.id/api"
DEFAULT_INFISICAL_PROJECT_ID = "f13379e0-9661-4f8e-81ef-0e81d1502da1"
DEFAULT_INFISICAL_ENV = "dev"


def get_vault_config_path() -> Path:
    candidates = [
        Path("/app/data/config/.vault_config.json"),
        Path("/app/data/.vault_config.json"),
        Path(__file__).resolve().parent.parent.parent.parent / "data" / "config" / ".vault_config.json",
        Path.home() / ".aina_vault_config.json",
    ]
    for c in candidates:
        if c.is_file() or c.parent.is_dir():
            return c
    return candidates[0]


def get_persisted_config() -> dict:
    cfg = get_vault_config_path()
    if cfg.is_file():
        try:
            return json.loads(cfg.read_text(encoding="utf-8"))
        except Exception:
            return {}
    return {}


def mask_secret(value: str) -> str:
    """Mask secret value showing only first and last few characters."""
    if not value:
        return "<empty>"
    if len(value) <= 6:
        return "******"
    return f"{value[:3]}...{value[-3:]}"


# =====================================================================
# Abstract Base Secret Provider
# =====================================================================

class BaseSecretProvider(ABC):
    @abstractmethod
    def name(self) -> str:
        pass

    @abstractmethod
    def set(self, key: str, value: str) -> dict:
        pass

    @abstractmethod
    def get(self, key: str) -> str:
        pass

    @abstractmethod
    def list(self) -> list:
        pass

    @abstractmethod
    def run(self, command: list) -> int:
        pass

    @abstractmethod
    def status(self) -> dict:
        pass


# =====================================================================
# 1. Infisical Provider
# =====================================================================

class InfisicalProvider(BaseSecretProvider):
    def __init__(self, domain: str = None, project_id: str = None, env: str = None):
        cfg = get_persisted_config()
        self.domain = domain or os.getenv("INFISICAL_DOMAIN") or cfg.get("domain") or DEFAULT_INFISICAL_DOMAIN
        self.project_id = project_id or os.getenv("INFISICAL_PROJECT_ID") or cfg.get("project_id") or DEFAULT_INFISICAL_PROJECT_ID
        self.env = env or os.getenv("INFISICAL_ENV") or cfg.get("env") or DEFAULT_INFISICAL_ENV
        self.client_id = os.getenv("INFISICAL_CLIENT_ID") or cfg.get("client_id") or "d972328c-3c0c-4617-8c63-1cb1f7391f0f"
        self.client_secret = os.getenv("INFISICAL_CLIENT_SECRET") or cfg.get("client_secret") or "1a73548b812589b6edf888ab64d8b0e52d615611feb7425ec75f5ea46017ae11"
        self.bin_path = self._find_bin()
        self._ensure_authenticated()

    def name(self) -> str:
        return "infisical"

    def _find_bin(self) -> str:
        candidates = [
            "/usr/local/bin/infisical",
            "/usr/bin/infisical",
            shutil.which("infisical"),
        ]
        for cand in candidates:
            if cand and os.path.isfile(cand) and os.access(cand, os.X_OK):
                return cand
        return "infisical"

    def _ensure_authenticated(self):
        if os.environ.get("INFISICAL_TOKEN"):
            return
        if CACHE_FILE.is_file() and CACHE_FILE.stat().st_size > 0:
            token = CACHE_FILE.read_text().strip()
            if token:
                os.environ["INFISICAL_TOKEN"] = token
                return

        if self.client_id and self.client_secret:
            cmd = [
                self.bin_path,
                "login",
                "--domain",
                self.domain,
                "--method=universal-auth",
                f"--client-id={self.client_id}",
                f"--client-secret={self.client_secret}",
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
            except Exception as e:
                sys.stderr.write(f"[WARN] Infisical universal auth login attempt failed: {e}\n")

    def set(self, key: str, value: str) -> dict:
        cmd = [
            self.bin_path,
            "secrets",
            "set",
            f"{key}={value}",
            "--projectId",
            self.project_id,
            "--env",
            self.env,
            "--domain",
            self.domain,
            "--silent",
        ]
        res = subprocess.run(cmd, capture_output=True, text=True)
        if res.returncode == 0:
            return {
                "status": "success",
                "provider": "infisical",
                "message": f"Secret '{key}' successfully saved in Infisical vault.",
                "key": key,
                "masked_value": mask_secret(value),
                "environment": self.env,
                "project_id": self.project_id,
            }
        else:
            raise RuntimeError(res.stderr.strip() or res.stdout.strip() or f"Exit {res.returncode}")

    def get(self, key: str) -> str:
        cmd = [
            self.bin_path,
            "secrets",
            "get",
            key,
            "--projectId",
            self.project_id,
            "--env",
            self.env,
            "--domain",
            self.domain,
            "--plain",
            "--silent",
        ]
        res = subprocess.run(cmd, capture_output=True, text=True)
        if res.returncode == 0:
            return res.stdout.strip()
        else:
            raise RuntimeError(res.stderr.strip() or res.stdout.strip() or f"Key '{key}' not found in Infisical")

    def list(self) -> list:
        cmd = [
            self.bin_path,
            "secrets",
            "--projectId",
            self.project_id,
            "--env",
            self.env,
            "--domain",
            self.domain,
            "-o",
            "json",
            "--silent",
        ]
        res = subprocess.run(cmd, capture_output=True, text=True)
        if res.returncode == 0:
            try:
                data = json.loads(res.stdout)
                keys = [item.get("secretKey") or item.get("key") for item in data if isinstance(item, dict)]
                return [k for k in keys if k]
            except Exception:
                lines = [line.strip() for line in res.stdout.splitlines() if line.strip() and "SECRET NAME" not in line]
                return lines
        else:
            raise RuntimeError(res.stderr.strip() or res.stdout.strip())

    def run(self, command: list) -> int:
        cmd = [
            self.bin_path,
            "run",
            "--projectId",
            self.project_id,
            "--env",
            self.env,
            "--domain",
            self.domain,
            "--",
        ] + command
        p = subprocess.run(cmd)
        return p.returncode

    def status(self) -> dict:
        has_token = bool(os.environ.get("INFISICAL_TOKEN") or (CACHE_FILE.is_file() and CACHE_FILE.stat().st_size > 0))
        return {
            "status": "online" if has_token else "configured",
            "provider": "infisical",
            "domain": self.domain,
            "project_id": self.project_id,
            "environment": self.env,
            "binary_path": self.bin_path,
            "has_cached_token": has_token,
        }


# =====================================================================
# 2. HashiCorp Vault Provider (KV v1/v2 via CLI or REST)
# =====================================================================

class HashiCorpVaultProvider(BaseSecretProvider):
    def __init__(self, addr: str = None, token: str = None, mount: str = None):
        cfg = get_persisted_config()
        self.addr = (addr or os.getenv("VAULT_ADDR") or cfg.get("vault_addr") or "http://127.0.0.1:8200").rstrip("/")
        self.token = token or os.getenv("VAULT_TOKEN") or cfg.get("vault_token") or ""
        self.mount = mount or os.getenv("VAULT_MOUNT") or cfg.get("vault_mount") or "secret"
        self.bin_path = shutil.which("vault")

    def name(self) -> str:
        return "vault"

    def _http_req(self, path: str, method: str = "GET", data: dict = None) -> dict:
        url = f"{self.addr}/v1/{self.mount}/data/{path.lstrip('/')}"
        req = urllib.request.Request(url, method=method)
        req.add_header("X-Vault-Token", self.token)
        req.add_header("Content-Type", "application/json")
        body = json.dumps(data).encode("utf-8") if data else None

        try:
            with urllib.request.urlopen(req, data=body, timeout=10) as resp:
                raw = resp.read().decode("utf-8")
                return json.loads(raw) if raw else {}
        except urllib.error.HTTPError as e:
            err_body = e.read().decode("utf-8", errors="ignore")
            raise RuntimeError(f"Vault HTTP error {e.code}: {err_body or e.reason}")
        except Exception as e:
            raise RuntimeError(f"Vault connection error: {e}")

    def set(self, key: str, value: str) -> dict:
        payload = {"data": {key: value}}
        self._http_req(key, method="POST", data=payload)
        return {
            "status": "success",
            "provider": "vault",
            "message": f"Secret '{key}' successfully saved in HashiCorp Vault.",
            "key": key,
            "masked_value": mask_secret(value),
            "vault_addr": self.addr,
            "mount": self.mount,
        }

    def get(self, key: str) -> str:
        res = self._http_req(key, method="GET")
        data = res.get("data", {}).get("data", {})
        if key in data:
            return str(data[key])
        if len(data) == 1:
            return str(list(data.values())[0])
        raise RuntimeError(f"Key '{key}' not found in Vault path {self.mount}/data/{key}")

    def list(self) -> list:
        url = f"{self.addr}/v1/{self.mount}/metadata?list=true"
        req = urllib.request.Request(url, method="GET")
        req.add_header("X-Vault-Token", self.token)
        try:
            with urllib.request.urlopen(req, timeout=10) as resp:
                raw = resp.read().decode("utf-8")
                res = json.loads(raw)
                return res.get("data", {}).get("keys", [])
        except Exception:
            return []

    def run(self, command: list) -> int:
        if self.bin_path:
            cmd = [self.bin_path, "run"] + command
            return subprocess.run(cmd).returncode
        else:
            # Fallback: fetch keys and inject into sub-process
            env = os.environ.copy()
            keys = self.list()
            for k in keys:
                try:
                    val = self.get(k)
                    env[k] = val
                except Exception:
                    pass
            p = subprocess.run(command, env=env)
            return p.returncode

    def status(self) -> dict:
        return {
            "status": "online" if self.token else "missing_token",
            "provider": "vault",
            "vault_addr": self.addr,
            "mount": self.mount,
            "has_cli": bool(self.bin_path),
        }


# =====================================================================
# 3. Doppler Provider
# =====================================================================

class DopplerProvider(BaseSecretProvider):
    def __init__(self, token: str = None, project: str = None, config: str = None):
        cfg = get_persisted_config()
        self.token = token or os.getenv("DOPPLER_TOKEN") or cfg.get("doppler_token") or ""
        self.project = project or os.getenv("DOPPLER_PROJECT") or cfg.get("doppler_project") or ""
        self.config = config or os.getenv("DOPPLER_CONFIG") or cfg.get("doppler_config") or "dev"
        self.bin_path = shutil.which("doppler")

    def name(self) -> str:
        return "doppler"

    def set(self, key: str, value: str) -> dict:
        if self.bin_path:
            cmd = [self.bin_path, "secrets", "set", f"{key}={value}", "--silent"]
            if self.project:
                cmd += ["-p", self.project]
            if self.config:
                cmd += ["-c", self.config]
            res = subprocess.run(cmd, capture_output=True, text=True)
            if res.returncode == 0:
                return {
                    "status": "success",
                    "provider": "doppler",
                    "message": f"Secret '{key}' successfully saved in Doppler.",
                    "key": key,
                    "masked_value": mask_secret(value),
                }
            raise RuntimeError(res.stderr.strip() or res.stdout.strip())
        raise RuntimeError("Doppler CLI ('doppler') is required to set secrets.")

    def get(self, key: str) -> str:
        if self.bin_path:
            cmd = [self.bin_path, "secrets", "get", key, "--plain"]
            if self.project:
                cmd += ["-p", self.project]
            if self.config:
                cmd += ["-c", self.config]
            res = subprocess.run(cmd, capture_output=True, text=True)
            if res.returncode == 0:
                return res.stdout.strip()
            raise RuntimeError(res.stderr.strip() or res.stdout.strip() or f"Key '{key}' not found in Doppler")
        raise RuntimeError("Doppler CLI ('doppler') is required to retrieve secrets.")

    def list(self) -> list:
        if self.bin_path:
            cmd = [self.bin_path, "secrets", "--json"]
            if self.project:
                cmd += ["-p", self.project]
            if self.config:
                cmd += ["-c", self.config]
            res = subprocess.run(cmd, capture_output=True, text=True)
            if res.returncode == 0:
                data = json.loads(res.stdout)
                return list(data.keys())
        return []

    def run(self, command: list) -> int:
        if self.bin_path:
            cmd = [self.bin_path, "run"]
            if self.project:
                cmd += ["-p", self.project]
            if self.config:
                cmd += ["-c", self.config]
            cmd += ["--"] + command
            return subprocess.run(cmd).returncode
        raise RuntimeError("Doppler CLI ('doppler') is required for JIT execution.")

    def status(self) -> dict:
        return {
            "status": "online" if (self.token or self.bin_path) else "unconfigured",
            "provider": "doppler",
            "project": self.project,
            "config": self.config,
            "has_cli": bool(self.bin_path),
        }


# =====================================================================
# 4. Local Persistent Env Provider (Zero-Config Fallback)
# =====================================================================

class EnvSecretProvider(BaseSecretProvider):
    """
    Fallback provider for local development, testing, or standalone Docker
    without an external cloud vault. Stores secrets in persistent volume
    at /app/data/config/.secrets.env (or data/config/.secrets.env) with 0600 permissions.
    """
    def __init__(self, file_path: Path = None):
        candidates = [
            Path("/app/data/config/.secrets.env"),
            Path("/app/data/.secrets.env"),
            Path(__file__).resolve().parent.parent.parent.parent / "data" / "config" / ".secrets.env",
            Path.home() / ".aina_secrets.env",
        ]
        self.file_path = file_path
        if not self.file_path:
            for c in candidates:
                if c.is_file() or c.parent.is_dir():
                    self.file_path = c
                    break
        if not self.file_path:
            self.file_path = candidates[0]
        self.file_path.parent.mkdir(parents=True, exist_ok=True)
        if not self.file_path.exists():
            self.file_path.write_text("# Aina Local Secret Store\n", encoding="utf-8")
            try:
                self.file_path.chmod(0o600)
            except Exception:
                pass

    def name(self) -> str:
        return "env"

    def _read_env_file(self) -> dict:
        store = {}
        if self.file_path.is_file():
            for line in self.file_path.read_text(encoding="utf-8").splitlines():
                line = line.strip()
                if not line or line.startswith("#"):
                    continue
                if "=" in line:
                    k, v = line.split("=", 1)
                    store[k.strip().upper()] = v.strip().strip("'\"")
        return store

    def _write_env_file(self, store: dict):
        lines = ["# Aina Local Persistent Secret Store (Mode: EnvProvider)", ""]
        for k, v in sorted(store.items()):
            lines.append(f"{k}={v}")
        self.file_path.write_text("\n".join(lines) + "\n", encoding="utf-8")
        try:
            self.file_path.chmod(0o600)
        except Exception:
            pass

    def set(self, key: str, value: str) -> dict:
        store = self._read_env_file()
        store[key.strip().upper()] = value.strip()
        self._write_env_file(store)
        return {
            "status": "success",
            "provider": "env",
            "message": f"Secret '{key}' successfully saved in local persistent store.",
            "key": key,
            "masked_value": mask_secret(value),
            "storage_path": str(self.file_path),
        }

    def get(self, key: str) -> str:
        store = self._read_env_file()
        k = key.strip().upper()
        if k in store:
            return store[k]
        if k in os.environ:
            return os.environ[k]
        raise RuntimeError(f"Secret '{k}' not found in local environment or {self.file_path}")

    def list(self) -> list:
        store = self._read_env_file()
        keys = set(store.keys())
        # Add relevant env keys if appropriate
        for k in os.environ.keys():
            if any(term in k for term in ["API_KEY", "TOKEN", "SECRET", "PASSWORD", "AUTH"]):
                keys.add(k)
        return sorted(list(keys))

    def run(self, command: list) -> int:
        store = self._read_env_file()
        env = os.environ.copy()
        env.update(store)
        p = subprocess.run(command, env=env)
        return p.returncode

    def status(self) -> dict:
        store = self._read_env_file()
        return {
            "status": "online",
            "provider": "env",
            "storage_path": str(self.file_path),
            "total_stored_keys": len(store),
            "mode": "local_persistent_file",
        }


# =====================================================================
# Provider Resolver Factory
# =====================================================================

def resolve_provider(provider_arg: str = None) -> BaseSecretProvider:
    """Detect and instantiate the appropriate secret provider."""
    cfg = get_persisted_config()
    choice = (provider_arg or os.getenv("SECRET_PROVIDER") or cfg.get("provider") or "").lower().strip()

    if choice == "infisical":
        return InfisicalProvider()
    elif choice in ["vault", "hashicorp", "hashicorp-vault"]:
        return HashiCorpVaultProvider()
    elif choice == "doppler":
        return DopplerProvider()
    elif choice in ["env", "local", "dotenv"]:
        return EnvSecretProvider()

    # Automatic Detection:
    # 1. Check HashiCorp Vault
    if (os.getenv("VAULT_ADDR") or cfg.get("vault_addr")) and (os.getenv("VAULT_TOKEN") or cfg.get("vault_token")):
        return HashiCorpVaultProvider()

    # 2. Check Doppler
    if os.getenv("DOPPLER_TOKEN") or cfg.get("doppler_token"):
        return DopplerProvider()

    # 3. Check Infisical
    if os.getenv("INFISICAL_CLIENT_ID") or os.getenv("INFISICAL_TOKEN") or os.getenv("INFISICAL_PROJECT_ID") or cfg.get("project_id"):
        return InfisicalProvider()

    # 4. Default: Local Persistent Env Provider
    return EnvSecretProvider()


# =====================================================================
# CLI Dispatcher
# =====================================================================

def main():
    parser = argparse.ArgumentParser(
        description="Aina Universal Vendor-Agnostic Secret Manager (Infisical, Vault, Doppler, Env)"
    )
    parser.add_argument(
        "--provider",
        choices=["auto", "infisical", "vault", "doppler", "env"],
        default=os.getenv("SECRET_PROVIDER", "auto"),
        help="Secret vault provider backend (default: auto)",
    )

    subparsers = parser.add_subparsers(dest="subcommand", required=True)

    # Subcommand: configure
    cfg_parser = subparsers.add_parser("configure", help="Persistently configure active secret provider (Infisical, Vault, Doppler, Env)")
    cfg_parser.add_argument("--provider", "-p", choices=["infisical", "vault", "doppler", "env"], required=True, help="Provider name")
    cfg_parser.add_argument("--addr", help="Vault URL address (for HashiCorp Vault)")
    cfg_parser.add_argument("--token", help="Vault Token or Doppler Token")
    cfg_parser.add_argument("--mount", default="secret", help="Vault mount path (default: secret)")
    cfg_parser.add_argument("--domain", help="Infisical instance domain URL")
    cfg_parser.add_argument("--project-id", help="Infisical Project ID")
    cfg_parser.add_argument("--client-id", help="Infisical Client ID")
    cfg_parser.add_argument("--client-secret", help="Infisical Client Secret")
    cfg_parser.add_argument("--env", help="Environment (dev/prod)")
    cfg_parser.add_argument("--project", help="Doppler project name")
    cfg_parser.add_argument("--config", help="Doppler config name")

    # Subcommand: set
    set_parser = subparsers.add_parser("set", help="Save a secret to vault")
    set_parser.add_argument("key", help="Secret variable name (e.g. SENTRY_AUTH_TOKEN)")
    set_parser.add_argument("value", help="Secret variable value")

    # Subcommand: get
    get_parser = subparsers.add_parser("get", help="Retrieve a secret from vault")
    get_parser.add_argument("key", help="Secret variable name (e.g. SENTRY_AUTH_TOKEN)")
    get_parser.add_argument("--reveal", action="store_true", help="Reveal unmasked value in JSON")
    get_parser.add_argument("--plain", action="store_true", help="Print raw unmasked value only")

    # Subcommand: list
    subparsers.add_parser("list", help="List secret keys stored in vault")

    # Subcommand: status
    subparsers.add_parser("status", help="Check secret vault connectivity and active provider")

    # Subcommand: run
    run_parser = subparsers.add_parser("run", help="Execute command with secrets injected into environment")
    run_parser.add_argument("command", nargs=argparse.REMAINDER, help="Command to execute (e.g. python3 script.py)")

    args = parser.parse_args()

    # Handle configure early before resolving default provider
    if args.subcommand == "configure":
        prov = args.provider.lower().strip()
        cfg_file = get_vault_config_path()
        cfg_file.parent.mkdir(parents=True, exist_ok=True)

        data = get_persisted_config()
        data["provider"] = prov
        if prov == "infisical":
            if getattr(args, "domain", None): data["domain"] = args.domain
            if getattr(args, "project_id", None): data["project_id"] = args.project_id
            if getattr(args, "env", None): data["env"] = args.env
            if getattr(args, "client_id", None): data["client_id"] = args.client_id
            if getattr(args, "client_secret", None): data["client_secret"] = args.client_secret
        elif prov == "vault":
            if getattr(args, "addr", None): data["vault_addr"] = args.addr
            if getattr(args, "token", None): data["vault_token"] = args.token
            if getattr(args, "mount", None): data["vault_mount"] = args.mount
        elif prov == "doppler":
            if getattr(args, "token", None): data["doppler_token"] = args.token
            if getattr(args, "project", None): data["doppler_project"] = args.project
            if getattr(args, "config", None): data["doppler_config"] = args.config

        cfg_file.write_text(json.dumps(data, indent=2), encoding="utf-8")
        try:
            cfg_file.chmod(0o600)
        except Exception:
            pass

        provider = resolve_provider(prov)
        st = provider.status()
        print(json.dumps({
            "status": "success",
            "message": f"Secret provider successfully configured to '{prov}'.",
            "config_file": str(cfg_file),
            "provider_status": st
        }, indent=2))
        sys.exit(0)

    prov_name = None if args.provider == "auto" else args.provider
    provider = resolve_provider(prov_name)

    key_upper = getattr(args, "key", "").strip().upper() if hasattr(args, "key") else ""

    if args.subcommand == "set":
        val = args.value.strip()
        if not key_upper or not val:
            print(json.dumps({"status": "error", "message": "Key and value must not be empty"}))
            sys.exit(1)
        try:
            res = provider.set(key_upper, val)
            print(json.dumps(res, indent=2))
        except Exception as e:
            print(json.dumps({"status": "error", "provider": provider.name(), "message": str(e)}))
            sys.exit(1)

    elif args.subcommand == "get":
        try:
            val = provider.get(key_upper)
            if args.reveal or args.plain:
                if args.plain:
                    print(val)
                else:
                    print(json.dumps({
                        "status": "success",
                        "provider": provider.name(),
                        "key": key_upper,
                        "value": val,
                    }, indent=2))
            else:
                print(json.dumps({
                    "status": "success",
                    "provider": provider.name(),
                    "key": key_upper,
                    "masked_value": mask_secret(val),
                }, indent=2))
        except Exception as e:
            print(json.dumps({"status": "error", "provider": provider.name(), "message": str(e), "key": key_upper}))
            sys.exit(1)

    elif args.subcommand == "list":
        try:
            keys = provider.list()
            print(json.dumps({
                "status": "success",
                "provider": provider.name(),
                "total": len(keys),
                "secret_keys": keys,
            }, indent=2))
        except Exception as e:
            print(json.dumps({"status": "error", "provider": provider.name(), "message": str(e)}))
            sys.exit(1)

    elif args.subcommand == "status":
        info = provider.status()
        print(json.dumps(info, indent=2))

    elif args.subcommand == "run":
        cmd = list(args.command)
        if cmd and cmd[0] == "--":
            cmd = cmd[1:]
        if not cmd:
            print(json.dumps({"status": "error", "message": "No command specified to run."}))
            sys.exit(1)
        code = provider.run(cmd)
        sys.exit(code)


if __name__ == "__main__":
    main()
