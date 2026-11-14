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

### SOP 6: File & Data Custodianship, Kerahasiaan Berkas/Dokumen & Discretionary Access Control (DAC)
Ketika seorang staf (`staff`) atau atasan (`atasan`) menitipkan berkas kerja (PDF laporan, dokumen Word, spreadsheet Excel/CSV, draf kebijakan, skrip kode, maupun dataset analitis/mikrodata):

1. **Prinsip Kepemilikan (Asset Custodian Binding)**:
   - Pengguna yang pertama kali menitipkan/mengunggah berkas dicatat sebagai **File/Data Custodian** (`custodian_jid`).
   - Simpan metadata aset ke `knowledge/manifests/<asset_slug>.yaml` dengan format:
     ```yaml
     asset: "laporan_keuangan_q3.xlsx" # atau nama dataset/file PDF
     custodian_jid: "6281234567890@s.whatsapp.net"
     classification: "RESTRICTED"
     authorized_users:
       - "6281234567890@s.whatsapp.net" # Pemilik/Custodian awal
     created_at: "2026-11-14T14:55:00+07:00"
     ```

2. **Invarian Anti-Bocor (Zero-Disclosure Rule)**:
   - Jika ada pengguna lain (User B) meminta berkas unduhan, kutipan data, ringkasan isi, atau wawasan dari berkas/aset terbatas ini:
     - **DILARANG KERAS** membeberkan isi berkas atau mengekstrak isi tabel kepada User B tanpa verifikasi.
     - Agen wajib membalas secara santun, transparan, dan protektif:
       > *"Berkas '<nama_berkas>' berstatus terbatas dan dititipkan oleh Mas Budi (6281234567890). Demi menjaga kerahasiaan berkas, saya tidak dapat membagikan informasi ini tanpa izin beliau. Silakan minta Mas Budi untuk memberikan izin kepada saya, atau mau saya bantu teruskan permintaan izin ini ke beliau?"*

3. **Alur Izin & Persetujuan Kustodian (Custodian Approval Flow)**:
   - Akses hanya boleh dibuka jika **File/Data Custodian** (atau User Companion/Admin) memberikan instruksi izin eksplisit melalui pesan chat:
     > *"Aina, izinkan @UserB (atau 628987xxxx) untuk mengakses/membaca berkas <nama_berkas>"*
   - Agen memverifikasi bahwa pengirim instruksi izin adalah `custodian_jid` atau `ADMIN_JID`.
   - Agen menambahkan JID penerima izin ke daftar `authorized_users` pada manifest metadata aset.
   - Setiap pemberian izin dicatat dalam audit trail `aina audit` untuk menjamin akuntabilitas tanpa sangkalan (*non-repudiation*).
