#!/usr/bin/env python3
"""
Aina Workspace & Knowledge Base Manager
======================================
Mengelola struktur workspace yang agnostik dan mendukung siklus pengetahuan terstruktur:
1. Workspace Management (list, create, groom)
2. Activity & Project Lifecycle (create-activity, list-activities)
3. Deterministic Deadline & Schedule Tracker (schedule --week/--month/--overdue)
4. Multi-tiered Knowledge Grooming (Progressive Disclosure index.md)
"""

import sys
import os
import re
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

# ─── PURE-PYTHON YAML FRONTMATTER PARSER & SERIALIZER ────────────────────────

def slugify(text: str) -> str:
    text = text.strip().lower()
    text = re.sub(r'[\s_]+', '-', text)
    text = re.sub(r'[^a-z0-9-]', '', text)
    return text.strip('-')

def parse_yaml_frontmatter(content: str) -> tuple[dict, str]:
    """
    Ekstrak YAML frontmatter sederhana (key-value + list of dicts/strings) dan body markdown.
    Zero external dependencies (pure Python).
    """
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

        # List key (misal "deadlines:")
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

        # Standard key-value
        if ':' in line:
            key, val = line.split(':', 1)
            metadata[key.strip()] = val.strip().strip('"\'')
        i += 1

    return metadata, body

def dump_yaml_frontmatter(metadata: dict, body: str) -> str:
    """Serialisasi dict metadata dan body markdown menjadi berkas frontmatter terstandarisasi."""
    lines = ["---"]
    for k, v in metadata.items():
        if isinstance(v, list):
            lines.append(f"{k}:")
            for item in v:
                if isinstance(item, dict):
                    keys = list(item.keys())
                    if keys:
                        first_key = keys[0]
                        lines.append(f'  - {first_key}: "{item[first_key]}"')
                        for sub_k in keys[1:]:
                            lines.append(f'    {sub_k}: "{item[sub_k]}"')
                else:
                    lines.append(f'  - "{item}"')
        else:
            lines.append(f'{k}: "{v}"')
    lines.append("---")
    lines.append("")
    lines.append(body.strip())
    lines.append("")
    return "\n".join(lines)

# ─── WORKSPACE SUBCOMMANDS ───────────────────────────────────────────────────

def get_workspace_dir(ws_name: str) -> Path:
    ws_dir = WORKSPACES_ROOT / ws_name.strip().lower()
    return ws_dir

def cmd_list(args):
    if not WORKSPACES_ROOT.exists():
        print("Direktori workspaces/ belum dibuat.")
        return

    workspaces = [p for p in WORKSPACES_ROOT.iterdir() if p.is_dir() and not p.name.startswith(".")]
    print(f"\n📁 DAFTAR WORKSPACE TERDAFTAR ({len(workspaces)}):")
    divider = "-" * 85
    print(divider)
    print(f"| {'Workspace':<16} | {'Kegiatan':<10} | {'Dokumen':<9} | {'Data':<6} | {'Deskripsi':<32} |")
    print(divider)

    for ws in sorted(workspaces):
        gemini_file = ws / "GEMINI.md"
        desc = "Tanpa deskripsi"
        if gemini_file.exists():
            for line in gemini_file.read_text(encoding="utf-8").splitlines():
                if line.startswith("# Workspace") or line.startswith("# Domain"):
                    desc = line.lstrip("#").strip()
                    break

        knowledge_dir = ws / "knowledge"
        knowledge_count = len(list(knowledge_dir.glob("*.md"))) if knowledge_dir.exists() else 0
        
        # Scan activities
        activities = scan_activities(ws)
        data_count = len(list((ws / "data").iterdir())) if (ws / "data").exists() else 0

        print(f"| {ws.name:<16} | {len(activities):<10} | {knowledge_count:<9} | {data_count:<6} | {desc[:32]:<32} |")
    print(divider)

