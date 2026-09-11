#!/usr/bin/env python3
"""
Aina Knowledge Base Deterministic Linter & Auto-Healer
=====================================================
Memeriksa kepatuhan kriteria kerapian basis pengetahuan secara deterministik (<50ms, 0 LLM token):
1. Struktur folder & slug naming (kegiatan/<slug>/<periode>/)
2. Validitas YAML frontmatter & field wajib (nama, kategori, status)
3. Integritas format tanggal pada array deadlines (YYYY-MM-DD)
4. Deteksi keusangan indeks (stale index.md detection)
5. Deteksi berkas nyasar (misplaced binary files di folder teks) & file sampah (*.tmp, ~*)
6. Pelacakan deadline overdue tanpa penyelesaian
7. Closed-loop auto-healing: Memanggil grooming engine jika terdeteksi inkonsistensi
"""

import sys
import os
import re
import json
import argparse
import datetime
from pathlib import Path

# ─── REPO & WORKSPACE RESOLVER ───────────────────────────────────────────────

def find_repo_root() -> Path:
    current = Path.cwd()
    for parent in [current] + list(current.parents):
        if (parent / "workspaces").exists() or (parent / ".git").exists():
            return parent
    return current

REPO_ROOT = find_repo_root()
WORKSPACES_ROOT = REPO_ROOT / "workspaces"

# ─── PURE-PYTHON YAML FRONTMATTER EXTRACTOR ──────────────────────────────────

def parse_yaml_frontmatter(content: str) -> tuple[dict, str]:
    if not content.startswith('---'):
        return {}, content
    parts = content.split('---', 2)
    if len(parts) < 3:
        return {}, content
    yaml_str = parts[1]
    body = parts[2].lstrip('\n')

    metadata: dict = {}
    lines = yaml_str.splitlines()
    i = 0
    while i < len(lines):
        line = lines[i]
        stripped = line.strip()
        if not stripped or stripped.startswith('#'):
            i += 1
            continue

        if stripped.endswith(':'):
            key = stripped[:-1].strip()
            i += 1
            list_items = []
            while i < len(lines) and (lines[i].startswith(' ') or lines[i].startswith('\t') or not lines[i].strip()):
                l = lines[i]
                if not l.strip():
                    i += 1
                    continue
                cleaned = l.strip()
                if cleaned.startswith('-'):
                    item: dict = {}
                    val = cleaned[1:].strip()
                    if ':' in val:
                        k, v = val.split(':', 1)
                        item[k.strip()] = v.strip().strip('"\'')
                    else:
                        item = val.strip('"\'')  # type: ignore
                    i += 1
                    while i < len(lines) and lines[i].startswith(' ') and not lines[i].strip().startswith('-'):
                        sub_line = lines[i].strip()
                        if sub_line and ':' in sub_line and isinstance(item, dict):
                            k, v = sub_line.split(':', 1)
                            item[k.strip()] = v.strip().strip('"\'')
                        i += 1
                    list_items.append(item)
                    continue
                else:
                    i += 1
            metadata[key] = list_items
            continue

        if ':' in line:
            key, val = line.split(':', 1)
            metadata[key.strip()] = val.strip().strip('"\'')
        i += 1

    return metadata, body

# ─── LINTING RULES ENGINE ───────────────────────────────────────────────────

PERIOD_REGEX = re.compile(r'^(\d{4}|\d{4}-\d{2}|\d{4}-Q[1-4])$')
SLUG_REGEX = re.compile(r'^[a-z0-9]+(-[a-z0-9]+)*$')

MISPLACED_EXTENSIONS = {'.xlsx', '.xls', '.csv', '.parquet', '.zip', '.tar', '.gz', '.pdf', '.docx', '.pptx'}
JUNK_PATTERNS = ['.tmp', '~', '.DS_Store', 'Thumbs.db']

class LintReport:
    def __init__(self, workspace_name: str):
        self.workspace_name = workspace_name
        self.violations: list[dict] = []
        self.warnings: list[dict] = []
        self.checked_items = 0

    def add_violation(self, rule_id: str, message: str, file_path: str = ""):
        self.violations.append({
            "workspace": self.workspace_name,
            "rule": rule_id,
            "message": message,
            "file": file_path,
            "level": "ERROR"
        })

    def add_warning(self, rule_id: str, message: str, file_path: str = ""):
        self.warnings.append({
            "workspace": self.workspace_name,
            "rule": rule_id,
            "message": message,
            "file": file_path,
            "level": "WARNING"
        })

    @property
    def is_clean(self) -> bool:
        return len(self.violations) == 0

