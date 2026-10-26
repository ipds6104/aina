#!/usr/bin/env python3
"""
skill_scaffolder.py - Skill Scaffolder, Validator, Tester & Extensibility Manager for Aina

Assists Aina in scaffolding standard Antigravity skills, verifying persistent volume
compliance (data/custom-skills/), validating YAML frontmatter, checking OpSec,
running automated tests, and synchronizing custom skills to private Git repositories (USER_SKILLS_REPO).
"""

import argparse
import json
import os
import re
import shutil
import subprocess
import sys
from pathlib import Path

# Resolve base directories
SCRIPT_DIR = Path(__file__).resolve().parent
PROJECT_ROOT = SCRIPT_DIR.parent.parent.parent

# Persistent custom skills directory candidates
CUSTOM_SKILLS_PATHS = [
    PROJECT_ROOT / "data" / "custom-skills",
    Path("/app/data/custom-skills"),
]


def get_custom_skills_dir() -> Path:
    for p in CUSTOM_SKILLS_PATHS:
        if p.is_dir() or p.parent.is_dir():
            p.mkdir(parents=True, exist_ok=True)
            return p
    target = PROJECT_ROOT / "data" / "custom-skills"
    target.mkdir(parents=True, exist_ok=True)
    return target


def sanitize_skill_name(name: str) -> str:
    cleaned = re.sub(r"[^a-zA-Z0-9_\-]", "-", name.strip().lower())
    return re.sub(r"-+", "-", cleaned).strip("-")


# ==============================================================================
# Template Generators
# ==============================================================================

def generate_minimal_template(skill_name: str, desc: str):
    """Generate minimal boilerplate skill."""
    skill_md = f"""---
name: {skill_name}
description: >-
  {desc}
---

# {skill_name.replace('-', ' ').title()} Skill

{desc}

---

## 1. When to Activate This Skill (Trigger Conditions)

Activate this skill immediately whenever:
1. **Permintaan Pengguna**: Pengguna meminta aksi atau otomasi terkait `{skill_name}`.
2. **Kondisi Khusus**: Terjadi event atau kondisi yang membutuhkan tool ini.

---

## 2. Strict OpSec & Anti-Leakage Rules

> [!CAUTION]
> 1. Dilarang meng-hardcode API key / token di dalam file script atau SKILL.md.
> 2. Gunakan `secret_tool.py run -- ...` atau `secret_tool.py get <KEY> --plain` dari Infisical vault jika memerlukan secret.
> 3. Jangan mengulang nilai token mentah ke dalam balasan obrolan chat.

---

## 3. Standard Operating Procedures (SOP)

### SOP 1: Menjalankan Tool Utama
```bash
python3 scripts/{skill_name}_tool.py --help
```

### SOP 2: Memeriksa Status Tool
```bash
python3 scripts/{skill_name}_tool.py status
```

---

## 4. Helper Tool Command Reference

| Perintah | Deskripsi |
| :--- | :--- |
| `python3 scripts/{skill_name}_tool.py status` | Memeriksa kesiapan dan status koneksi tool. |
| `python3 scripts/{skill_name}_tool.py run` | Menjalankan logika utama skill. |
"""

    tool_py = f"""#!/usr/bin/env python3
\"\"\"
{skill_name}_tool.py - Main CLI tool for {skill_name} skill.
\"\"\"

import argparse
import json
import os
import sys


def cmd_status(args):
    print(json.dumps({{
        "status": "ready",
        "skill": "{skill_name}",
        "message": "Tool is operational"
    }}, indent=2))


def cmd_run(args):
    print(json.dumps({{
        "status": "success",
        "skill": "{skill_name}",
        "result": "Completed successfully"
    }}, indent=2))


def main():
    parser = argparse.ArgumentParser(description="{skill_name} CLI Tool")
    subparsers = parser.add_subparsers(dest="command", required=True)

    p_status = subparsers.add_parser("status", help="Check tool readiness")
    p_status.set_defaults(func=cmd_status)

    p_run = subparsers.add_parser("run", help="Execute main tool logic")
    p_run.set_defaults(func=cmd_run)

    args = parser.parse_args()
    args.func(args)


if __name__ == "__main__":
    main()
"""
    return skill_md, tool_py


