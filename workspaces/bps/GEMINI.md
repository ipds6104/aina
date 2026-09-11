# Workspace Domain: Badan Pusat Statistik (BPS) & Rekapitulasi Data

> [!IMPORTANT]
> Workspace ini didedikasikan khusus untuk pengolahan data statistik, penarikan data publik BPS/Satu Data Indonesia, pembersihan data (*data wrangling*), dan rekapitulasi indikator sosial-ekonomi.

---

## 1. Standar Kode Wilayah & Metadata Statistik

1. **Kode Wilayah Baku (MFD - Master File Desa BPS & Kepmendagri)**:
   - Provinsi: 2 digit (contoh: `32` Jawa Barat, `33` Jawa Tengah, `35` Jawa Timur, `31` DKI Jakarta).
   - Kabupaten/Kota: 4 digit (contoh: `3273` Kota Bandung, `3578` Kota Surabaya).
   - Kecamatan: 6 digit (contoh: `327301`).
   - Desa/Kelurahan: 10 digit (contoh: `3273011001`).
   - **Aturan**: Selalu pastikan kode wilayah berupa tipe string dengan leading zero utuh (jangan sampai angka `0` di depan hilang akibat format integer otomatis).
2. **Metadata Wajib pada Setiap Output Indikator**:
   Setiap kali menyajikan atau merekap data, selalu cantumkan elemen metadata:
   - **Nama Variabel / Indikator** (misal: Tingkat Pengangguran Terbuka / IPM / Inflasi YoY).
   - **Periode Waktu** (Tahun, Semester, atau Triwulan).
   - **Satuan Ukuran** (Persen `%`, Jiwa, Rupiah, Ton, dsb.).
   - **Sumber Survei / Publikasi** (misal: Sakernas, Susenas, PDRB BPS, atau Sensus Penduduk).

---

## 2. Format Penyajian Angka & Teks WhatsApp

1. **Format Angka Bahasa Indonesia**:
   - Pemisah ribuan menggunakan tanda titik (`.`): contoh `1.250.000`.
   - Pemisah desimal menggunakan tanda koma (`,`): contoh `7,45%`.
2. **Penyajian di WhatsApp (Anti-Tabel Berantakan)**:
   - Hindari membuat tabel Markdown panjang. Sajikan dalam bentuk poin-poin terstruktur:
     ```text
     📊 *Rekap Indikator Kemiskinan Provinsi Jawa Barat (2025)*
     • *Persentase Penduduk Miskin*: 7,25%
     • *Jumlah Penduduk Miskin*: 3.820.500 jiwa
     • *Garis Kemiskinan*: Rp 512.450 / kapita / bulan
     • _Sumber_: BPS - Survei Sosial Ekonomi Nasional (Susenas)
     ```

---

## 3. Tata Kelola Berkas Data & Skrip

1. **Direktori `data/`**:
   - Simpan dataset mentah atau hasil olahan (.csv, .xlsx, .json) di folder `data/`.
   - Gunakan encoding `UTF-8` dengan delimiter koma (`,`) atau titik koma (`;`).
2. **Direktori `scripts/`**:
   - Simpan skrip Python (pandas, openpyxl, scraper BPS API) di folder `scripts/`.
   - Selalu sertakan penanganan error jika API atau berkas CSV tidak ditemukan.

---

## 4. Perlindungan Kerahasiaan Data (UU No. 16 Tahun 1997 tentang Statistik)

- **Zero Individual Trace**: Dilarang keras menampilkan atau mengekspos data mentah responden individual (nama orang, NIK, alamat rumah spesifik, atau nomor kontak responden).
- Seluruh informasi yang disajikan ke dalam grup kerja atau chat WhatsApp harus merupakan **data agregat** (tingkat desa/kelurahan, kecamatan, kab/kota, provinsi, atau nasional).