def cmd_create(args):
    name = slugify(args.name)
    ws_dir = WORKSPACES_ROOT / name
    if ws_dir.exists():
        print(f"⚠️ Workspace '{name}' sudah ada di {ws_dir}")
        return

    knowledge_dir = ws_dir / "knowledge"
    kegiatan_dir = knowledge_dir / "kegiatan"
    data_dir = ws_dir / "data"
    scripts_dir = ws_dir / "scripts"

    knowledge_dir.mkdir(parents=True, exist_ok=True)
    kegiatan_dir.mkdir(parents=True, exist_ok=True)
    data_dir.mkdir(parents=True, exist_ok=True)
    scripts_dir.mkdir(parents=True, exist_ok=True)

    (data_dir / ".gitkeep").touch()
    (scripts_dir / ".gitkeep").touch()
    (kegiatan_dir / ".gitkeep").touch()

    title = args.title or f"Domain {name.capitalize()}"
    domain_desc = args.domain or f"Workspace terdedikasi untuk proyek dan domain {name}."
    now_str = datetime.datetime.now().strftime("%Y-%m-%d %H:%M WIB")

    gemini_content = f"""# Workspace: {title}

> [!IMPORTANT]
> Workspace ini diinisialisasi otomatis untuk domain **{name}**.
> Dibuat: {now_str}

---

## 1. Fokus & Lingkup Domain
- **Deskripsi**: {domain_desc}
- **Peran Aina**: Mengelola basis pengetahuan, memproses data, dan menjalankan otomasi spesifik domain {name}.

---

## 2. Struktur Knowledge Base
- `knowledge/index.md`: Ringkasan dan indeks seluruh topik penting dalam domain ini.
- `knowledge/facts.md`: Fakta, keputusan diskusi, parameter teknis, dan catatan penting dari percakapan.
- `knowledge/procedures.md`: Standar Operasional Prosedur (SOP), alur kerja, dan instruksi rutin.
- `knowledge/kegiatan/`: Arsip kegiatan/proyek terstruktur berdasarkan waktu (`kegiatan/<nama>/<periode>/README.md`).
- `data/`: Penyimpanan berkas data mentah (.xlsx, .csv, .json, .pdf).
- `scripts/`: Skrip automasi dan pipeline pemrosesan data.

---

## 3. Tata Kelola Data & Pembaruan
1. Simpan fakta terverifikasi ke dalam `knowledge/facts.md`.
2. Jika ada SOP baru dari rekan kerja, dokumentasikan ke `knowledge/procedures.md`.
3. Untuk proyek/kegiatan terikat waktu, buat sub-kegiatan via `python3 scripts/workspace_manager.py create-activity {name} "<nama>" "<periode>"`.
4. Jalankan `python3 scripts/workspace_manager.py groom {name}` secara berkala untuk memperbarui indeks dan agenda.
"""
    (ws_dir / "GEMINI.md").write_text(gemini_content, encoding="utf-8")

    index_content = f"""# 📚 Indeks Knowledge Base: {title}

Dokumen ini adalah ringkasan terstruktur dari seluruh pengetahuan di workspace `{name}`.

## Daftar Berkas Pengetahuan
- [`facts.md`](facts.md) : Kumpulan fakta terverifikasi dan parameter domain.
- [`procedures.md`](procedures.md) : Alur kerja, SOP, dan panduan langkah demi langkah.

## Ringkasan Eksekutif
Workspace baru saja diinisialisasi. Belum ada catatan tambahan.
"""
    (knowledge_dir / "index.md").write_text(index_content, encoding="utf-8")

    facts_content = f"""# 💡 Fakta & Catatan Kunci: {title}

*Dokumentasikan fakta penting, keputusan rapat, atau data acuan di sini.*

- **Inisialisasi**: Workspace `{name}` resmi dibentuk pada {now_str}.
"""
    (knowledge_dir / "facts.md").write_text(facts_content, encoding="utf-8")

    procedures_content = f"""# 📋 Prosedur & SOP: {title}

*Dokumentasikan langkah-langkah kerja atau skrip rutin di sini.*

1. **Pengumpulan Data**: Simpan file input di folder `data/`.
2. **Pemrosesan**: Gunakan skrip di folder `scripts/`.
"""
    (knowledge_dir / "procedures.md").write_text(procedures_content, encoding="utf-8")

    # Copy utility scripts
    model_ctrl = REPO_ROOT / "workspaces" / "default" / "scripts" / "model_control.py"
    if model_ctrl.exists():
        (scripts_dir / "model_control.py").write_text(model_ctrl.read_text(encoding="utf-8"), encoding="utf-8")
        os.chmod(scripts_dir / "model_control.py", 0o755)

    ws_mgr = REPO_ROOT / "scripts" / "workspace_manager.py"
    if ws_mgr.exists():
        (scripts_dir / "workspace_manager.py").write_text(ws_mgr.read_text(encoding="utf-8"), encoding="utf-8")
        os.chmod(scripts_dir / "workspace_manager.py", 0o755)

    print(f"✅ Berhasil membuat workspace baru: '{name}' di {ws_dir}")
    print(f"   • Rules payung domain: {ws_dir}/GEMINI.md")
    print(f"   • Knowledge base: {knowledge_dir}/")
    print(f"   • Sub-kegiatan: {kegiatan_dir}/")
    print(f"   • Data store: {data_dir}/")
    print(f"   • Scripts folder: {scripts_dir}/")