def generate_api_template(skill_name: str, desc: str):
    """Generate REST API client skill template with JIT secret injection."""
    env_token_var = f"{skill_name.upper().replace('-', '_')}_API_KEY"

    skill_md = f"""---
name: {skill_name}
description: >-
  {desc}
---

# {skill_name.replace('-', ' ').title()} API Integration Skill

{desc}

---

## 1. When to Activate This Skill (Trigger Conditions)

Activate this skill immediately whenever:
1. **Permintaan API**: Pengguna meminta query, fetch, atau interaksi data ke layanan `{skill_name}`.
2. **Sinkronisasi Data**: Dibutuhkan pengiriman atau penarikan data berkala ke API `{skill_name}`.

---

## 2. Strict OpSec & Runtime Secret Broker Protocol

> [!CAUTION]
> **ATURAN MUTLAK KREDENSIAL (ZERO HARDCODED SECRETS)**:
> 1. Dilarang keras meng-hardcode token/kredensial ke dalam file script ataupun SKILL.md.
> 2. Variabel environment yang diharapkan: `{env_token_var}`.
> 3. Simpan secret ke Infisical vault sekali saja:
>    ```bash
>    secret_tool set {env_token_var} "<token_rahasia>"
>    ```
> 4. Eksekusi tool **WAJIB** melalui runtime secret broker (JIT injection):
>    ```bash
>    secret_tool run -- python3 scripts/{skill_name}_tool.py <subcommand>
>    ```
> 5. Dilarang membocorkan token mentah ke ruang chat WhatsApp.

---

## 3. WhatsApp Formatting Guidelines

- Gunakan bullet point `•` untuk daftar item.
- Dilarang menggunakan `# Heading` Markdown atau tabel Markdown `| a | b |` di WhatsApp.
- Sajikan ringkasan penting dan tautan langsung bila ada.

---

## 4. Helper Tool Command Reference

| Perintah | Deskripsi |
| :--- | :--- |
| `python3 scripts/{skill_name}_tool.py status` | Memeriksa ketersediaan konfigurasi & konektivitas API |
| `secret_tool run -- python3 scripts/{skill_name}_tool.py fetch` | Mengambil data dari endpoint API |
| `secret_tool run -- python3 scripts/{skill_name}_tool.py send --data '<json>'` | Mengirim payload ke endpoint API |
"""

    tool_py = f"""#!/usr/bin/env python3
\"\"\"
{skill_name}_tool.py - REST API Client for {skill_name}.
Enforces runtime secret retrieval via {env_token_var}.
\"\"\"

import argparse
import json
import os
import sys
import urllib.request
import urllib.error
import urllib.parse


API_TOKEN_ENV = "{env_token_var}"
DEFAULT_TIMEOUT = 30


def get_token() -> str:
    token = os.environ.get(API_TOKEN_ENV, "").strip()
    return token


def cmd_status(args):
    token = get_token()
    has_token = bool(token)
    token_preview = f"{{token[:4]}}...{{token[-4:]}}" if len(token) >= 8 else ("configured" if token else "missing")

    print(json.dumps({{
        "status": "ready" if has_token else "needs_auth",
        "skill": "{skill_name}",
        "secret_env": API_TOKEN_ENV,
        "token_configured": has_token,
        "token_preview": token_preview,
        "advice": "Run via 'secret_tool run -- ...' if token is missing" if not has_token else "Ready for API calls"
    }}, indent=2))


def cmd_fetch(args):
    token = get_token()
    endpoint = args.endpoint or "https://httpbin.org/get"

    headers = {{
        "User-Agent": "Aina-Agent-Skill/2026.10 ({skill_name})",
        "Accept": "application/json",
    }}
    if token:
        headers["Authorization"] = f"Bearer {{token}}"

    req = urllib.request.Request(endpoint, headers=headers)
    try:
        with urllib.request.urlopen(req, timeout=DEFAULT_TIMEOUT) as resp:
            raw = resp.read().decode("utf-8")
            try:
                data = json.loads(raw)
            except Exception:
                data = raw
            print(json.dumps({{
                "status": "success",
                "http_code": resp.status,
                "data": data
            }}, indent=2))
    except urllib.error.HTTPError as e:
        err_body = e.read().decode("utf-8", errors="ignore")
        print(json.dumps({{
            "status": "error",
            "http_code": e.code,
            "error": str(e),
            "details": err_body
        }}, indent=2), file=sys.stderr)
        sys.exit(1)
    except Exception as e:
        print(json.dumps({{
            "status": "error",
            "error": str(e)
        }}, indent=2), file=sys.stderr)
        sys.exit(1)


def main():
    parser = argparse.ArgumentParser(description="{skill_name} API Integration Tool")
    subparsers = parser.add_subparsers(dest="command", required=True)

    p_status = subparsers.add_parser("status", help="Check API credentials and readiness")
    p_status.set_defaults(func=cmd_status)

    p_fetch = subparsers.add_parser("fetch", help="Fetch data from API endpoint")
    p_fetch.add_argument("--endpoint", "-e", help="Target API URL")
    p_fetch.set_defaults(func=cmd_fetch)

    args = parser.parse_args()
    args.func(args)


if __name__ == "__main__":
    main()
"""
    return skill_md, tool_py


