#!/usr/bin/env python3
"""
Aina Workspace & Knowledge Base Manager
Memungkinkan Aina dan pengguna membuat, mengelola, dan merapikan (grooming)
struktur workspace dan knowledge base secara modular dan terisolasi.
"""

import sys
import os
import argparse
import datetime
from pathlib import Path

def find_repo_root():
    current = Path.cwd()
    for parent in [current] + list(current.parents):
        if (parent / "workspaces").exists() or (parent / ".git").exists():
            return parent
    return current

REPO_ROOT = find_repo_root()
WORKSPACES_ROOT = REPO_ROOT / "workspaces"

def cmd_list(args):
    if not WORKSPACES_ROOT.exists():
        print("Direktori workspaces/ belum dibuat.")
        return

    workspaces = [p for p in WORKSPACES_ROOT.iterdir() if p.is_dir() and not p.name.startswith(".")]
    print(f"📁 Daftar Workspace Terdaftar ({len(workspaces)}):")
    for ws in sorted(workspaces):
        gemini_file = ws / "GEMINI.md"
        desc = "Tanpa deskripsi"
        if gemini_file.exists():
            for line in gemini_file.read_text().splitlines():
                if line.startswith("# Workspace") or line.startswith("# Domain"):
                    desc = line.lstrip("#").strip()
                    break

        knowledge_count = len(list((ws / "knowledge").glob("*.md"))) if (ws / "knowledge").exists() else 0
        data_count = len(list((ws / "data").iterdir())) if (ws / "data").exists() else 0
        print(f"- {ws.name}/ : {desc} (Catatan: {knowledge_count}, Berkas data: {data_count})")

def cmd_create(args):
    name = args.name.strip().lower().replace(" ", "-")
    ws_dir = WORKSPACES_ROOT / name
    if ws_dir.exists():
        print(f"⚠️ Workspace '{name}' sudah ada di {ws_dir}")
        return

    # Create directory structure
    knowledge_dir = ws_dir / "knowledge"
    data_dir = ws_dir / "data"
    scripts_dir = ws_dir / "scripts"

    knowledge_dir.mkdir(parents=True, exist_ok=True)
    data_dir.mkdir(parents=True, exist_ok=True)
    scripts_dir.mkdir(parents=True, exist_ok=True)

    # Touch .gitkeep files
    (data_dir / ".gitkeep").touch()
    (scripts_dir / ".gitkeep").touch()

    # Generate domain GEMINI.md
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
- `data/`: Penyimpanan berkas data (.xlsx, .csv, .json, .pdf).
- `scripts/`: Skrip automasi dan pipeline pemrosesan data.

---

## 3. Tata Kelola Data & Pembaruan
1. Simpan fakta terverifikasi ke dalam `knowledge/facts.md`.
2. Jika ada SOP baru dari rekan kerja, dokumentasikan ke `knowledge/procedures.md`.
3. Jalankan `python3 scripts/workspace_manager.py groom {name}` secara berkala untuk merapikan indeks pengetahuan.
"""
    (ws_dir / "GEMINI.md").write_text(gemini_content, encoding="utf-8")

    # Generate initial knowledge files
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
    print(f"   • Data store: {data_dir}/")
    print(f"   • Scripts folder: {scripts_dir}/")

def cmd_groom(args):
    name = args.name.strip().lower()
    ws_dir = WORKSPACES_ROOT / name
    if not ws_dir.exists():
        print(f"❌ Workspace '{name}' tidak ditemukan di {WORKSPACES_ROOT}")
        return

    knowledge_dir = ws_dir / "knowledge"
    if not knowledge_dir.exists():
        print(f"❌ Folder knowledge/ tidak ditemukan di workspace '{name}'")
        return

    print(f"🧹 Merapikan (grooming) knowledge base pada workspace '{name}'...")
    now_str = datetime.datetime.now().strftime("%Y-%m-%d %H:%M WIB")

    md_files = [f for f in knowledge_dir.glob("*.md") if f.name != "index.md"]
    
    file_summaries = []
    total_lines = 0
    for f in sorted(md_files):
        lines = f.read_text(encoding="utf-8").splitlines()
        total_lines += len(lines)
        title = lines[0].lstrip("#").strip() if lines else f.name
        file_summaries.append((f.name, title, len(lines)))

    index_lines = [
        f"# 📚 Indeks Knowledge Base: {name.upper()}",
        "",
        f"> Terakhir dirapikan: {now_str} | Total berkas: {len(md_files)} | Total baris: {total_lines}",
        "",
        "## 📑 Katalog Berkas Pengetahuan",
        ""
    ]

    for fname, ftitle, flen in file_summaries:
        index_lines.append(f"- [**{ftitle}**]({fname}) — `{flen} baris`")

    index_lines.extend([
        "",
        "## 🔍 Panduan Pengambilan Pengetahuan (Retrieval)",
        "1. Agen dapat membaca berkas indeks ini terlebih dahulu untuk memetakan topik yang tersedia.",
        "2. Gunakan pembacaan file terarah ke berkas spesifik di atas untuk menjawab pertanyaan teknis mendalam rekan kerja.",
        ""
    ])

    (knowledge_dir / "index.md").write_text("\n".join(index_lines), encoding="utf-8")
    print(f"✅ Indeks knowledge base '{name}' berhasil diperbarui di {knowledge_dir / 'index.md'}")

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

    # groom
    p_groom = subparsers.add_parser("groom", help="Rapikan (groom) knowledge base di workspace")
    p_groom.add_argument("name", help="Nama workspace yang akan dirapikan")

    args = parser.parse_args()

    if args.subcommand == "list":
        cmd_list(args)
    elif args.subcommand == "create":
        cmd_create(args)
    elif args.subcommand == "groom":
        cmd_groom(args)
    else:
        parser.print_help()
        sys.exit(1)

if __name__ == "__main__":
    main()