# ─── ACTIVITY & TIMELINE MANAGEMENT ──────────────────────────────────────────

def scan_activities(ws_dir: Path) -> list[dict]:
    """Memindai seluruh kegiatan di bawah knowledge/kegiatan/ atau knowledge/activities/."""
    activities = []
    candidates = [
        ws_dir / "knowledge" / "kegiatan",
        ws_dir / "knowledge" / "activities",
        ws_dir / "kegiatan",
    ]

    for base in candidates:
        if not base.exists():
            continue
        for readme in base.glob("*/*/README.md"):
            periode = readme.parent.name
            slug_name = readme.parent.parent.name
            content = readme.read_text(encoding="utf-8")
            metadata, body = parse_yaml_frontmatter(content)
            activities.append({
                "workspace": ws_dir.name,
                "path": readme,
                "rel_path": readme.relative_to(ws_dir / "knowledge") if (ws_dir / "knowledge") in readme.parents else readme.name,
                "slug": slug_name,
                "periode": periode,
                "nama": metadata.get("nama", slug_name.replace('-', ' ').title()),
                "kategori": metadata.get("kategori", "umum"),
                "rutinitas": metadata.get("rutinitas", "rutin"),
                "frekuensi": metadata.get("frekuensi", "bulanan"),
                "peran": metadata.get("peran", "anggota"),
                "status": metadata.get("status", "aktif"),
                "deadlines": metadata.get("deadlines", []),
                "metadata": metadata,
                "body": body,
            })
    return activities

def cmd_create_activity(args):
    ws_name = slugify(args.workspace)
    ws_dir = get_workspace_dir(ws_name)
    if not ws_dir.exists():
        print(f"❌ Workspace '{ws_name}' tidak ditemukan. Buat workspace terlebih dahulu dengan 'create'.")
        return

    activity_slug = slugify(args.nama)
    periode = args.periode.strip()
    act_dir = ws_dir / "knowledge" / "kegiatan" / activity_slug / periode
    act_dir.mkdir(parents=True, exist_ok=True)
    readme_path = act_dir / "README.md"

    if readme_path.exists() and not args.force:
        print(f"⚠️ Kegiatan '{args.nama}' ({periode}) sudah ada di {readme_path}. Gunakan --force untuk menimpa.")
        return

    # Prepare default deadline
    today_str = datetime.date.today().strftime("%Y-%m-%d")
    deadlines = []
    if args.deadline:
        for dl in args.deadline:
            if ":" in dl:
                tgl, label = dl.split(":", 1)
                deadlines.append({"tanggal": tgl.strip(), "kegiatan": label.strip(), "status": "belum"})
            else:
                deadlines.append({"tanggal": today_str, "kegiatan": dl.strip(), "status": "belum"})
    else:
        deadlines.append({
            "tanggal": today_str,
            "kegiatan": "Kick-off & Persiapan Awal",
            "status": "belum"
        })

    metadata = {
        "nama": args.nama.strip(),
        "kategori": args.kategori,
        "rutinitas": args.rutinitas,
        "frekuensi": args.frekuensi,
        "peran": args.peran,
        "status": args.status,
        "deadlines": deadlines,
    }

    body = f"""# {args.nama.strip()} ({periode})

## 🎯 Deskripsi Kegiatan
Dokumentasikan target, latar belakang, dan cakupan kegiatan di sini.

## 📌 Catatan Pelaksanaan & Dinamika Chat
- Catat perkembangan, hasil koordinasi, dan kendala operasional di sini.
"""

    content = dump_yaml_frontmatter(metadata, body)
    readme_path.write_text(content, encoding="utf-8")
    print(f"✅ Berhasil membuat kegiatan: '{args.nama}' ({periode})")
    print(f"   • Lokasi: {readme_path}")
    print(f"   • Status: {args.status.upper()} | Kategori: {args.kategori}")