def lint_workspace(ws_dir: Path) -> LintReport:
    ws_name = ws_dir.name
    report = LintReport(ws_name)
    today = datetime.date.today()

    knowledge_dir = ws_dir / "knowledge"
    if not knowledge_dir.exists():
        report.add_violation("KB001", f"Direktori knowledge/ tidak ditemukan di workspace '{ws_name}'", str(ws_dir))
        return report

    # ── Rule 1: Index Presence & Freshness
    index_file = knowledge_dir / "index.md"
    report.checked_items += 1
    if not index_file.exists():
        report.add_violation("KB002", "Berkas 'knowledge/index.md' wajib ada sebagai katalog acuan", str(index_file))
    else:
        index_mtime = index_file.stat().st_mtime
        # Check staleness against other knowledge files
        stale = False
        newest_file = ""
        for md_file in knowledge_dir.rglob("*.md"):
            if md_file.name == "index.md":
                continue
            if md_file.stat().st_mtime > index_mtime:
                stale = True
                newest_file = str(md_file.relative_to(ws_dir))
                break
        if stale:
            report.add_violation("KB003", f"Indeks 'knowledge/index.md' usang (outdated). Ada berkas lebih baru: {newest_file}", str(index_file))

    # ── Rule 2: Misplaced Files & Junk Cleanup
    for item in knowledge_dir.rglob("*"):
        if item.is_dir():
            continue
        rel_path = str(item.relative_to(ws_dir))
        report.checked_items += 1

        # Check junk file
        if any(item.name.endswith(pat) or item.name.startswith(pat) for pat in JUNK_PATTERNS):
            report.add_violation("KB004", f"Ditemukan berkas sampah/sementara: '{item.name}'", rel_path)

        # Check binary file placed outside data/
        ext = item.suffix.lower()
        if ext in MISPLACED_EXTENSIONS:
            # check if it is under a data/ folder
            if "data" not in item.parts:
                report.add_violation("KB005", f"Berkas biner/tabular '{item.name}' tersimpan di luar folder 'data/'", rel_path)

    # ── Rule 3: Activities & Periods Format
    kegiatan_dirs = [knowledge_dir / "kegiatan", knowledge_dir / "activities", ws_dir / "kegiatan"]
    found_activities_dir = False

    for k_base in kegiatan_dirs:
        if not k_base.exists():
            continue
        found_activities_dir = True

        for act_path in k_base.iterdir():
            if not act_path.is_dir() or act_path.name.startswith("."):
                continue

            slug = act_path.name
            report.checked_items += 1
            if not SLUG_REGEX.match(slug):
                report.add_violation("KB006", f"Nama folder kegiatan '{slug}' harus format kebab-case lowercase", str(act_path.relative_to(ws_dir)))

            # Check period subdirectories
            for period_path in act_path.iterdir():
                if not period_path.is_dir() or period_path.name.startswith("."):
                    continue

                period_name = period_path.name
                report.checked_items += 1
                if not PERIOD_REGEX.match(period_name):
                    report.add_violation("KB007", f"Format periode '{period_name}' tidak baku. Gunakan YYYY-MM, YYYY-QX, atau YYYY", str(period_path.relative_to(ws_dir)))

                # Check README.md
                readme_path = period_path / "README.md"
                report.checked_items += 1
                if not readme_path.exists():
                    report.add_violation("KB008", f"Berkas 'README.md' tidak ditemukan pada kegiatan periode '{period_name}'", str(period_path.relative_to(ws_dir)))
                    continue

                # Parse Frontmatter
                content = readme_path.read_text(encoding="utf-8")
                metadata, body = parse_yaml_frontmatter(content)

                if not metadata:
                    report.add_violation("KB009", "Frontmatter YAML tidak ditemukan atau tidak valid", str(readme_path.relative_to(ws_dir)))
                    continue

                # Mandatory fields
                for req_field in ["nama", "kategori", "status"]:
                    if req_field not in metadata or not metadata[req_field]:
                        report.add_violation("KB010", f"Field wajib '{req_field}' kosong pada frontmatter", str(readme_path.relative_to(ws_dir)))

                # Valid status value
                status_val = str(metadata.get("status", "")).lower()
                if status_val not in ["aktif", "selesai"]:
                    report.add_violation("KB011", f"Nilai status '{status_val}' tidak legal. Hanya boleh 'aktif' atau 'selesai'", str(readme_path.relative_to(ws_dir)))

                # Check deadlines formatting and overdue status
                deadlines = metadata.get("deadlines", [])
                if isinstance(deadlines, list):
                    for dl in deadlines:
                        if not isinstance(dl, dict):
                            continue
                        tgl_str = dl.get("tanggal", "")
                        dl_status = dl.get("status", "belum").lower()
                        if tgl_str:
                            try:
                                dl_date = datetime.datetime.strptime(tgl_str, "%Y-%m-%d").date()
                                if dl_date < today and dl_status != "selesai":
                                    # Overdue warning
                                    report.add_warning("KB012", f"Deadline '{dl.get('kegiatan', '')}' telah lewat ({tgl_str}) dan status masih '{dl_status}'", str(readme_path.relative_to(ws_dir)))
                            except ValueError:
                                report.add_violation("KB013", f"Format tanggal deadline '{tgl_str}' salah. Gunakan YYYY-MM-DD", str(readme_path.relative_to(ws_dir)))

    return report

