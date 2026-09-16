# 📋 Standar Operasional & Alur Kerja Kantor (SOP & Playbook)

## 1. Peta Sumber Data Resmi & Routing Cepat (Canonical Data Routing)

Agar proses pencarian data tidak tersesat atau memakan waktu puluhan menit, selalu gunakan jalur resmi berikut:

| Kategori Permintaan | Sumber Data Resmi | Tool / Skrip Eksekusi | Larangan / Catatan Kinerja |
| :--- | :--- | :--- | :--- |
| **Sensus Ekonomi (SE / SE2026)**<br>(Nama ART, usaha, koordinat, link GMaps, SLS) | **SurrealDB (`se2026`)**<br>Tabel `nested_dtsen_var` & `assignment` | Skrip SurrealDB (`se2026_model.py`) / query langsung ke SurrealDB | ⚠️ **DILARANG KERAS** mencari di Google Drive atau membedah file PDF/DOCX! Data sensus lengkap ada di SurrealDB (hasil dalam **0,4 detik**). Google Drive hanya berisi file administrasi perjalanan dinas. |
| **Monitoring WB2**<br>(Progress PML, target SLS) | **Google Sheets WB2** | `gdrive_tool sheets-read` (Tab Perpml) / format `htmlembed` | Untuk tangkapan layar bersih tanpa toolbar rumus, gunakan format embed. |
| **Dokumen / Surat Tugas / SK** | **Google Drive Tim** | `gdrive_tool drive-list` / `drive-download` | Khusus dokumen PDF/Word/Excel pendukung administrasi kantor. |

---

## 2. Standar Keamanan Akses Data (`scripts/data_guard.py`)

Sebelum menyajikan data sensitif (misal data SE2026, data kepegawaian, data kemiskinan perorangan):
1. **Wajib Cek Akses**: Jalankan verifikasi deterministik:
   ```bash
   python3 scripts/data_guard.py check --dataset <slug> --sender "<sender_jid>" --chat "<chat_jid>"
   ```
2. **Jika Hasil `DENY`**:
   - Dilarang membocorkan data fisik atau ringkasannya sedikitpun.
   - Arahkan pemohon untuk konfirmasi langsung ke Bang Ihza (Admin).
   - Tawarkan pembuatan tiket izin: `python3 scripts/data_guard.py request-access --dataset <slug> --sender "<sender_jid>" --reason "<alasan>"`.
3. **Jika Hasil `ALLOW`**: Eksekusi kueri ke database resmi sesuai hak akses.

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
