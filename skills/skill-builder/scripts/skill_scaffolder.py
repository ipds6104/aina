#!/usr/bin/env python3
"""
skill_scaffolder.py - Skill Scaffolder, Validator & Persistence Manager for Aina

Assists Aina in scaffolding standard Antigravity skills, verifying persistent volume
compliance (data/custom-skills/), validating YAML frontmatter, checking OpSec,
and synchronizing custom skills to private Git repositories (USER_SKILLS_REPO).
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


def cmd_init(args):
    """Scaffold a new compliant Antigravity skill in persistent custom-skills storage."""
    skill_name = sanitize_skill_name(args.name)
    if not skill_name:
        print(json.dumps({"status": "error", "message": "Invalid skill name"}))
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

    desc = args.description or f"Custom autonomous skill for {skill_name}."
    skill_md_content = f"""---
name: {skill_name}
description: >-
  {desc}
---

# {skill_name.replace('-', ' ').title()} Skill

{desc}

---

## 1. When to Activate This Skill (Trigger Conditions)

Activate this skill immediately whenever:
1. **Permintaan Langsung Pengguna**: Pengguna meminta tugas atau otomasi terkait `{skill_name}`.
2. **Kondisi Khusus**: Terjadi event atau kondisi yang membutuhkan tool ini.

---

## 2. Strict OpSec & Anti-Leakage Rules

> [!CAUTION]
> **ATURAN MUTLAK KEAMANAN KREDENSIAL**:
> 1. Dilarang meng-hardcode API key / token di dalam file script atau SKILL.md.
> 2. Gunakan `secret_tool.py run -- ...` atau `secret_tool.py get <KEY> --plain` dari Infisical vault.
> 3. Jangan mengulang nilai token mentah ke dalam balasan obrolan chat.

---

## 3. Standard Operating Procedures (SOP)

### SOP 1: Menjalankan Tool Utama
```bash
python3 scripts/{skill_name}_tool.py --help
```

### SOP 2: Menjalankan Tool dengan Kredensial Terinjeksi (JIT)
```bash
python3 skills/infisical/scripts/secret_tool.py run -- python3 scripts/{skill_name}_tool.py run
```

---

## 4. Helper Tool Command Reference

| Perintah | Deskripsi |
| :--- | :--- |
| `python3 scripts/{skill_name}_tool.py status` | Memeriksa kesiapan dan status koneksi tool. |
| `python3 scripts/{skill_name}_tool.py run` | Menjalankan logika utama skill. |
"""

    skill_md_path = skill_dir / "SKILL.md"
    skill_md_path.write_text(skill_md_content, encoding="utf-8")

    tool_py_content = f"""#!/usr/bin/env python3
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

    tool_py_path = scripts_dir / f"{skill_name}_tool.py"
    tool_py_path.write_text(tool_py_content, encoding="utf-8")
    tool_py_path.chmod(0o755)

    print(json.dumps({
        "status": "success",
        "skill_name": skill_name,
        "path": str(skill_dir),
        "is_persistent": True,
        "files_created": [
            str(skill_md_path),
            str(tool_py_path)
        ],
        "message": f"Skill '{skill_name}' successfully scaffolded in persistent storage."
    }, indent=2))


def cmd_validate(args):
    """Validate skill compliance: frontmatter, directory structure, permissions, and OpSec."""
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
    print(json.dumps({
        "status": status,
        "skill_name": skill_name,
        "path": str(skill_dir),
        "is_persistent_storage": is_persistent,
        "errors": errors,
        "warnings": warnings,
    }, indent=2))

    if errors:
        sys.exit(1)


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
    p_init.add_argument("--desc", "-d", dest="description", help="Short description of the skill")
    p_init.add_argument("--force", "-f", action="store_true", help="Overwrite if skill directory already exists")

    # validate
    p_val = subparsers.add_parser("validate", help="Validate a custom skill for compliance and OpSec")
    p_val.add_argument("name", help="Name or path of the skill to validate")

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
    elif args.command == "list":
        cmd_list(args)
    elif args.command == "sync":
        cmd_sync(args)


if __name__ == "__main__":
    main()
