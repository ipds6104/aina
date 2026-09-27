---
name: knowledge-curator
description: >-
  Use this skill whenever the user asks to groom, organize, or validate the structured knowledge base,
  track project deadlines and temporal activities, manage massive analytical data lakes (DuckDB/Parquet/SQLite),
  or import and search WhatsApp chat archive history.
---

# Knowledge Curator, Data Lake & Workspace Grooming Skill

This skill equips Aina with production-grade operating procedures to curate the structured knowledge base, track project agendas, manage multi-gigabyte datasets, and search historical conversation archives.

---

## 1. When to Activate This Skill (Trigger Conditions)

Activate this skill whenever:
1. **Merapikan / Memperbarui Katalog Pengetahuan (Grooming)**: Pengguna meminta merapikan knowledge base atau memperbarui ringkasan `knowledge/index.md`.
2. **Pemeriksaan Kerapian & Auto-Healing (Linter)**: Memvalidasi kepatuhan format berkas Markdown, struktur direktori, dan frontmatter YAML.
3. **Pelacakan Agenda, Rapat, & Deadline Proyek**: Pengguna menanyakan jadwal kerja ("*apa agenda minggu ini?*", "*deadline apa yang paling dekat?*") atau meminta membuat proyek baru.
4. **Pengelolaan Dataset Masif (>10 MB s.d. Multi-GB)**: Tim membagikan atau memproses dataset besar untuk analisis data/statistik.
5. **Impor & Pencarian Arsip Obrolan WhatsApp**: Pengguna mengirimkan file backup chat (`.zip` / `.txt`) atau mencari percakapan masa lalu dengan filter waktu.

---

## 2. Standard Operating Procedures (SOP)

### SOP 1: Merapikan Indeks Katalog (Knowledge Grooming)
Jalankan perintah grooming untuk memperbarui `knowledge/index.md` secara deterministik (<50ms):
```bash
aina kb groom
# atau spesifik workspace: aina kb groom --workspace workspaces/kantor
```
*Grooming merangkum seluruh berkas dokumen umum, matriks kegiatan aktif, dan menghitung hitung mundur (countdown) deadline terdekat.*

### SOP 2: Validasi Kerapian & Closed-Loop Auto-Healing
Untuk memeriksa apakah ada format dokumen atau frontmatter yang rusak:
```bash
# Validasi & otomatis perbaiki kesalahan formatting:
aina kb lint --auto-heal
```

### SOP 3: Pelacakan Agenda Kerja & Pembuatan Kegiatan Berkala
1. **Melihat Agenda & Deadline Proyek**:
   ```bash
   aina kb schedule
   # atau via python: python3 scripts/workspace_manager.py schedule --week
   ```
2. **Membuat Kegiatan Berjangka Baru**:
   ```bash
   python3 scripts/workspace_manager.py create-activity default "<nama_kegiatan>" "<periode>" --kategori "<kategori>" --deadline "YYYY-MM-DD:Keterangan"
   ```
   *Berkas otomatis dibuat di `knowledge/kegiatan/<slug>/<periode>/README.md` dengan frontmatter YAML terstandarisasi.*

### SOP 4: Pengelolaan Dataset Masif (>10 MB s.d. Multi-GB / Data Lake)
Jika menerima atau mengolah dataset besar:
1. **DILARANG Masuk ke Git**: Simpan selalu berkas mentah di folder `shared_data/` (folder ini terhubung otomatis di seluruh workspace dan diabaikan dari Git).
2. **Format Data Efisien & Hemat RAM**:
   - Gunakan **DuckDB (`.duckdb`)**, **SQLite (`.db` mode WAL)**, atau **Parquet (`.parquet`)**.
   - Dilarang membaca CSV masif secara utuh dengan Pandas ke dalam RAM agar server tidak mengalami OOM (*Out of Memory*).
3. **Disaster Recovery (Backup Google Drive)**:
   Cadangkan dataset ke Google Drive tim:
   ```bash
   python3 skills/gdrive/scripts/gdrive_tool.py drive-upload --file "shared_data/<nama_file>" --share anyone
   ```
   Catat metadata, skema, dan tautan sharing-nya ke `knowledge/manifests/<slug>.yaml`.
4. **Progressive Distillation (Wawasan Bertumbuh di Git)**:
   Catat intisari analisis, angka agregasi penting, atau anomali temuan ke berkas Markdown (`knowledge/facts.md`). Biarkan bahan mentah tetap di disk, sedangkan wawasan matang tersimpan rapi di Git!

### SOP 5: Arsip Obrolan WhatsApp & Pencarian Temporal Cepat (<5ms)
1. **Mengimpor Berkas Ekspor Chat (.zip / .txt)**:
   ```bash
   aina archive import /path/ke/chat.zip --slug "tim-proyek"
   ```
2. **Pencarian Riwayat Percakapan (FTS5 BM25 + Filter Waktu Presisi)**:
   ```bash
   # Cari percakapan terkini (7 hari terakhir):
   aina archive search "<kata_kunci>" --since 7d

   # Cari percakapan pada rentang tanggal spesifik:
   aina archive search "<kata_kunci>" --from 2026-09-01 --to 2026-09-10
   ```
