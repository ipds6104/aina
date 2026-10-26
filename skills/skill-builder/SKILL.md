---
name: skill-builder
description: >-
  Use this skill whenever the user asks Aina to create, build, extend, test, or update custom skills, integrations,
  or autonomous tools. Guides compliant Antigravity skill structure, persistent storage in `data/custom-skills`,
  JIT secret handling via Infisical, multi-template scaffolding (api, cli, data, minimal), self-validation,
  automated testing, and Git synchronization to `USER_SKILLS_REPO`.
---

# Autonomous Skill Builder & Extensibility Manager

This skill equips Aina with production-grade operating procedures to **build, extend, test, and persist** new custom skills and third-party integrations autonomously upon user request.

---

## 1. When to Activate This Skill (Trigger Conditions)

Activate this skill immediately whenever:
1. **Permintaan Integrasi / Fitur Baru dari Pengguna**:
   Pengguna di WhatsApp atau web chat meminta:
   - *"Aina, tolong buatkan integrasi / skill baru untuk Tally.so"*
   - *"Aina, buatkan tool untuk auto-deploy repo X"*
   - *"Aina, tambahkan skill untuk query database / parquet"*
   - *"Aina, tolong buat skill kustom baru untuk API Y"*
2. **Kebutuhan Tool Khusus Repositori Produksi**:
   Aina mengidentifikasi perlunya script otomasi baru untuk menangani repositori atau layanan tertentu.
3. **Pencadangan / Sinkronisasi Skill ke Git**:
   Pengguna meminta: *"Aina, backup / sync custom skill ke GitHub"*.

---

## 2. Aturan Mutlak Persistensi & Arsitektur (Zero Data Loss)

> [!IMPORTANT]
> **LOKASI PERSISTENSI SKILL WAJIB**:
> 1. **Dilarang Menyimpan Skill Kustom di `/app/skills/` Inti**:
>    Direktori `/app/skills/` adalah bawaan image container yang bersifat *read-only* atau ter-reset saat image diperbarui.
> 2. **Wajib Simpan di `data/custom-skills/<nama_skill>/`**:
>    Folder ini terhubung langsung ke persistent volume Docker `aina_data:/app/data/custom-skills/`. Data di sini **TIDAK AKAN HILANG** saat container di-redeploy, di-restart, atau di-rebuild di Coolify.
> 3. **Discovery Otomatis AGY**:
>    Folder ini telah terdaftar secara bawaan di `.agents/skills.json` sehingga mesin Antigravity langsung mengenali skill baru tanpa perlu restart server!

---

## 3. Pilihan Template Arsitektur

Saat scaffolding skill baru, pilih `--template` yang paling sesuai:

| Template | Deskripsi | Cocok Digunakan Untuk |
| :--- | :--- | :--- |
| `api` | HTTP REST API client lengkap dengan timeout, Bearer auth, retries, dan format WhatsApp. | Layanan eksternal (Tally, Notion, GitHub API, Cloudflare, Webhook). |
| `cli` | Wrapper biner/subproses eksternal dengan isolasi timeout dan format JSON. | Biner sistem, docker CLI, git runner, utilitas jaringan. |
| `data` | Tabular query & filter (SQLite, DuckDB, Parquet) hemat memori. | Dataset analitik, database lokal, laporan CSV/Parquet. |
| `minimal` | Template dasar Python dengan subcommand `status` dan `run`. | Otomasi sederhana atau prototipe cepat. |

---

## 4. Strict OpSec & Secret Storage (Infisical Integration)

> [!CAUTION]
> **JANGAN PERNAH MENG-HARDCODE KREDENSIAL / TOKEN**:
> 1. Jika skill baru membutuhkan API Key (misal: `TALLY_API_KEY`, `NOTION_TOKEN`, `DATADOG_API_KEY`):
>    - Simpan token ke Infisical vault:
>      ```bash
>      python3 skills/infisical/scripts/secret_tool.py set <NAMA_KEY> "<NILAI_TOKEN>"
>      ```
>    - Jalankan tool baru dengan injeksi Just-In-Time (JIT):
>      ```bash
>      python3 skills/infisical/scripts/secret_tool.py run -- python3 data/custom-skills/<skill>/scripts/<tool>.py
>      ```
> 2. Di dalam kode Python/Bash, selalu baca dari environment: `os.getenv("NAMA_KEY")`.
> 3. Jangan pernah mencetak token mentah ke obrolan WhatsApp atau layar log!

---

## 5. Standard Operating Procedures (SOP Pembuatan Skill Baru)

```text
┌────────────────┐     ┌────────────────┐     ┌────────────────┐     ┌────────────────┐     ┌────────────────┐
│ 1. Scaffold    │ ──> │ 2. Implement   │ ──> │ 3. OpSec &     │ ──> │ 4. Auto-Test   │ ──> │ 5. Git Backup  │
│ persistent dir │     │ logic & scripts│     │ Vault Secrets  │     │ & Validate     │     │ & Confirm User │
└────────────────┘     └────────────────┘     └────────────────┘     └────────────────┘     └────────────────┘
```

