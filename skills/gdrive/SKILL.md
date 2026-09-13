---
name: gdrive
description: >-
  Use this skill whenever the user asks to create, read, update, or append to Google Spreadsheets (GSheet),
  upload files, documents, or data reports to Google Drive, download or export Drive files and spreadsheets,
  generate shareable Google Drive/Sheet links, or manage Google Drive authentication for Aina.
---

# Google Drive & Google Sheets Skill for Aina

This skill equips Aina with production-ready workflows to interact with **Google Drive API v3** and **Google Sheets API v4** using the companion CLI tool [`scripts/gdrive_tool.py`](./scripts/gdrive_tool.py).

---

## 1. When to Activate This Skill

Activate this skill when:
1. **Membuat Google Spreadsheet Baru**: Pengguna meminta dibuatkan spreadsheet/tabel online (misal: *"Aina, tolong buatkan Google Sheet rekap data absensi bulan ini"*).
2. **Membaca atau Mengambil Data dari Google Sheet**: Pengguna memberikan link atau ID Google Sheet dan meminta Aina menganalisis, membaca isi, atau memeriksa data di dalamnya.
3. **Menambahkan Baris / Mengupdate Spreadsheet**: Pengguna meminta menambahkan data baru ke Google Sheet yang sudah ada (misal: *"Aina, catat pengeluaran tadi ke spreadsheet operasional"*).
4. **Mengunggah File / Laporan ke Google Drive**: Aina telah menghasilkan file (PDF, Excel `.xlsx`, gambar chart, arsip zip, dokumen teks) di workspace dan pengguna meminta untuk mengunggahnya ke Google Drive serta memberikan link sharing-nya.
5. **Mengunduh atau Mengekspor File Drive / Sheet**: Pengguna meminta mengunduh file dari Google Drive ke server lokal atau mengekspor Google Sheet menjadi format Excel `.xlsx`, `.csv`, atau `.pdf`.
6. **Memeriksa Status Akun Google**: Pengguna menanyakan apakah Aina sudah terhubung ke Google Drive/Sheets.

---

## 2. Helper Script Location & Calling Convention

The helper tool is located at:
- Global binary / alias: `gdrive_tool`
- In skill directory: `python3 /app/skills/gdrive/scripts/gdrive_tool.py` *(or `python3 skills/gdrive/scripts/gdrive_tool.py`)*

Cek status koneksi:
```bash
gdrive_tool status
```

---

## 3. Standard Operating Procedures (SOP)

### SOP 1: One-Time OAuth 2.0 Setup (Permanent Offline Access)

> [!IMPORTANT]
> **KUNCI AGAR OAUTH PERMANEN (TIDAK EXPIRE TIAP 7 HARI)**:
> 1. Pada Google Cloud Console > **APIs & Services** > **OAuth consent screen**:
>    - Ubah **Publishing status** dari *"Testing"* menjadi **"In production"** (klik tombol **PUBLISH APP**).
>    - Aplikasi pribadi / internal (<100 pengguna) **TIDAK MEMERLUKAN VERIFIKASI GOOGLE**. Status "In production" akan menghapus batasan kedaluwarsa refresh token 7 hari secara permanen!
> 2. Kredensial OAuth dibuat bertipe **Desktop App** (atau Web App dengan redirect `http://localhost:8085`).
> 3. Unduh file JSON kredensial dan simpan sebagai `config/client_secrets.json`.

Langkah menjalankan otorisasi:
```bash
# Jalankan perintah auth
gdrive_tool auth
```
- Jika dijalankan di server dengan browser lokal: CLI akan membuka listener di port 8085 dan menangani callback otomatis.
- Jika dijalankan di remote server (headless/Coolify): Buka link otorisasi yang tercetak di browser, login Google, izinkan akses, lalu salin kode atau URL redirect dan paste ke prompt terminal.

---

### SOP 2: Membuat Google Spreadsheet Baru & Membagikan Link

