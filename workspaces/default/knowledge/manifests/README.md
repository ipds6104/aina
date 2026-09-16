# 📋 Dataset Manifests & Cloud Disaster Recovery Registry

Direktori ini mencatat metadata, kamus skema kolom, dan tautan cadangan cloud (*remote backup*) untuk dataset analitis masif yang disimpan di `shared_data/`.

## 💡 Tujuan
1. **Pemisahan Penyimpanan**: Data biner berukuran ratusan MB hingga multi-GB disimpan di `shared_data/` (di-`.gitignore`), sedangkan resep, skema, dan metadata resminya disimpan di sini (masuk Git).
2. **Disaster Recovery (Ketahanan Mati Listrik / Kerusakan Server)**: Jika server mati listrik dan mengalami kerusakan disk fisik, dataset masif dapat diunduh kembali secara instan dari Google Drive melalui informasi pada file manifest ini.

## 📄 Contoh Format Manifest (`<dataset_slug>.yaml`)
```yaml
id: monitoring_wb2_2026
title: "Dataset Monitoring Sensus & Survei WB2 Kalimantan Barat"
format: duckdb # duckdb | sqlite | parquet
local_path: "shared_data/monitoring_wb2_2026.duckdb"
size_bytes: 1932735283 # ~1.8 GB
checksum_sha256: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
backup_source: gdrive # gdrive | r2 | s3
backup_id: "1abcXYZ..." # Google Drive File ID
backup_url: "https://drive.google.com/file/d/1abcXYZ.../view"
last_updated: "2026-09-16T21:00:00+07:00"
schema:
  tables:
    - name: monitoring_pml
      description: "Data progres approve dan status SLS per PML"
      columns:
        - name: kode_sls
          type: VARCHAR
          description: "Kode SLS 14 digit BPS"
        - name: nama_pml
          type: VARCHAR
          description: "Nama petugas Pengawas / PML"
        - name: progress_approve
          type: DOUBLE
          description: "Persentase persetujuan (0.0 - 100.0)"
```