def cmd_schedule(args):
    """Menampilkan timeline dan deadline jadwal kegiatan deterministik."""
    workspaces_to_scan = []
    if args.workspace:
        ws_dir = get_workspace_dir(args.workspace)
        if not ws_dir.exists():
            print(f"❌ Workspace '{args.workspace}' tidak ditemukan.")
            return
        workspaces_to_scan.append(ws_dir)
    else:
        workspaces_to_scan = [p for p in WORKSPACES_ROOT.iterdir() if p.is_dir() and not p.name.startswith(".")]

    all_deadlines = []
    today = datetime.date.today()

    for ws in workspaces_to_scan:
        acts = scan_activities(ws)
        for act in acts:
            for dl in act.get("deadlines", []):
                if not isinstance(dl, dict):
                    continue
                tgl_str = dl.get("tanggal", "")
                if not tgl_str:
                    continue
                try:
                    tgl = datetime.datetime.strptime(tgl_str, "%Y-%m-%d").date()
                except ValueError:
                    continue
                all_deadlines.append({
                    "workspace": act["workspace"],
                    "nama_kegiatan": act["nama"],
                    "periode": act["periode"],
                    "tanggal": tgl,
                    "kegiatan": dl.get("kegiatan", "Kegiatan tanpa judul"),
                    "status": dl.get("status", "belum").lower(),
                })

    if not all_deadlines:
        print("ℹ️ Tidak ada deadline atau jadwal kegiatan yang tercatat.")
        return

    all_deadlines.sort(key=lambda x: (x["tanggal"], x["nama_kegiatan"]))

    # Date range filters
    weekday = today.weekday()
    start_of_week = today - datetime.timedelta(days=weekday)
    end_of_week = start_of_week + datetime.timedelta(days=6)
    start_of_month = today.replace(day=1)
    if start_of_month.month == 12:
        end_of_month = today.replace(year=today.year + 1, month=1, day=1) - datetime.timedelta(days=1)
    else:
        end_of_month = today.replace(month=today.month + 1, day=1) - datetime.timedelta(days=1)

    filtered = all_deadlines
    title_text = "SEMUA JADWAL & DEADLINE KEGIATAN"

    if args.week:
        filtered = [d for d in all_deadlines if start_of_week <= d["tanggal"] <= end_of_week]
        title_text = f"DEADLINE MINGGU INI ({start_of_week.strftime('%d %b %Y')} s.d. {end_of_week.strftime('%d %b %Y')})"
    elif args.month:
        filtered = [d for d in all_deadlines if start_of_month <= d["tanggal"] <= end_of_month]
        title_text = f"DEADLINE BULAN INI ({start_of_month.strftime('%B %Y')})"
    elif args.overdue:
        filtered = [d for d in all_deadlines if d["tanggal"] < today and d["status"] != "selesai"]
        title_text = "DEADLINE OVERDUE (TERLEWAT & BELUM SELESAI)"

    print(f"\n📅 === {title_text} ===")
    print(f"Hari ini: {today.strftime('%A, %d %B %Y')} | Total: {len(filtered)} jadwal")

    if not filtered:
        print("✅ Tidak ada agenda yang tertunda pada filter yang dipilih.\n")
        return

    _DAY_ID = {
        "Monday": "Senin", "Tuesday": "Selasa", "Wednesday": "Rabu",
        "Thursday": "Kamis", "Friday": "Jumat", "Saturday": "Sabtu", "Sunday": "Minggu"
    }

    current_date = None
    for item in filtered:
        if item["tanggal"] != current_date:
            current_date = item["tanggal"]
            day_name = _DAY_ID.get(current_date.strftime("%A"), current_date.strftime("%A"))
            delta = (current_date - today).days
            if delta == 0:
                rel_badge = "[HARI INI]"
            elif delta > 0:
                rel_badge = f"[{delta} hari lagi]"
            else:
                rel_badge = f"[Terlewat {-delta} hari]"

            print(f"\n📌 {day_name.upper()}, {current_date.strftime('%d %b %Y')} {rel_badge}:")

        is_done = item["status"] == "selesai"
        is_overdue = item["tanggal"] < today and not is_done

        if is_done:
            status_box = "✓ [SELESAI]"
        elif is_overdue:
            status_box = "⚠️ [OVERDUE]"
        else:
            status_box = "⏳ [BELUM]"

        print(f"   {status_box:<12} {item['kegiatan']} ({item['nama_kegiatan']} - {item['periode']}) [{item['workspace']}]")
    print("")

# ─── ENHANCED KNOWLEDGE BASE GROOMING ────────────────────────────────────────

