# 🗄️ Global Shared Data Lake (Aina Large Datasets)

Direktori ini adalah **penyimpanan fisik terpusat lintas workspace** untuk bahan data analitis berukuran masif (>10 MB hingga multi-GB, misalnya dataset 1.8 GB).

## 📌 Aturan & Prinsip Utama
1. **Di Luar Git (`.gitignore`)**:
   - Seluruh file database biner, dump tabular masif, dan arsip data di direktori ini secara permanen diabaikan dari Git agar repositori tetap ringan (<50 MB) dan cepat di-clone/sync.
2. **Format Tahan Banting Listrik Padam (Crash Resilient) & Hemat RAM**:
   - **DuckDB (`.duckdb`)**: Format analitik kolumnar terbaik. Mampu melakukan query agregasi pada jutaan baris (1.8 GB+) dengan konsumsi RAM di bawah 50 MB langsung dari disk.
   - **SQLite (`.db`)**: Wajib menggunakan mode WAL (`PRAGMA journal_mode = WAL;`) agar aman dari korupsi data saat server mati listrik mendadak.
   - **Parquet (`.parquet`)**: Format penyimpanan arsip biner berkecepatan tinggi dan terkompresi.
   - *Hindari memuat file CSV/JSON mentah raksasa (>500 MB) secara utuh ke memori Python Pandas karena dapat memicu crash OOM (Out-of-Memory).*
3. **Akses Lintas Workspace**:
   - Direktori ini ditautkan secara otomatis (symlink) ke setiap workspace di `workspaces/<workspace_name>/shared_data`.
   - Data fisik hanya ada **satu salinan di disk**, sehingga tidak ada pemborosan kapasitas SSD.
4. **Disaster Recovery (Backup Remote ke Google Drive)**:
   - Cadangan data besar disimpan di Google Drive tim via `gdrive_tool drive-upload`.
   - Metadata, skema, checksum SHA-256, dan ID remote Google Drive dicatat di `knowledge/manifests/<dataset>.yaml` di dalam repositori Git.