def generate_cli_template(skill_name: str, desc: str):
    """Generate external CLI wrapper skill template."""
    skill_md = f"""---
name: {skill_name}
description: >-
  {desc}
---

# {skill_name.replace('-', ' ').title()} CLI Wrapper Skill

{desc}

---

## 1. When to Activate This Skill (Trigger Conditions)

Activate this skill immediately whenever:
1. **Perintah Subproses**: Diperlukan eksekusi atau otomasi CLI untuk `{skill_name}`.
2. **Inspeksi Sistem**: Membutuhkan eksekusi biner terisolasi dengan output JSON terstruktur.

---

## 2. Standard Operating Procedures (SOP)

### SOP 1: Memeriksa Kesiapan Tool
```bash
python3 scripts/{skill_name}_tool.py status
```

### SOP 2: Menjalankan Perintah CLI
```bash
python3 scripts/{skill_name}_tool.py exec --args "<perintah>"
```
"""

    tool_py = f"""#!/usr/bin/env python3
\"\"\"
{skill_name}_tool.py - CLI wrapper utility for {skill_name}.
\"\"\"

import argparse
import json
import shutil
import subprocess
import sys


def cmd_status(args):
    # Check if target binary or dependencies are installed
    print(json.dumps({{
        "status": "ready",
        "skill": "{skill_name}",
        "message": "CLI wrapper is ready"
    }}, indent=2))


def cmd_exec(args):
    cmd = args.args
    if not cmd:
        print(json.dumps({{"status": "error", "message": "No command arguments specified"}}, indent=2))
        sys.exit(1)

    try:
        proc = subprocess.run(
            cmd,
            shell=True,
            capture_output=True,
            text=True,
            timeout=args.timeout
        )
        print(json.dumps({{
            "status": "success" if proc.returncode == 0 else "failed",
            "exit_code": proc.returncode,
            "stdout": proc.stdout.strip(),
            "stderr": proc.stderr.strip()
        }}, indent=2))
        if proc.returncode != 0:
            sys.exit(proc.returncode)
    except subprocess.TimeoutExpired:
        print(json.dumps({{
            "status": "error",
            "message": f"Command timed out after {{args.timeout}} seconds"
        }}, indent=2), file=sys.stderr)
        sys.exit(124)
    except Exception as e:
        print(json.dumps({{
            "status": "error",
            "message": str(e)
        }}, indent=2), file=sys.stderr)
        sys.exit(1)


def main():
    parser = argparse.ArgumentParser(description="{skill_name} CLI Wrapper")
    subparsers = parser.add_subparsers(dest="command", required=True)

    p_status = subparsers.add_parser("status", help="Check CLI readiness")
    p_status.set_defaults(func=cmd_status)

    p_exec = subparsers.add_parser("exec", help="Execute command via wrapper")
    p_exec.add_argument("args", nargs="?", default="", help="Command line string")
    p_exec.add_argument("--timeout", "-t", type=int, default=60, help="Execution timeout in seconds")
    p_exec.set_defaults(func=cmd_exec)

    args = parser.parse_args()
    args.func(args)


if __name__ == "__main__":
    main()
"""
    return skill_md, tool_py