Ketika pengguna meminta membuat spreadsheet baru:
1. Siapkan data yang ingin dimasukkan (opsional, bisa dalam format CSV lokal atau JSON array).
2. Jalankan perintah `sheets-create`:
   ```bash
   gdrive_tool sheets-create \
     --title "Rekap Data Kegiatan September 2026" \
     --data-csv "workspace/rekap_september.csv" \
     --share anyone \
     --role writer
   ```
3. Script akan mengembalikan output JSON:
   ```json
   {
     "status": "success",
     "id": "1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms",
     "title": "Rekap Data Kegiatan September 2026",
     "url": "https://docs.google.com/spreadsheets/d/1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms/edit?usp=sharing",
     "rows_populated": 15,
     "shared": true
   }
   ```
4. Kirimkan link `url` yang didapatkan langsung ke pengguna di chat WhatsApp!

---

### SOP 3: Membaca Data dari Google Spreadsheet

Ketika pengguna memberikan link atau ID spreadsheet dan meminta membacanya:
```bash
# Membaca sebagai format tabel teks rapi
gdrive_tool sheets-read --url "https://docs.google.com/spreadsheets/d/1BxiMVs.../edit" --range "Sheet1!A1:E20" --format table

# Atau membaca sebagai JSON terstruktur untuk diolah kode Python
gdrive_tool sheets-read --id "1BxiMVs..." --range "Sheet1" --format json
```

---

### SOP 4: Menambahkan Baris ke Spreadsheet (Append)

Ketika pengguna meminta mencatat atau menambahkan transaksi/baris baru:
```bash
# Menambahkan satu baris langsung via parameter:
gdrive_tool sheets-append \
  --url "https://docs.google.com/spreadsheets/d/1BxiMVs.../edit" \
  --range "Sheet1" \
  --row "2026-09-13,Pembelian Kertas A4,Operasional,75000"

# Atau menambahkan banyak baris dari file CSV:
gdrive_tool sheets-append \
  --id "1BxiMVs..." \
  --range "Sheet1" \
  --data-csv "data_tambahan.csv"
```

---

### SOP 5: Mengunggah File ke Google Drive & Mendapatkan Link

Ketika pengguna meminta mengunggah laporan, dokumen PDF, atau arsip:
```bash
gdrive_tool drive-upload \
  --file "workspace/Laporan_Keuangan_2026.pdf" \
  --name "Laporan_Keuangan_Q3_2026.pdf" \
  --share anyone \
  --role reader
```
Output JSON mengembalikan `web_view_link`:
```json
{
  "status": "success",
  "id": "1A2B3C...",
  "name": "Laporan_Keuangan_Q3_2026.pdf",
  "web_view_link": "https://drive.google.com/file/d/1A2B3C.../view?usp=sharing",
  "shared": true
}
```
Sertakan tautan `web_view_link` ini kepada pengguna dalam balasan chat.

> [!TIP]
> **Konversi Otomatis Excel/CSV ke Native Google Sheet**:
> Jika mengunggah file CSV atau Excel dan ingin file tersebut langsung menjadi Google Spreadsheet interaktif (bukan binary xlsx biasa), tambahkan flag `--convert-to-sheets`:
> ```bash
> gdrive_tool drive-upload --file "data.xlsx" --convert-to-sheets --share anyone
> ```

---

### SOP 6: Mengunduh atau Mengekspor File Google Drive / Sheets

Untuk mengunduh file Google Drive atau mengekspor Google Sheet ke file Excel/PDF lokal:
```bash
# Ekspor Google Sheet ke Excel (.xlsx) lokal:
gdrive_tool drive-download \
  --url "https://docs.google.com/spreadsheets/d/1BxiMVs.../edit" \
  --out "workspace/rekap_downloaded.xlsx" \
  --export-format xlsx

# Ekspor Google Sheet atau Doc ke PDF:
gdrive_tool drive-download \
  --id "1BxiMVs..." \
  --out "workspace/laporan.pdf" \
  --export-format pdf

# Unduh file biner umum (gambar/zip/dokumen):
gdrive_tool drive-download \
  --id "1Z9Y8X..." \
  --out "workspace/dokumen_asli.docx"
```
Setelah file berada di workspace lokal, file tersebut dapat dikirimkan langsung ke WhatsApp menggunakan:
```bash
wa_tool send-media --to "<JID>" --file "workspace/rekap_downloaded.xlsx" --caption "Ini file rekap Excel-nya yaa!"
```