def cmd_groom(args):
    ws_name = slugify(args.name)
    ws_dir = get_workspace_dir(ws_name)
    if not ws_dir.exists():
        print(f"❌ Workspace '{ws_name}' tidak ditemukan di {WORKSPACES_ROOT}")
        return

    knowledge_dir = ws_dir / "knowledge"
    if not knowledge_dir.exists():
        print(f"❌ Folder knowledge/ tidak ditemukan di workspace '{ws_name}'")
        return

    print(f"🧹 Merapikan (grooming) knowledge base pada workspace '{ws_name}'...")
    now = datetime.datetime.now()
    now_str = now.strftime("%Y-%m-%d %H:%M WIB")
    today = now.date()

    # 1. Scan General Flat Knowledge Files
    flat_files = [f for f in knowledge_dir.glob("*.md") if f.name != "index.md"]
    flat_summaries = []
    total_flat_lines = 0
    for f in sorted(flat_files):
        lines = f.read_text(encoding="utf-8").splitlines()
        total_flat_lines += len(lines)
        title = lines[0].lstrip("#").strip() if lines else f.name
        flat_summaries.append((f.name, title, len(lines)))

    # 2. Scan Activities & Deadlines
    activities = scan_activities(ws_dir)
    total_deadlines = 0
    pending_deadlines = []

    for act in activities:
        for dl in act.get("deadlines", []):
            total_deadlines += 1
            if isinstance(dl, dict):
                tgl_str = dl.get("tanggal", "")
                status = dl.get("status", "belum").lower()
                if status != "selesai" and tgl_str:
                    try:
                        tgl = datetime.datetime.strptime(tgl_str, "%Y-%m-%d").date()
                        pending_deadlines.append({
                            "kegiatan": dl.get("kegiatan", ""),
                            "nama_kegiatan": act["nama"],
                            "periode": act["periode"],
                            "tanggal": tgl,
                            "rel_path": act["rel_path"],
                        })
                    except ValueError:
                        pass

    pending_deadlines.sort(key=lambda x: x["tanggal"])

    # 3. Compile Progressive Disclosure index.md
    index_lines = [
        f"# 📚 Indeks Terpadu Knowledge Base: {ws_name.upper()}",
        "",
        f"> Terakhir dirapikan: **{now_str}** | Berkas Dokumen: **{len(flat_files)}** | Kegiatan Aktif: **{len(activities)}** | Agenda Tertunda: **{len(pending_deadlines)}**",
        "",
        "---",
        "",
        "## 📑 1. Pengetahuan Umum & Pedoman Dasar (Universal Knowledge)",
        ""
    ]

    if flat_summaries:
        for fname, ftitle, flen in flat_summaries:
            index_lines.append(f"- [**{ftitle}**]({fname}) — `{flen} baris`")
    else:
        index_lines.append("- *(Belum ada berkas pengetahuan umum tambahan)*")

    index_lines.extend([
        "",
        "---",
        "",
        "## 🗓️ 2. Matriks Kegiatan & Proyek Berdasarkan Waktu",
        ""
    ])

    if activities:
        index_lines.append("| Nama Kegiatan | Periode | Kategori | Status | Berkas Rujukan |")
        index_lines.append("| :--- | :--- | :--- | :--- | :--- |")
        for act in sorted(activities, key=lambda x: (x["periode"], x["nama"])):
            status_badge = "🟢 Selesai" if act["status"].lower() == "selesai" else "🟡 Aktif"
            index_lines.append(f"| **{act['nama']}** | `{act['periode']}` | {act['kategori']} | {status_badge} | [`README.md`]({act['rel_path']}) |")
    else:
        index_lines.append("- *(Belum ada kegiatan/survei berkala yang diarsipkan)*")

    index_lines.extend([
        "",
        "---",
        "",
        "## ⏰ 3. Sorotan Agenda & Deadline Terdekat",
        ""
    ])

    if pending_deadlines:
        for item in pending_deadlines[:5]:
            delta = (item["tanggal"] - today).days
            if delta == 0:
                rel_str = "**HARI INI** 🚨"
            elif delta < 0:
                rel_str = f"**TERLEWAT {-delta} HARI** ⚠️"
            else:
                rel_str = f"{delta} hari lagi"
            index_lines.append(f"- **{item['tanggal'].strftime('%d %b %Y')}** ({rel_str}): {item['kegiatan']} — *{item['nama_kegiatan']} ({item['periode']})* [`Rujukan`]({item['rel_path']})")
    else:
        index_lines.append("✅ *Semua tenggat waktu kegiatan telah diselesaikan!*")

    index_lines.extend([
        "",
        "---",
        "",
        "## 🔍 Panduan Pengambilan Pengetahuan (Progressive Retrieval)",
        "1. **Peta Konteks Awal**: Asisten AI membaca berkas `index.md` ini di awal sesi untuk memetakan dokumen umum, kegiatan yang sedang berjalan, dan tenggat waktu terdekat.",
        "2. **Penyelaman Konteks Spesifik**: Saat pengguna menanyakan detail SOP atau kegiatan tertentu, agen **hanya** membuka berkas target (misal `kegiatan/<slug>/<periode>/README.md`) tanpa membaca seluruh repositori, sehingga menghemat konsumsi token dan menjaga kecepatan berpikir (5–15 detik).",
        ""
    ])

    (knowledge_dir / "index.md").write_text("\n".join(index_lines), encoding="utf-8")
    print(f"✅ Indeks knowledge base '{ws_name}' berhasil diperbarui di {knowledge_dir / 'index.md'}")
    print(f"   • {len(flat_files)} dokumen umum dirangkum")
    print(f"   • {len(activities)} kegiatan terpetakan")
    print(f"   • {len(pending_deadlines)} agenda deadline tersinkronisasi")