# ─── CLI DISPATCHER & AUTO-HEALING ───────────────────────────────────────────

def run_linter(workspaces: list[Path], auto_heal: bool = False) -> tuple[int, list[LintReport]]:
    total_violations = 0
    total_warnings = 0
    reports = []

    print(f"\n🔍 === AINA KNOWLEDGE BASE DETERMINISTIC LINTER ===")
    print(f"Timestamp: {datetime.datetime.now().strftime('%Y-%m-%d %H:%M:%S WIB')} | Target: {len(workspaces)} workspace\n")

    for ws_dir in workspaces:
        rep = lint_workspace(ws_dir)
        reports.append(rep)
        total_violations += len(rep.violations)
        total_warnings += len(rep.warnings)

        status_badge = "✅ CLEAN" if rep.is_clean else "❌ VIOLATIONS FOUND"
        print(f"📁 Workspace: {rep.workspace_name:<16} [{status_badge}] ({rep.checked_items} item diperiksa)")

        for v in rep.violations:
            loc = f" [{v['file']}]" if v['file'] else ""
            print(f"   🚨 [{v['rule']}] {v['message']}{loc}")

        for w in rep.warnings:
            loc = f" [{w['file']}]" if w['file'] else ""
            print(f"   ⚠️  [{w['rule']}] {w['message']}{loc}")
        print("-" * 75)

    print(f"\n📊 HASIL AKHIR: {total_violations} Pelanggaran Kerapian | {total_warnings} Peringatan")

    # Closed-Loop Auto-Healing Trigger
    if total_violations > 0 and auto_heal:
        print("\n🔧 Memicu Closed-Loop Auto-Healing via Grooming Engine...")
        ws_mgr_script = REPO_ROOT / "scripts" / "workspace_manager.py"
        if ws_mgr_script.exists():
            import subprocess
            for rep in reports:
                if not rep.is_clean:
                    # Run groom
                    print(f"   ⚡ Menjalankan grooming otomatis untuk '{rep.workspace_name}'...")
                    subprocess.run([sys.executable, str(ws_mgr_script), "groom", rep.workspace_name], check=False)

            # Re-linting for Closed-Loop Verification
            print("\n🔄 Memverifikasi Ulang Setelah Grooming (Closed-Loop Verification)...")
            recheck_violations = 0
            for ws_dir in workspaces:
                re_rep = lint_workspace(ws_dir)
                recheck_violations += len(re_rep.violations)
                if re_rep.is_clean:
                    print(f"   ✅ Workspace '{re_rep.workspace_name}' sekarang SUDAH BERSIH & RAPI!")
                else:
                    print(f"   ⚠️ Workspace '{re_rep.workspace_name}' masih memiliki {len(re_rep.violations)} isu yang butuh intervensi.")

            if recheck_violations == 0:
                print("\n🎉 CLOSED-LOOP AUTO-HEALING BERHASIL! Seluruh workspace kini 100% aman.")
                return 0, reports
            else:
                print(f"\n⚠️ Masih tersisa {recheck_violations} pelanggaran yang memerlukan penanganan manual.")
                return 1, reports

    exit_code = 0 if total_violations == 0 else 1
    return exit_code, reports

def main():
    parser = argparse.ArgumentParser(description="Aina KB Deterministic Linter")
    parser.add_argument("workspace", nargs="?", help="Nama workspace tertentu (opsional, default: semua)")
    parser.add_argument("--auto-heal", action="store_true", help="Otomatis picu grooming jika ditemukan ketidakrapian")
    parser.add_argument("--json", action="store_true", help="Output hasil dalam format JSON")

    args = parser.parse_args()

    target_dirs = []
    if args.workspace:
        ws_path = WORKSPACES_ROOT / args.workspace.strip().lower()
        if not ws_path.exists():
            print(f"❌ Workspace '{args.workspace}' tidak ditemukan.")
            sys.exit(1)
        target_dirs.append(ws_path)
    else:
        if WORKSPACES_ROOT.exists():
            target_dirs = [p for p in WORKSPACES_ROOT.iterdir() if p.is_dir() and not p.name.startswith(".")]

    if not target_dirs:
        print("ℹ️ Tidak ada workspace yang ditemukan.")
        sys.exit(0)

    exit_code, reports = run_linter(target_dirs, auto_heal=args.auto_heal)

    if args.json:
        all_data = {
            "timestamp": datetime.datetime.now().isoformat(),
            "exit_code": exit_code,
            "workspaces": [
                {
                    "name": r.workspace_name,
                    "clean": r.is_clean,
                    "violations": r.violations,
                    "warnings": r.warnings
                }
                for r in reports
            ]
        }
        print(json.dumps(all_data, indent=2))

    sys.exit(exit_code)

if __name__ == "__main__":
    main()
