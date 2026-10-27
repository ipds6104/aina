# 2. Batasan Arsitektur (Architecture Constraints)

> [!INFO] **Architecture Provenance & Freshness**
> - **Verified Commit**: `e9b88a2` (branch: `main`)
> - **Verified Timestamp**: `2026-10-26T11:19:12+07:00`
> - **Tracked Paths**: `src/`, `config/`, `skills/`
> - **Drift Status**: `verified`

Bab ini merangkum batas-batas teknis, operasional, dan aturan baku yang menjadi pagar pembatas dalam pengembangan Aina. Batasan ini dirancang agar sistem tetap ringan, aman, dan mudah dipindahkan ke server mana pun.

---

## 2.1 Batasan Teknis (Technical Constraints)

1. **Bahasa Pemrograman Rust (Edisi 2024)**:
   - Seluruh logika inti aplikasi dibangun menggunakan bahasa **Rust**.
   - *Alasan*: Menghindari kebocoran memori (*memory safety*), tidak memerlukan proses pembersihan sampah berkala (*zero garbage collection*), dan sangat hemat RAM (hanya memakai memori sekitar 20–35 MB saat berjalan).

2. **Server Web Asinkron (Axum & Tokio)**:
   - Server web menggunakan kerangka kerja **Axum** di atas runtime asinkron **Tokio**.
   - Mampu menangani ribuan antrean pesan masuk dari webhook WhatsApp secara bersamaan tanpa membuat server macet (*non-blocking*).

3. **Penyimpanan Mandiri (SQLite WAL & FTS5)**:
   - Seluruh data percakapan, status sesi, dan profil pengguna disimpan di dalam satu file database lokal **SQLite** menggunakan mode penulisan cepat (*Write-Ahead Logging / WAL*).
   - Dilengkapi fitur pencarian teks penuh (*Full-Text Search / FTS5*) bawaan.
   - *Keuntungan*: Pengguna tidak perlu memasang database server terpisah yang berat (seperti PostgreSQL atau MySQL). Cukup satu berkas `aina.db`.

4. **Gerbang WhatsApp (Whatsmeow Gateway)**:
   - Koneksi ke jaringan WhatsApp ditangani oleh layanan gerbang mandiri berbasis **Whatsmeow** yang berkomunikasi dengan Aina melalui protokol HTTP REST standar.
   - Aina tidak menangani enkripsi soket WhatsApp secara langsung, melainkan menyerahkannya pada Whatsmeow.

5. **Mesin Agen AI Eksternal (Google Antigravity CLI / `agy`)**:
   - Pemikiran cerdas, pemilihan alat (*tool selection*), dan eksekusi skrip diserahkan kepada biner resmi Antigravity CLI (`agy`).
   - Eksekusi kode dibatasi hanya di dalam folder kerja khusus (*sandbox workspace*) agar tidak merusak sistem server utama.

6. **Lingkungan Server & Kontainer Ringan**:
   - Sistem dapat dijalankan langsung di Linux fisik (komputer x86_64 atau ARM64 seperti smartphone via Termux) maupun di dalam kontainer Docker / Coolify.
   - Ukuran citra kontainer dijaga sangat kecil (< 50 MB) dengan proses kompilasi *multi-stage build*.

---

## 2.2 Batasan Organisasi dan Operasional (Operational Constraints)

1. **Prinsip Netralitas Inti Program (Core Neutrality)**:
   - Repositori utama Aina adalah perangkat lunak umum yang bebas dari keterikatan organisasi atau kantor tertentu.
   - **Aturan Baku**: Dilarang memasukkan skrip atau prosedur kerja khusus instansi (misalnya kueri basis data sensus, format surat dinas khusus, atau aplikasi internal kantor) ke dalam folder kode utama. Seluruh kebutuhan khusus wajib diletakkan di folder keahlian tambahan (`data/custom-skills`).

2. **Larangan Keras Menyimpan Kunci Rahasia Statis**:
   - Dilarang keras menuliskan password, token API, atau kunci akses apa pun di dalam kode sumber program maupun file teks biasa.
   - Semua rahasia wajib disuntikkan secara dinamis saat alat dijalankan (*Runtime Secret Broker*).

3. **Standar Waktu Resmi (WIB / UTC+7)**:
   - Seluruh pencatatan waktu, jadwal pengingat, dan pemahaman hari dihitung berdasarkan zona waktu **WIB (Waktu Indonesia Barat / Asia/Jakarta, UTC+7)** dengan penanggalan bahasa Indonesia.

4. **Adab dan Privasi Ruang Obrolan**:
   - Aina mematuhi aturan privasi ketat: pesan pribadi dari kontak lain di akun sensor bayangan tidak pernah dicatat.
   - Di grup obrolan, Aina hanya berstatus menyimak pasif (*RecordOnly*) dan tidak boleh mengirim balasan kecuali jika dipanggil secara sengaja oleh anggota grup.