# ─── MAIN CLI DISPATCHER ─────────────────────────────────────────────────────

def main():
    parser = argparse.ArgumentParser(description="Aina Workspace & Knowledge Base Manager")
    subparsers = parser.add_subparsers(dest="subcommand", help="Perintah manajemen")

    # list
    p_list = subparsers.add_parser("list", help="Daftar seluruh workspace")

    # create
    p_create = subparsers.add_parser("create", help="Buat workspace baru")
    p_create.add_argument("name", help="Nama folder workspace (contoh: bps, riset-ai, devops)")
    p_create.add_argument("--title", help="Judul lengkap workspace")
    p_create.add_argument("--domain", help="Deskripsi domain fokus workspace")

    # create-activity
    p_act = subparsers.add_parser("create-activity", aliases=["activity"], help="Buat kegiatan baru dalam workspace")
    p_act.add_argument("workspace", help="Nama workspace target")
    p_act.add_argument("nama", help="Nama kegiatan (contoh: 'Sakernas Agustus', 'Audit Server')")
    p_act.add_argument("periode", help="Periode waktu (contoh: '2026-08', '2026-Q3', '2026')")
    p_act.add_argument("--kategori", default="umum", help="Kategori kegiatan (survey, sensus, devops, dll)")
    p_act.add_argument("--rutinitas", default="rutin", choices=["rutin", "non-rutin"], help="Sifat kegiatan")
    p_act.add_argument("--frekuensi", default="bulanan", choices=["bulanan", "triwulanan", "semesteran", "tahunan", "ad-hoc"], help="Frekuensi kegiatan")
    p_act.add_argument("--peran", default="anggota", choices=["ketua", "anggota", "admin"], help="Peran penanggung jawab")
    p_act.add_argument("--status", default="aktif", choices=["aktif", "selesai"], help="Status kegiatan")
    p_act.add_argument("--deadline", action="append", help="Tambah deadline awal format: YYYY-MM-DD:Keterangan")
    p_act.add_argument("--force", action="store_true", help="Timpa file jika sudah ada")

    # schedule
    p_sched = subparsers.add_parser("schedule", help="Tampilkan agenda dan deadline kegiatan")
    p_sched.add_argument("workspace", nargs="?", help="Nama workspace tertentu (opsional)")
    p_sched.add_argument("--week", action="store_true", help="Hanya deadline minggu ini")
    p_sched.add_argument("--month", action="store_true", help="Hanya deadline bulan ini")
    p_sched.add_argument("--overdue", action="store_true", help="Hanya deadline yang sudah lewat")

    # groom
    p_groom = subparsers.add_parser("groom", help="Rapikan (groom) knowledge base di workspace")
    p_groom.add_argument("name", help="Nama workspace yang akan dirapikan")

    args = parser.parse_args()

    if args.subcommand == "list":
        cmd_list(args)
    elif args.subcommand == "create":
        cmd_create(args)
    elif args.subcommand in ["create-activity", "activity"]:
        cmd_create_activity(args)
    elif args.subcommand == "schedule":
        cmd_schedule(args)
    elif args.subcommand == "groom":
        cmd_groom(args)
    else:
        parser.print_help()
        sys.exit(1)

if __name__ == "__main__":
    main()