def generate_data_template(skill_name: str, desc: str):
    """Generate tabular data query & extraction skill template (SQLite/DuckDB/Parquet)."""
    skill_md = f"""---
name: {skill_name}
description: >-
  {desc}
---

# {skill_name.replace('-', ' ').title()} Data Extraction Skill

{desc}

---

## 1. When to Activate This Skill (Trigger Conditions)

Activate this skill immediately whenever:
1. **Analisis Data**: Pengguna meminta query, agregasi, atau inspeksi dataset `{skill_name}`.
2. **Kueri Tabular**: Diperlukan filter data atau ekspor hasil ke format JSON/Parquet/CSV.

---

## 2. Standard Operating Procedures (SOP)

### SOP 1: Memeriksa Status & Skema Dataset
```bash
python3 scripts/{skill_name}_tool.py status
```

### SOP 2: Menjalankan Kueri SQL Aman
```bash
python3 scripts/{skill_name}_tool.py query --sql "SELECT * FROM dataset LIMIT 10"
```
"""

    tool_py = f"""#!/usr/bin/env python3
\"\"\"
{skill_name}_tool.py - Data extraction and analytical query tool.
\"\"\"

import argparse
import json
import os
import sqlite3
import sys
from pathlib import Path


DEFAULT_LIMIT = 50


def cmd_status(args):
    print(json.dumps({{
        "status": "ready",
        "skill": "{skill_name}",
        "features": ["sqlite", "parquet", "json_stream"],
        "message": "Data query tool is ready"
    }}, indent=2))


def cmd_query(args):
    sql = args.sql.strip()
    if not sql:
        print(json.dumps({{"status": "error", "message": "SQL query is empty"}}, indent=2))
        sys.exit(1)

    # Simple demonstration SQLite query in memory or file
    db_path = args.db or ":memory:"
    try:
        conn = sqlite3.connect(db_path)
        conn.row_factory = sqlite3.Row
        cur = conn.cursor()
        cur.execute(sql)
        rows = cur.fetchmany(args.limit)
        results = [dict(r) for r in rows]
        print(json.dumps({{
            "status": "success",
            "row_count": len(results),
            "rows": results
        }}, indent=2))
        conn.close()
    except Exception as e:
        print(json.dumps({{
            "status": "error",
            "error": str(e)
        }}, indent=2), file=sys.stderr)
        sys.exit(1)


def main():
    parser = argparse.ArgumentParser(description="{skill_name} Data Tool")
    subparsers = parser.add_subparsers(dest="command", required=True)

    p_status = subparsers.add_parser("status", help="Check dataset status")
    p_status.set_defaults(func=cmd_status)

    p_query = subparsers.add_parser("query", help="Execute SQL query on dataset")
    p_query.add_argument("--sql", "-s", required=True, help="SQL query string")
    p_query.add_argument("--db", "-d", help="Database file path (default: in-memory)")
    p_query.add_argument("--limit", "-l", type=int, default=DEFAULT_LIMIT, help="Max rows to return")
    p_query.set_defaults(func=cmd_query)

    args = parser.parse_args()
    args.func(args)


if __name__ == "__main__":
    main()
"""
    return skill_md, tool_py


