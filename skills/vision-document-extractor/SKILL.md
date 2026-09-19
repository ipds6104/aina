---
name: vision-document-extractor
description: >-
  Gunakan skill ini setiap kali pengguna mengirimkan berkas PDF, dokumen laporan keuangan,
  tabel bertingkat, dokumen multi-halaman, gambar struk belanja, invoice, atau foto berkas pindaian (scan)
  yang membutuhkan pembacaan teks, rumus, dan ekstraksi tabel berakurasi tinggi (100% akurasi tabel).
---

# Vision-Language Document & Table Extraction Skill

Skill ini membekali Aina dengan kemampuan ekstraksi dokumen berbasis Vision-Language Model (via 9Router OpenAI-compatible VLM hub) dengan beban CPU lokal 0%, preservasi tabel kompleks (borderless, multi-line cells, colspan/rowspan), serta konversi otomatis ke berkas CSV.

---

## 1. Kapan Harus Mengaktifkan Skill Ini

Aktifkan skill ini ketika:
1. **Pengguna Mengirimkan PDF**: Pengguna di WhatsApp mengirim file PDF (laporan kerja, regulasi, materi rapat, slip gaji, dsb.).
2. **Ekstraksi Tabel & Angka Finansial**: Pengguna meminta audit, pemeriksaan angka, atau konversi tabel dari PDF/gambar ke data terstruktur / CSV / Excel.
3. **Dokumen Hasil Pindaian (Scan) / Struk**: Gambar foto dokumen yang sulit dibaca oleh OCR biasa karena ada bayangan, kemiringan kertas, stempel, atau font khusus.
4. **Permintaan Pengguna**: *"Aina tolong baca isi PDF ini"*, *"Tolong rangkum halaman 3-5 saja"*, *"Ekstrak tabel di PDF ini ke CSV"*, atau *"Berapa total nilai pada invoice ini?"*.

---

## 2. Pilihan Flag CLI yang Fleksibel & Best Practice

Gunakan perintah resmi `agy-doc-extract` (atau `python3 skills/vision-document-extractor/scripts/doc_extract.py`):

```bash
agy-doc-extract "<file>" [flags]
```

### Daftar Flag Operasional:

| Flag | Argumen | Fungsi & Contoh Penggunaan |
| :--- | :--- | :--- |
| `-o, --output` | `<dir>` | Direktori output (default: `output/doc_extract/`). |
| `-p, --pages` | `1-5`, `3,7` | **Filter Halaman**: Memproses halaman tertentu saja. Sangat hemat kuota & cepat jika PDF tebal. Contoh: `-p 3-5`. |
| `-m, --mode` | `auto`, `tables`, `text`, `math`, `raw` | **Mode Ekstraksi Spesifik**:<br/>• `tables`: Hanya ekstrak tabel (abaikan narasi).<br/>• `text`: Hanya ekstrak teks narasi/paragraf.<br/>• `math`: Ekstrak rumus ke format LaTeX `$...$`.<br/>• `auto`: Ekstrak menyeluruh (default). |
| `--csv` | *(tanpa argumen)* | **Ekspor CSV Otomatis**: Setiap tabel HTML otomatis dikonversi menjadi berkas `.csv` terpisah di folder output (siap dikirim ke WhatsApp via `wa_tool send-media`). |
| `-s, --summary` | *(tanpa argumen)* | **Ringkasan Cepat**: Meminta VLM menyertakan poin-poin ringkasan eksekutif di awal hasil. |
| `-q, --prompt` | `"<teks>"` | **Fokus Khusus**: Memberi arahan pencarian tertentu ke VLM (misal: `-q "fokus pada total tagihan dan nomor rekening"`). |
| `--stdout` | *(tanpa argumen)* | Menampilkan hasil langsung ke terminal (berguna jika dokumen hanya 1 halaman). |
| `--dpi` | `150`, `200`, `300` | Resolusi rasterisasi PDF (default: `200`). Gunakan `300` untuk struk/tabel sangat kecil. |
| `-c, --concurrency`| `1..16` | Jumlah worker paralel per halaman (default: `4`, optimal: `8`–`16`). |
| `-r, --retries` | `<N>` | **Auto-Restart / Retry**: Jumlah percobaan ulang otomatis jika ada halaman gagal/error jaringan (default: `3`). |
| `--retry-delay` | `<detik>` | Jeda waktu awal antar percobaan ulang dengan exponential backoff (default: `2.0` detik). |
| `--continue-on-error`| *(tanpa argumen)* | Tetap lanjutkan pemrosesan halaman lain jika suatu halaman gagal setelah seluruh retry habis (tidak membatalkan dokumen utuh). |
| `--model` | `<nama_model>` | Override model VLM (default: `cbai/deepseek-v4.1-flash`). |



---

## 3. Alur Kerja Standar Operasional (SOP) di WhatsApp

### Skenario A: Pengguna Mengirim PDF dan Minta Rangkuman / Jawaban
1. Jalankan ekstraksi:
   ```bash
   agy-doc-extract "<path_file.pdf>" -o output/doc_extract/
   ```
2. Buka dan baca `output/doc_extract/extracted_content.md` menggunakan `view_file`.
3. Balas ke chat WhatsApp dengan format teks ramah, dan ubah tabel menjadi format poin *bullet* (`•`) yang rapi.

### Skenario B: Pengguna Hanya Ingin Halaman Tertentu
Contoh: *"Aina tolong cek isi halaman 4 dan 5 saja"*:
```bash
agy-doc-extract "<path_file.pdf>" -p 4-5 -o output/doc_extract/
```

### Skenario C: Pengguna Minta Tabel Diubah ke File CSV / Excel
Contoh: *"Aina tolong convert tabel di laporan ini ke CSV"*:
```bash
agy-doc-extract "<path_file.pdf>" --mode tables --csv -o output/doc_extract/
```
Kirim file `.csv` yang dihasilkan ke pengguna WhatsApp:
```bash
python3 skills/whatsmeow/scripts/wa_tool.py send-media --to "<chat_jid>" --file output/doc_extract/table_p1_1.csv --caption "Ini tabel hasil konversinya yaa mas 📊"
```
