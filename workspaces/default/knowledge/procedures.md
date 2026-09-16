# 📋 Standar Operasional & Alur Kerja (SOP & Playbook)

## 1. Peta Sumber Data & Routing Cepat (Canonical Data Routing Pattern)

Agar proses pencarian data tidak tersesat atau memakan waktu puluhan menit, ikuti prinsip routing baku berikut:

| Kategori Data yang Dicari | Karakteristik Sumber Data | Tool / Skrip Eksekusi | Larangan / Anti-Pattern Kinerja |
| :--- | :--- | :--- | :--- |
| **Data Tabular / Entitas / Transaksional**<br>(Data survei, sensus, log, metrik kuantitatif) | **Database Analitik Terstruktur**<br>(DuckDB, SQLite WAL, atau database analitik terpasang) | Kueri SQL terstruktur / skrip model data | ⚠️ **DILARANG KERAS** mencari data entitas/kuantitatif di Google Drive atau membedah file dokumen PDF/DOCX! Data terstruktur harus selalu ditarik dari database resmi (hasil instan sub-detik). |
| **Data Spreadsheet Operasional**<br>(Monitoring harian, form input) | **Spreadsheet Resmi / Cloud Sheets** | Helper spreadsheet resmi (`gdrive_tool sheets-read`) | Gunakan format embed bersih jika dibutuhkan screenshot. |
| **Dokumen Naratif / Surat / Administrasi** | **Penyimpanan Dokumen Cloud** | Tool penyimpanan berkas (`gdrive_tool drive-list`) | Khusus dokumen PDF/Word/surat keputusan pendukung administrasi. |

---

## 2. Standar Keamanan Akses Data (`scripts/data_guard.py`)

Sebelum menyajikan data yang diklasifikasikan sebagai sensitif (`INTERNAL`, `RESTRICTED`, atau `CONFIDENTIAL`):
1. **Wajib Cek Akses**: Jalankan verifikasi deterministik:
   ```bash
   python3 scripts/data_guard.py check --dataset <slug> --sender "<sender_jid>" --chat "<chat_jid>"
   ```
2. **Jika Hasil `DENY`**:
   - Dilarang membocorkan data fisik atau ringkasannya sedikitpun.
   - Arahkan pemohon untuk konfirmasi langsung ke Administrator / Penanggung Jawab Dataset (Approver).
   - Tawarkan pembuatan tiket izin: `python3 scripts/data_guard.py request-access --dataset <slug> --sender "<sender_jid>" --reason "<alasan>"`.
3. **Jika Hasil `ALLOW`**: Eksekusi kueri ke database resmi sesuai hak akses yang diberikan.

---

## 3. Aturan Anti-Rabbit Hole (Time-Boxing & Tool Failure Recovery)

1. **Batas Eksplorasi Tool (Maksimal 2 Menit)**:
   - Jika suatu pustaka tidak terpasang (misal `pypdf`, `pdftotext`), **DILARANG KERAS** menulis skrip mandiri untuk membedah biner zlib/stream dokumen secara manual!
   - Berhentilah sejenak (*Tawaqquf Pause*) dan evaluasi: *"Apakah saya sedang mencari data di tempat yang salah?"*.
2. **Prioritaskan Database Terstruktur**:
   - Jika yang dicari adalah data entitas orang, lokasi, atau koordinat, hampir selalu data tersebut tersimpan di database analitik (SurrealDB, SQLite, DuckDB), bukan di dalam dokumen Word/PDF.

---

## 4. Pemeliharaan & Troubleshooting Umum
1. **Pemeriksaan Model AI**: Jalankan `scripts/model_control.py` untuk memeriksa status model aktif.
2. **Pengorganisasian Proyek Baru**: Jalankan `python3 scripts/workspace_manager.py create <nama_proyek>` untuk membuat ruang kerja terisolasi.