TEMPLATES = {
    "minimal": generate_minimal_template,
    "api": generate_api_template,
    "cli": generate_cli_template,
    "data": generate_data_template,
}


# ==============================================================================
# CLI Commands
# ==============================================================================

def cmd_init(args):
    """Scaffold a new compliant Antigravity skill in persistent custom-skills storage."""
    skill_name = sanitize_skill_name(args.name)
    if not skill_name:
        print(json.dumps({"status": "error", "message": "Invalid skill name"}))
        sys.exit(1)

    template_type = args.template.lower() if args.template else "minimal"
    if template_type not in TEMPLATES:
        print(json.dumps({
            "status": "error",
            "message": f"Unknown template '{template_type}'. Available: {list(TEMPLATES.keys())}"
        }))
        sys.exit(1)

    custom_dir = get_custom_skills_dir()
    skill_dir = custom_dir / skill_name
    scripts_dir = skill_dir / "scripts"

    if skill_dir.exists() and not args.force:
        print(json.dumps({
            "status": "error",
            "message": f"Skill '{skill_name}' already exists at {skill_dir}. Use --force to overwrite."
        }))
        sys.exit(1)

    skill_dir.mkdir(parents=True, exist_ok=True)
    scripts_dir.mkdir(parents=True, exist_ok=True)

    desc = args.description or f"Custom autonomous skill for {skill_name} ({template_type} template)."
    generator = TEMPLATES[template_type]
    skill_md_content, tool_py_content = generator(skill_name, desc)

    skill_md_path = skill_dir / "SKILL.md"
    skill_md_path.write_text(skill_md_content, encoding="utf-8")

    tool_py_path = scripts_dir / f"{skill_name}_tool.py"
    tool_py_path.write_text(tool_py_content, encoding="utf-8")
    tool_py_path.chmod(0o755)

    print(json.dumps({
        "status": "success",
        "skill_name": skill_name,
        "template": template_type,
        "path": str(skill_dir),
        "is_persistent": True,
        "files_created": [
            str(skill_md_path),
            str(tool_py_path)
        ],
        "message": f"Skill '{skill_name}' ({template_type}) successfully scaffolded in persistent storage."
    }, indent=2))


