---
name: se-data
description: >-
  Gunakan skill ini setiap kali ada pertanyaan, analisis, atau permintaan data terkait Sensus Ekonomi (SE / SE2026),
  direktori perusahaan, unit usaha, assignment petugas, muatan SLS/meteran, atau data tabular SE lainnya.
  Menjamin penggunaan dataset resmi Parquet terpusat di shared_data/ via DuckDB tanpa perlu menebak sumber data.
---

# Sensus Ekonomi 2026 (SE2026) Official Dataset & OLAP Engine Skill

Skill ini mengatur tata kelola, standar kueri, dan protokol akses data resmi untuk seluruh kebutuhan data Sensus Ekonomi (SE2026) dan direktori usaha/perusahaan.

---

## 1. Aturan Sumber Data Utama (Primary Data Source Rule)

> [!IMPORTANT]
> **Kebijakan Default Mutlak**:  
> Seluruh permintaan data terkait **Sensus Ekonomi (SE)**, **unit usaha**, **perusahaan**, atau **profil kegiatan ekonomi** **WAJIB MENGGUNAKAN DATA PARQUET INI**.  
> **Pengecualian**: Hanya boleh menggunakan sumber lain bila pengguna secara eksplisit meminta sumber tertentu (misalnya: "*pakai file CSV Realisasi*", "*cek di Google Sheets ad-hoc*", atau "*ambil dari database PostgreSQL/SurrealDB langsung*").

---

## 2. Lokasi Fisik & Struktur Dataset Parquet

Seluruh berkas tersimpan secara persisten di folder terpusat:
`shared_data/` (tertaut ke `/app/data/shared_data/` pada persistent storage).

### 6 Tabel Master Parquet SE2026:
1. **`assignment.parquet`** (~131.133 baris):
   - Muatan: Penugasan beban kerja sensus per petugas (PPL/PML), status alokasi, progres SLS.
   - Kolom Kunci: `assignment_id`, `sls_id`, `petugas_id`, `status`, `target`, `realisasi`.
2. **`se2026_nested.parquet`** (~78.271 baris):
   - Muatan: Master data unit usaha/perusahaan hasil pendataan lengkap SE2026.
   - Kolom Kunci: Identitas usaha/perusahaan, nama pemilik, alamat, KBLI, skala usaha, status operasional.
3. **`nested_dtsen.parquet`** (~260.612 baris):
   - Muatan: Data rincian identitas dan karakteristik usaha/keluarga sensus.
4. **`nested_dtsen_var.parquet`** (~250.274 baris):
   - Muatan: Variabel-variabel detail lanjutan dari unit usaha (omzet, tenaga kerja, aset, dsb.).
5. **`nested_meteran.parquet`** (~68.384 baris):
   - Muatan: Data nomor ID meteran listrik/PLN unit usaha untuk validasi lokasi & konsumsi energi.
6. **`kp_nested.parquet`** (~6 baris):
   - Muatan: Klasifikasi referensi atau kelompok master sensus.

---

## 3. Standar Kueri Analitik (DuckDB High-Performance OLAP)

Gunakan selalu engine **DuckDB** untuk kueri agregasi, pemfilteran, dan tabulasi silang. Kueri berjalan langsung dari disk dengan efisiensi RAM sangat tinggi (<50 MB) dan kecepatan sub-detik.

### Cara 1: Menggunakan Utilitas Script (CLI)
```bash
python3 scripts/sync_se2026_parquet.py query "SELECT COUNT(*) FROM 'shared_data/se2026_nested.parquet';"
```

### Cara 2: Menggunakan Python DuckDB di Workspace
```python
import duckdb

con = duckdb.connect()
df = con.execute("""
    SELECT 
        nama_usaha,
        alamat,
        kbli,
        kategori
    FROM 'shared_data/se2026_nested.parquet'
    WHERE lower(nama_usaha) LIKE '%makmur%'
    LIMIT 20
""").fetchdf()
print(df)
```

---

## 4. Standar Operasional Prosedur (SOP) Analisis Data

1. **Pemilihan Tabel yang Tepat**:
   - Jika menanyakan **perusahaan / unit usaha / jenis usaha**: Kueri `se2026_nested.parquet` (dan join dengan `nested_dtsen_var.parquet` bila membutuhkan variabel detail).
   - Jika menanyakan **beban kerja / progres petugas / SLS**: Kueri `assignment.parquet`.
   - Jika menanyakan **meteran listrik PLN usaha**: Kueri `nested_meteran.parquet`.
2. **Penyajian Hasil ke Rekan Kerja (Format WhatsApp Ramah Ponsel)**:
   - Hindari format tabel Markdown `| a | b |` (tabel rusak parah di layar ponsel).
   - Gunakan format ringkasan naratif, poin-poin dengan huruf *TEBAL*, dan daftar butir `•`.
   - Sertakan metrik penting: total unit usaha, persentase breakdown, atau temuan anomali utama.
3. **Ekspor Hasil Analisis (Bila Diminta Rekan Kerja)**:
   - Jika rekan kerja meminta daftar baris atau file excel:
     Simpan hasil kueri ke format Excel/CSV di folder `output/` atau `workspace/`.
     Kirimkan menggunakan tool WhatsApp:
     `python3 skills/whatsmeow/scripts/wa_tool.py send-media --to <chat_jid> --file output/hasil_se2026.xlsx --caption "<ringkasan>"`

---

## 5. Pemeriksaan Status & Sinkronisasi Data (Observability)

- **Cek Status & Ketersediaan Berkas Lokal**:
  ```bash
  python3 scripts/sync_se2026_parquet.py status
  ```
- **Sinkronisasi Ulang / Delta Sync dari SurrealDB**:
  > [!IMPORTANT]
  > Untuk sinkronisasi data besar yang membutuhkan waktu >1 menit, **WAJIB sertakan flag `--daemon`** agar berjalan di latar belakang tanpa menahan chat WhatsApp:
  ```bash
  python3 scripts/sync_se2026_parquet.py sync --tables all --daemon --notify-wa "<chat_jid>"
  ```