### Langkah 1: Scaffold Skill Baru di Persistent Storage
Gunakan helper tool `skill_scaffolder.py` atau CLI `aina skill`:
```bash
# Contoh membuat skill API
python3 skills/skill-builder/scripts/skill_scaffolder.py init <nama_skill> --template api --desc "<Deskripsi fungsi skill>"

# Atau via CLI Aina
aina skill init <nama_skill> --template api --desc "<Deskripsi fungsi skill>"
```
*Perintah ini otomatis membuat:*
- `data/custom-skills/<nama_skill>/SKILL.md` (lengkap dengan frontmatter standar).
- `data/custom-skills/<nama_skill>/scripts/<nama_skill>_tool.py` (executable `chmod +x`).

### Langkah 2: Tulis Logika Tool & Dokumentasi
1. Sesuaikan skrip di `data/custom-skills/<nama_skill>/scripts/`:
   - Gunakan argument parser (`argparse` untuk Python).
   - Selalu kembalikan output berformat JSON (`{"status": "success", ...}`) agar mudah dicerna AI.
2. Perbarui `SKILL.md`:
   - Pastikan field `name` sama dengan nama folder.
   - Cantumkan trigger conditions dan contoh perintah yang jelas.

### Langkah 3: Integrasikan Kredensial via Infisical
Jika tool membutuhkan kredensial pihak ketiga:
```bash
secret_tool set <KEY_NAME> "<VALUE>"
```

### Langkah 4: Pengujian Otomatis (Closed-Loop Testing & Validation)
Sebelum mengonfirmasi ke pengguna, jalankan pengujian otomatis:
```bash
# 1. Jalankan test otomatis (compile, --help, status command)
python3 skills/skill-builder/scripts/skill_scaffolder.py test <nama_skill>

# 2. Jalankan validator kepatuhan format & OpSec
python3 skills/skill-builder/scripts/skill_scaffolder.py validate <nama_skill>
```
*Pengujian memastikan skrip dapat dikompilasi, dapat dieksekusi, statusnya siap, dan tidak ada hardcoded secret.*

### Langkah 5: Pencadangan & Sinkronisasi Git (`USER_SKILLS_REPO`)
Tawarkan pencadangan ke repositori Git privat pengguna:
```bash
python3 skills/skill-builder/scripts/skill_scaffolder.py sync --message "Add <nama_skill> integration"
```

### Langkah 6: Konfirmasi ke Pengguna
Kirimkan konfirmasi santun dan jelas:
> *"Siaapp Kak/Bang, skill baru `<nama_skill>` sudah selesai Aina buat dan uji coba 👍. Template `<template>` telah aktif dan tersimpan persisten di volume data!"*

---

### SOP Alternatif: Install Custom Skill dari Repositori Git

Bila pengguna di WhatsApp atau web chat meminta:
- *"Aina, tolong install skill X dari github.com/..."*
- *"Aina, pasang skill `arc42-architect` dari repo `ihkaru/agent-skills`"*

Jalankan perintah Git installer:
```bash
# 1. Install 1 skill spesifik dari repositori multi-skill / hub:
python3 skills/skill-builder/scripts/skill_scaffolder.py install https://github.com/ihkaru/agent-skills --skill <nama_skill>

# 2. Install langsung dari repositori single-skill:
python3 skills/skill-builder/scripts/skill_scaffolder.py install <url_repo_git>

# 3. Atau via CLI aina:
aina skill install <url_repo_git> [--skill <nama_skill>] [--all]
```
*Installer otomatis meng-clone shallow `--depth 1`, mendeteksi struktur repo, memindahkan berkas ke volume persisten `data/custom-skills/<nama>/`, memasang izin `chmod +x`, serta menginstall dependensi `requirements.txt` via `pip install --user` ke storage persisten Python.*

---

## 6. Helper Tool Command Reference

| Perintah | Deskripsi |
| :--- | :--- |
| `skill_scaffolder init <NAME> [--template api\|cli\|data\|minimal] [--desc "<DESC>"]` | Membuat template skill baru di `data/custom-skills/`. |
| `skill_scaffolder install <URL> [--skill <NAME>] [--all] [--force]` | Meng-clone dan meng-install skill dari repositori Git ke volume persisten. |
| `skill_scaffolder test <NAME>` | Menjalankan suite pengujian otomatis (syntax compile, `--help`, status command). |
| `skill_scaffolder validate <NAME>` | Memvalidasi kepatuhan format YAML, izin eksekusi, dan kebocoran secret. |
| `skill_scaffolder list` | Menampilkan seluruh custom skill yang terpasang dan status remote Git. |
| `skill_scaffolder delete <NAME> [--force]` | Menghapus skill kustom dari volume persisten. |
| `skill_scaffolder sync [--repo <URL>]` | Melakukan commit dan push folder custom skills ke Git repository. |


---

## 7. Sinergi dengan Skill Lain

- **`skills/infisical`**: Brankas kredensial untuk semua token pihak ketiga yang dibutuhkan skill baru.
- **`skills/knowledge-curator`**: Mencatat katalog kapabilitas baru ke `knowledge/index.md`.
- **`skills/tabayyun`**: Meminta izin atau klarifikasi parameter jika instruksi pembuatan skill belum lengkap.