def cmd_validate(args):
    """Validate skill compliance: frontmatter, directory structure, permissions, syntax, and OpSec."""
    skill_name = sanitize_skill_name(args.name)
    custom_dir = get_custom_skills_dir()
    skill_dir = custom_dir / skill_name

    if not skill_dir.is_dir():
        # Check in project skills/
        repo_skill = PROJECT_ROOT / "skills" / skill_name
        if repo_skill.is_dir():
            skill_dir = repo_skill
        else:
            # Check relative or direct path
            cand = Path(args.name)
            if cand.is_dir():
                skill_dir = cand
                skill_name = skill_dir.name
            else:
                print(json.dumps({
                    "status": "error",
                    "message": f"Skill directory not found at {skill_dir} or {repo_skill}"
                }))
                sys.exit(1)

    errors = []
    warnings = []

    # 1. Check SKILL.md
    skill_md = skill_dir / "SKILL.md"
    if not skill_md.is_file():
        errors.append("Missing SKILL.md file")
    else:
        content = skill_md.read_text(encoding="utf-8")
        if not content.startswith("---"):
            errors.append("SKILL.md missing opening YAML frontmatter '---'")
        match = re.search(r"^name:\s*([^\n]+)", content, re.MULTILINE)
        if not match:
            errors.append("SKILL.md frontmatter missing 'name:' field")
        elif match.group(1).strip() != skill_name:
            warnings.append(f"Frontmatter name '{match.group(1).strip()}' differs from directory name '{skill_name}'")

        if "description:" not in content:
            errors.append("SKILL.md frontmatter missing 'description:' field")

    # 2. Check scripts directory & executable permissions
    scripts_dir = skill_dir / "scripts"
    if scripts_dir.is_dir():
        for f in scripts_dir.iterdir():
            if f.is_file():
                if not os.access(f, os.X_OK):
                    warnings.append(f"Script '{f.name}' is not marked executable (+x)")
                # Python syntax compile check
                if f.suffix == ".py":
                    res = subprocess.run([sys.executable, "-m", "py_compile", str(f)], capture_output=True, text=True)
                    if res.returncode != 0:
                        errors.append(f"Python syntax error in '{f.name}': {res.stderr.strip()}")

                # 3. Check for hardcoded API keys / secrets (Basic OpSec Check)
                try:
                    f_content = f.read_text(encoding="utf-8", errors="ignore")
                    if re.search(r"(api[_-]?key|secret|token|password)\s*=\s*['\"][a-zA-Z0-9_\-]{16,}['\"]", f_content, re.IGNORECASE):
                        errors.append(f"Potential hardcoded secret or token detected in script '{f.name}'! Use secret_tool instead.")
                except Exception:
                    pass

    is_persistent = False
    for p in CUSTOM_SKILLS_PATHS:
        try:
            if skill_dir.resolve().is_relative_to(p.resolve()):
                is_persistent = True
                break
        except Exception:
            pass

    status = "valid" if not errors else "invalid"
    res_obj = {
        "status": status,
        "skill_name": skill_name,
        "path": str(skill_dir),
        "is_persistent_storage": is_persistent,
        "errors": errors,
        "warnings": warnings,
    }
    print(json.dumps(res_obj, indent=2))

    if errors:
        sys.exit(1)
    return res_obj


def cmd_test(args):
    """Run automated verification test on custom skill (compile, --help, status)."""
    skill_name = sanitize_skill_name(args.name)
    custom_dir = get_custom_skills_dir()
    skill_dir = custom_dir / skill_name

    if not skill_dir.is_dir():
        repo_skill = PROJECT_ROOT / "skills" / skill_name
        if repo_skill.is_dir():
            skill_dir = repo_skill
        else:
            print(json.dumps({
                "status": "error",
                "message": f"Skill directory not found at {skill_dir}"
            }))
            sys.exit(1)

    test_results = []
    all_passed = True

    # 1. Structural validation
    skill_md = skill_dir / "SKILL.md"
    has_skill_md = skill_md.is_file()
    test_results.append({
        "test": "SKILL.md exists",
        "passed": has_skill_md,
        "details": str(skill_md) if has_skill_md else "File missing"
    })
    if not has_skill_md:
        all_passed = False

    scripts_dir = skill_dir / "scripts"
    scripts = list(scripts_dir.glob("*.py")) if scripts_dir.is_dir() else []

    if not scripts:
        test_results.append({
            "test": "Executable script exists",
            "passed": False,
            "details": "No Python scripts found in scripts/"
        })
        all_passed = False
    else:
        for s in scripts:
            # 2. Syntax test
            res_syntax = subprocess.run([sys.executable, "-m", "py_compile", str(s)], capture_output=True, text=True)
            syntax_ok = res_syntax.returncode == 0
            test_results.append({
                "test": f"Syntax compile ({s.name})",
                "passed": syntax_ok,
                "details": "OK" if syntax_ok else res_syntax.stderr.strip()
            })
            if not syntax_ok:
                all_passed = False

            # 3. Help invocation test
            res_help = subprocess.run([sys.executable, str(s), "--help"], capture_output=True, text=True, timeout=10)
            help_ok = res_help.returncode == 0
            test_results.append({
                "test": f"--help execution ({s.name})",
                "passed": help_ok,
                "details": f"Exit code {res_help.returncode}"
            })
            if not help_ok:
                all_passed = False

            # 4. Status invocation test
            try:
                res_status = subprocess.run([sys.executable, str(s), "status"], capture_output=True, text=True, timeout=15)
                status_ok = res_status.returncode == 0
                test_results.append({
                    "test": f"status command ({s.name})",
                    "passed": status_ok,
                    "details": res_status.stdout.strip()[:200] if status_ok else res_status.stderr.strip()[:200]
                })
                if not status_ok:
                    all_passed = False
            except Exception as e:
                test_results.append({
                    "test": f"status command ({s.name})",
                    "passed": False,
                    "details": str(e)
                })
                all_passed = False

    report = {
        "status": "passed" if all_passed else "failed",
        "skill_name": skill_name,
        "path": str(skill_dir),
        "tests_run": len(test_results),
        "passed_count": sum(1 for t in test_results if t["passed"]),
        "failed_count": sum(1 for t in test_results if not t["passed"]),
        "results": test_results
    }
    print(json.dumps(report, indent=2))

    if not all_passed:
        sys.exit(1)