---

### SOP 7: Membuat Folder & Mengatur Akses Sharing (File / Folder / Sheet)

Aina dapat membuat folder di Google Drive dan mengatur izin akses untuk file maupun folder:
```bash
# 1. Buat folder baru di Google Drive:
gdrive_tool drive-create-folder --name "Laporan Sensus 2026" --share anyone --role reader

# 2. Bagikan file/folder ke email tertentu (reader, commenter, writer):
gdrive_tool drive-share --url "<link_folder_atau_file>" --share user --email "rekan@gmail.com" --role writer

# 3. Bagikan ke seluruh domain organisasi (misal: @bps.go.id):
gdrive_tool drive-share --id "<id_file_atau_folder>" --share domain --domain "bps.go.id" --role reader

# 4. Ubah file/folder agar dapat diedit oleh siapa saja yang memiliki link:
gdrive_tool drive-share --id "<id>" --share anyone --role writer
```

---

### SOP 8: Memeriksa & Mencabut Izin Akses (Permissions List & Unshare)

Untuk melihat siapa saja yang memiliki akses ke suatu file/folder atau mencabut akses publik:
```bash
# Lihat daftar hak akses aktif:
gdrive_tool drive-permissions-list --url "<link_folder_atau_file>"

# Cabut link sharing publik (anyoneWithLink):
gdrive_tool drive-unshare --id "<id>"

# Cabut akses email tertentu:
gdrive_tool drive-unshare --id "<id>" --email "rekan@gmail.com"
```

---

### SOP 9: Mencari & Memfilter File di Google Drive (`drive-list`)

Aina dapat mencari berkas di Google Drive berdasarkan nama, tipe file, kata kunci konten, maupun folder induk:
```bash
# 1. Cari berdasarkan nama file (substring match):
gdrive_tool drive-list --name "Sensus Ekonomi"

# 2. Filter berdasarkan tipe file tertentu (sheet, doc, slide, folder, pdf, image, video, zip, csv, xlsx):
gdrive_tool drive-list --type sheet --limit 10
gdrive_tool drive-list --type pdf --limit 5

# 3. Pencarian kata kunci menyeluruh (isi dokumen & judul file):
gdrive_tool drive-list --search "Kabupaten Mempawah"

# 4. Filter berkas dalam folder tertentu:
gdrive_tool drive-list --folder-id "<folder_id>" --type sheet

# 5. Lihat berkas yang sedang berada di folder Sampah (Trash):
gdrive_tool drive-list --trashed --limit 10
```

---

### SOP 10: Menghapus & Memulihkan File dari Google Drive (`drive-delete` & `drive-restore`)

Aina mendukung penghapusan aman (pindah ke Sampah / Trash) dan pemulihan berkas:
```bash
# 1. Hapus aman (Pindahkan ke Sampah / Trash Google Drive):
gdrive_tool drive-delete --id "<id_file_atau_folder>"
gdrive_tool drive-delete --url "<link_file_atau_folder>"

# 2. Pulihkan berkas dari Sampah ke lokasi semula:
gdrive_tool drive-restore --id "<id_file_atau_folder>"

# 3. Hapus permanen (Hard Delete - tidak dapat dipulihkan):
gdrive_tool drive-delete --id "<id_file_atau_folder>" --permanent
```

---

## 4. Automatic Token Refresh Mechanism

Aina tidak perlu meminta pengguna login berulang kali. Kapanpun `gdrive_tool` dipanggil:
1. Tool memeriksa apakah `expires_at` pada `google_token.json` masih valid (> 60 detik).
2. Jika sudah mendekati kedaluwarsa atau habis, tool secara otomatis mengirim permintaan refresh ke Google OAuth token endpoint (`https://oauth2.googleapis.com/token`) menggunakan `refresh_token`.
3. Token baru langsung diperbarui di `google_token.json` tanpa mengganggu proses perintah sama sekali.

