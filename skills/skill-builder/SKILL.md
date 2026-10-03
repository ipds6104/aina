---
name: skill-builder
description: >-
  Use this skill whenever the user asks Aina to create, build, extend, or update custom skills, integrations,
  or autonomous tools. Guides compliant Antigravity skill structure, persistent storage in `data/custom-skills`,
  JIT secret handling via Infisical, self-validation, and Git synchronization to `USER_SKILLS_REPO`.
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
   - *"Aina, tambahkan skill untuk query Prometheus / Loki"*
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

## 3. Strict OpSec & Secret Storage (Infisical Integration)

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

## 4. Standard Operating Procedures (SOP Pembuatan Skill Baru)

```text
┌────────────────┐     ┌────────────────┐     ┌────────────────┐     ┌────────────────┐     ┌────────────────┐
│ 1. Scaffold    │ ──> │ 2. Implement   │ ──> │ 3. OpSec &     │ ──> │ 4. Self-Test   │ ──> │ 5. Git Backup  │
│ persistent dir │     │ logic & scripts│     │ Vault Secrets  │     │ & Validate     │     │ & Confirm User │
└────────────────┘     └────────────────┘     └────────────────┘     └────────────────┘     └────────────────┘
```

### Langkah 1: Scaffold Skill Baru di Persistent Storage
Gunakan helper tool `skill_scaffolder.py`:
```bash
python3 skills/skill-builder/scripts/skill_scaffolder.py init <nama_skill> --desc "<Deskripsi fungsi skill>"
```
*Perintah ini otomatis membuat:*
- `data/custom-skills/<nama_skill>/SKILL.md` (lengkap dengan frontmatter standar).
- `data/custom-skills/<nama_skill>/scripts/<nama_skill>_tool.py` (executable `chmod +x`).

### Langkah 2: Tulis Logika Tool & Dokumentasi
1. Tulis atau modifikasi skrip di `data/custom-skills/<nama_skill>/scripts/`:
   - Gunakan argument parser (`argparse` untuk Python atau standard flags).
   - Selalu kembalikan output berformat JSON (`{"status": "success", ...}`) agar mudah dicerna AI.
2. Perbarui `SKILL.md`:
   - Pastikan field `name` sama dengan nama folder.
   - Cantumkan trigger conditions dan contoh perintah yang jelas.

### Langkah 3: Integrasikan Kredensial via Infisical
Jika tool membutuhkan kredensial pihak ketiga:
```bash
# Tanya pengguna dengan santun atau ambil dari brankas:
secret_tool list
```

### Langkah 4: Pengujian Mandiri (Closed-Loop Validation)
Sebelum mengonfirmasi ke pengguna:
1. **Jalankan skrip secara langsung**:
   ```bash
   python3 data/custom-skills/<nama_skill>/scripts/<nama_skill>_tool.py status
   ```
2. **Jalankan validator kepatuhan format & OpSec**:
   ```bash
   python3 skills/skill-builder/scripts/skill_scaffolder.py validate <nama_skill>
   ```
   *Validator akan memastikan tidak ada hardcoded token, izin berkas benar, dan frontmatter valid.*

### Langkah 5: Pencadangan & Sinkronisasi Git (`USER_SKILLS_REPO`)
Tawarkan pencadangan ke repositori Git privat pengguna:
```bash
# Jika USER_SKILLS_REPO sudah disetel:
python3 skills/skill-builder/scripts/skill_scaffolder.py sync --message "Add <nama_skill> integration"

# Atau inisialisasi repo baru via GitHub CLI:
gh repo create my-custom-skills --private --source=data/custom-skills --push
```

### Langkah 6: Konfirmasi ke Pengguna
Kirimkan konfirmasi santun dan jelas:
> *"Siaapp Kak/Bang, skill baru `<nama_skill>` sudah selesai Aina buat dan uji coba 👍. Skill tersimpan secara persisten di volume brankas data dan siap dipanggil kapan saja!"*

---

## 5. Helper Tool Command Reference

| Perintah | Deskripsi |
| :--- | :--- |
| `skill_scaffolder.py init <NAME> --desc "<DESC>"` | Membuat template skill baru di direktori persisten `data/custom-skills/`. |
| `skill_scaffolder.py validate <NAME>` | Memvalidasi kepatuhan format YAML, izin eksekusi, dan memeriksa adanya kebocoran secret. |
| `skill_scaffolder.py list` | Menampilkan seluruh custom skill yang terpasang dan status remote Git. |
| `skill_scaffolder.py sync [--repo <URL>]` | Melakukan commit dan push folder custom skills ke repositori Git pengguna. |

---

## 6. Sinergi dengan Skill Lain

- **`skills/infisical`**: Brankas kredensial untuk semua token pihak ketiga yang dibutuhkan skill baru.
- **`skills/knowledge-curator`**: Mencatat katalog kapabilitas baru ke `knowledge/index.md`.
- **`skills/tabayyun`**: Meminta izin atau klarifikasi parameter jika instruksi pembuatan skill belum lengkap.