def cmd_delete(args):
    """Safely delete a custom skill from persistent storage."""
    skill_name = sanitize_skill_name(args.name)
    custom_dir = get_custom_skills_dir()
    skill_dir = custom_dir / skill_name

    if not skill_dir.is_dir():
        print(json.dumps({
            "status": "error",
            "message": f"Custom skill '{skill_name}' not found at {skill_dir}"
        }))
        sys.exit(1)

    if not args.force:
        print(json.dumps({
            "status": "error",
            "message": f"Confirmation required to delete '{skill_name}'. Pass --force to proceed."
        }))
        sys.exit(1)

    shutil.rmtree(skill_dir)
    print(json.dumps({
        "status": "success",
        "skill_name": skill_name,
        "message": f"Custom skill '{skill_name}' deleted from persistent storage."
    }, indent=2))


def cmd_list(args):
    """List all custom skills and their status."""
    custom_dir = get_custom_skills_dir()
    skills = []

    if custom_dir.is_dir():
        for item in sorted(custom_dir.iterdir()):
            if item.is_dir() and (item / "SKILL.md").is_file():
                content = (item / "SKILL.md").read_text(encoding="utf-8")
                desc_match = re.search(r"description:\s*(?:>-\s*)?([^\n]+)", content)
                desc = desc_match.group(1).strip() if desc_match else "-"
                scripts = [f.name for f in (item / "scripts").iterdir() if f.is_file()] if (item / "scripts").is_dir() else []
                skills.append({
                    "name": item.name,
                    "description": desc,
                    "scripts_count": len(scripts),
                    "scripts": scripts,
                    "path": str(item)
                })

    # Repo skills
    repo_skills = []
    repo_dir = PROJECT_ROOT / "skills"
    if repo_dir.is_dir():
        for item in sorted(repo_dir.iterdir()):
            if item.is_dir() and (item / "SKILL.md").is_file():
                try:
                    content = (item / "SKILL.md").read_text(encoding="utf-8")
                    desc_match = re.search(r"description:\s*(?:>-\s*)?([^\n]+)", content)
                    desc = desc_match.group(1).strip() if desc_match else "-"
                    scripts = [f.name for f in (item / "scripts").iterdir() if f.is_file()] if (item / "scripts").is_dir() else []
                    repo_skills.append({
                        "name": item.name,
                        "description": desc,
                        "scripts_count": len(scripts),
                        "scripts": scripts,
                        "path": str(item)
                    })
                except Exception:
                    pass

    # Check Git repository status
    git_dir = custom_dir / ".git"
    git_status = {
        "is_git_repo": git_dir.is_dir(),
        "remote_url": None,
    }
    if git_dir.is_dir():
        try:
            rem = subprocess.run(["git", "remote", "get-url", "origin"], cwd=custom_dir, capture_output=True, text=True)
            if rem.returncode == 0:
                git_status["remote_url"] = rem.stdout.strip()
        except Exception:
            pass

    print(json.dumps({
        "status": "success",
        "custom_skills_dir": str(custom_dir),
        "custom_skills_count": len(skills),
        "custom_skills": skills,
        "repo_skills_count": len(repo_skills),
        "repo_skills": repo_skills,
        "git_status": git_status
    }, indent=2))


