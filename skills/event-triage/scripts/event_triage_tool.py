#!/usr/bin/env python3
"""
event_triage_tool.py - Official Event & Observability Incident Triage Helper for Aina

Assists Aina in parsing, summarizing, and triaging incoming observability alerts,
CI/CD failures, and system events. Integrates seamlessly with `secret_tool` for JIT secret fetching.
"""

import argparse
import json
import os
import sys
from datetime import datetime, timezone
from pathlib import Path


def format_timestamp(epoch_val) -> str:
    try:
        dt = datetime.fromtimestamp(int(epoch_val), tz=timezone.utc)
        return dt.strftime("%Y-%m-%d %H:%M:%S UTC")
    except Exception:
        return str(epoch_val)


def triage_severity(severity_str: str) -> dict:
    s = (severity_str or "info").lower().strip()
    if s in ["critical", "fatal", "sev1", "p1"]:
        return {
            "level": "CRITICAL",
            "emoji": "🔴",
            "action_priority": "Immediate Attention Required (Urgent Response)",
            "escalate": True,
        }
    elif s in ["error", "err", "sev2", "p2"]:
        return {
            "level": "ERROR",
            "emoji": "🟠",
            "action_priority": "High Priority (Investigate Root Cause)",
            "escalate": False,
        }
    elif s in ["warning", "warn", "sev3", "p3"]:
        return {
            "level": "WARNING",
            "emoji": "🟡",
            "action_priority": "Medium Priority (Monitor & Analyze Trends)",
            "escalate": False,
        }
    else:
        return {
            "level": "INFO",
            "emoji": "🔵",
            "action_priority": "Informational / Routine Log",
            "escalate": False,
        }


def cmd_parse(args):
    """Parse raw JSON event payload and generate a structured triage analysis."""
    input_data = ""
    if args.input_file:
        input_data = Path(args.input_file).read_text(encoding="utf-8")
    elif args.json_string:
        input_data = args.json_string
    else:
        # Read from stdin
        input_data = sys.stdin.read()

    if not input_data.strip():
        print(json.dumps({"status": "error", "message": "No input payload provided"}))
        sys.exit(1)

    try:
        raw = json.loads(input_data)
    except json.JSONDecodeError as e:
        print(json.dumps({"status": "error", "message": f"Invalid JSON payload: {e}"}))
        sys.exit(1)

    source = raw.get("source") or "observability"
    event_name = raw.get("event") or "alert"
    severity_input = raw.get("severity") or raw.get("level") or "info"
    service = raw.get("service") or raw.get("app") or "unknown-service"
    repository = raw.get("repository") or raw.get("repo") or "-"
    title = raw.get("title") or raw.get("summary") or raw.get("message") or "Observability Event Triggered"
    details = raw.get("details") or raw.get("message") or "-"
    metadata = raw.get("metadata") or raw.get("labels") or {}
    suggested_actions = raw.get("suggested_actions") or []

    sev_info = triage_severity(severity_input)

    output = {
        "status": "success",
        "triage": {
            "source": source.upper(),
            "event": event_name.upper(),
            "severity": sev_info["level"],
            "severity_badge": f"{sev_info['emoji']} {sev_info['level']}",
            "priority": sev_info["action_priority"],
            "needs_escalation": sev_info["escalate"],
            "service": service,
            "repository": repository,
            "title": title,
            "details": details,
            "metadata": metadata,
            "suggested_actions": suggested_actions,
        },
        "recommendations": [
            f"1. Periksa repository target: {repository}" if repository != "-" else "1. Tentukan repository terkait dari nama service.",
            f"2. Gunakan `secret_tool.py run -- ...` jika membutuhkan API key atau kredensial database untuk {service}.",
            "3. Lakukan Root Cause Analysis (RCA) berdasarkan rincian stacktrace/log di atas.",
            "4. Laporkan ringkasan diagnosis ke pengguna dan siapkan rekomendasi atau PR perbaikan jika disetujui."
        ]
    }

    if args.markdown:
        md = f"""### {sev_info['emoji']} [{sev_info['level']}] {title}
- **Sumber:** `{source.upper()}` | **Kategori:** `{event_name.upper()}`
- **Layanan:** `{service}` | **Repo:** `{repository}`
- **Prioritas Tindakan:** {sev_info['action_priority']}

#### 📝 Rincian Kejadian:
```text
{details if isinstance(details, str) else json.dumps(details, indent=2)}
```

#### 🛠️ Rekomendasi Investigasi Aina:
- Verifikasi commit/perubahan terbaru di `{repository}`.
- Gunakan `secret_tool` secara aman bila membutuhkan token service.
"""
        print(md)
    else:
        print(json.dumps(output, indent=2, ensure_ascii=False))


def cmd_check_secrets(args):
    """Check if secrets for a given repository/service exist in Infisical without printing values."""
    service = args.service.strip().upper()
    prefix = f"{service}_"

    bin_path = "/usr/local/bin/secret_tool"
    if not os.path.isfile(bin_path):
        bin_path = str(Path(__file__).resolve().parent.parent.parent / "infisical" / "scripts" / "secret_tool.py")

    import subprocess
    cmd = [sys.executable, bin_path, "list"]
    try:
        res = subprocess.run(cmd, capture_output=True, text=True, check=True)
        data = json.loads(res.stdout)
        keys = data.get("keys", [])
        matched = [k for k in keys if k.startswith(prefix) or service in k]
        print(json.dumps({
            "status": "success",
            "service": service,
            "matching_keys_found": matched,
            "count": len(matched)
        }, indent=2))
    except Exception as e:
        print(json.dumps({"status": "error", "message": str(e)}))
        sys.exit(1)


def cmd_webhook_info(args):
    """Generate ready-to-paste webhook integration info for Sentry, Prometheus, and CI/CD."""
    events_key = os.getenv("AINA_EVENTS_API_KEY") or os.getenv("EVENTS_API_KEY") or os.getenv("ADMIN_KEY") or os.getenv("SETUP_CODE") or "SET_YOUR_API_KEY"
    base_url = (args.base_url or os.getenv("AINA_PUBLIC_URL") or os.getenv("SERVER_BASE_URL") or "http://localhost:8080").rstrip("/")
    service = args.service or "billing-service"
    repo = args.repo or "my-org/billing-service"

    endpoint = f"{base_url}/api/v1/events"
    endpoint_query = f"{base_url}/api/v1/events?api_key={events_key}"

    res = {
        "status": "ready",
        "api_endpoints": {
            "primary": endpoint,
            "alias": f"{base_url}/api/events",
            "url_with_api_key": endpoint_query,
        },
        "authentication": {
            "header_x_api_key": f"X-API-Key: {events_key}",
            "header_bearer": f"Authorization: Bearer {events_key}",
            "url_query_param": f"?api_key={events_key}"
        },
        "ready_to_use_recipes": {
            "sentry_webhook_url": endpoint_query,
            "test_curl": f"curl -X POST '{endpoint}' -H 'Content-Type: application/json' -H 'X-API-Key: {events_key}' -d '{{\"source\":\"sentry\",\"severity\":\"critical\",\"service\":\"{service}\",\"repository\":\"{repo}\",\"title\":\"Test incident ping\"}}'",
            "prometheus_alertmanager_yaml": f"receivers:\n  - name: 'aina-events'\n    webhook_configs:\n      - url: '{endpoint}'\n        send_resolved: true\n        http_config:\n          authorization:\n            credentials: '{events_key}'"
        }
    }
    if args.markdown:
        md = f"""### 🚨 Panduan Integrasi Webhook Observability Aina

#### 1. Webhook URL Siap Pakai:
- **URL (Header Auth)**: `{endpoint}`
- **URL (Query Parameter)**: `{endpoint_query}`

#### 2. Cara Pasang di Sentry:
1. Buka project **{service}** di Sentry.
2. Masuk ke **Alerts** -> **Create Alert Rule** -> **Action: Send a Webhook**.
3. Masukkan Webhook URL:
   `{endpoint_query}`

#### 3. Tes Pengiriman (cURL):
```bash
curl -X POST '{endpoint}' \\
  -H 'Content-Type: application/json' \\
  -H 'X-API-Key: {events_key}' \\
  -d '{{\"source\":\"sentry\",\"severity\":\"critical\",\"service\":\"{service}\",\"repository\":\"{repo}\",\"title\":\"Test incident ping\"}}'
```
"""
        print(md)
    else:
        print(json.dumps(res, indent=2))


def main():
    parser = argparse.ArgumentParser(description="Aina Event & Observability Incident Triage Helper")
    subparsers = parser.add_subparsers(dest="command", required=True)

    # Parse subcommand
    p_parse = subparsers.add_parser("parse", help="Parse and triage incoming event JSON")
    p_parse.add_argument("--file", "-f", dest="input_file", help="Path to JSON file")
    p_parse.add_argument("--json", "-j", dest="json_string", help="JSON string literal")
    p_parse.add_argument("--markdown", "-m", action="store_true", help="Output formatted markdown report")

    # Check secrets subcommand
    p_sec = subparsers.add_parser("check-secrets", help="Check if secrets for a service exist in vault")
    p_sec.add_argument("service", help="Service or repo name (e.g. billing-service)")

    # Webhook info subcommand
    p_info = subparsers.add_parser("webhook-info", help="Get ready-to-paste webhook URL and recipes")
    p_info.add_argument("--service", help="Target service name")
    p_info.add_argument("--repo", help="Target repository")
    p_info.add_argument("--base-url", help="Public base URL of Aina server")
    p_info.add_argument("--markdown", "-m", action="store_true", help="Output formatted markdown guide")

    args = parser.parse_args()

    if args.command == "parse":
        cmd_parse(args)
    elif args.command == "check-secrets":
        cmd_check_secrets(args)
    elif args.command == "webhook-info":
        cmd_webhook_info(args)


if __name__ == "__main__":
    main()