def cmd_sync(args):
    """Sync custom skills directory with remote Git repository (USER_SKILLS_REPO)."""
    custom_dir = get_custom_skills_dir()
    repo_url = args.repo or os.environ.get("USER_SKILLS_REPO")

    if not (custom_dir / ".git").is_dir():
        if not repo_url:
            print(json.dumps({
                "status": "error",
                "message": "Custom skills directory is not yet a git repository. Provide --repo <url> or set USER_SKILLS_REPO."
            }))
            sys.exit(1)
        subprocess.run(["git", "init"], cwd=custom_dir, check=True)
        subprocess.run(["git", "remote", "add", "origin", repo_url], cwd=custom_dir, check=True)

    # Stage, commit, push
    msg = args.message or "Auto-update custom skills by Aina"
    subprocess.run(["git", "add", "."], cwd=custom_dir, check=True)
    subprocess.run(["git", "commit", "-m", msg], cwd=custom_dir)
    res = subprocess.run(["git", "push", "-u", "origin", "main"], cwd=custom_dir, capture_output=True, text=True)

    if res.returncode == 0:
        print(json.dumps({
            "status": "success",
            "message": "Custom skills successfully synchronized to remote Git repository."
        }, indent=2))
    else:
        print(json.dumps({
            "status": "warning",
            "message": "Committed locally, but git push encountered an issue.",
            "stderr": res.stderr.strip()
        }, indent=2))


def main():
    parser = argparse.ArgumentParser(description="Aina Skill Scaffolder & Extensibility Persistence Manager")
    subparsers = parser.add_subparsers(dest="command", required=True)

    # init
    p_init = subparsers.add_parser("init", help="Scaffold a new custom skill in persistent volume")
    p_init.add_argument("name", help="Name of the new skill (e.g. tally-integration)")
    p_init.add_argument("--template", "-t", choices=["minimal", "api", "cli", "data"], default="minimal", help="Skill architecture template")
    p_init.add_argument("--desc", "-d", dest="description", help="Short description of the skill")
    p_init.add_argument("--force", "-f", action="store_true", help="Overwrite if skill directory already exists")

    # validate
    p_val = subparsers.add_parser("validate", help="Validate a custom skill for compliance, syntax, and OpSec")
    p_val.add_argument("name", help="Name or path of the skill to validate")

    # test
    p_test = subparsers.add_parser("test", help="Run automated test suite on custom skill")
    p_test.add_argument("name", help="Name of the custom skill to test")

    # delete
    p_del = subparsers.add_parser("delete", help="Delete a custom skill from persistent storage")
    p_del.add_argument("name", help="Name of the custom skill to delete")
    p_del.add_argument("--force", "-f", action="store_true", help="Force deletion without prompt")

    # list
    subparsers.add_parser("list", help="List all installed custom skills")

    # sync
    p_sync = subparsers.add_parser("sync", help="Synchronize custom skills to remote Git repo")
    p_sync.add_argument("--repo", "-r", help="Git repository URL")
    p_sync.add_argument("--message", "-m", help="Commit message")

    args = parser.parse_args()

    if args.command == "init":
        cmd_init(args)
    elif args.command == "validate":
        cmd_validate(args)
    elif args.command == "test":
        cmd_test(args)
    elif args.command == "delete":
        cmd_delete(args)
    elif args.command == "list":
        cmd_list(args)
    elif args.command == "sync":
        cmd_sync(args)


if __name__ == "__main__":
    main()
