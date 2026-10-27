# 9. Keputusan Arsitektur (Architecture Decisions)

> [!INFO] **Architecture Provenance & Freshness**
> - **Verified Commit**: `e9b88a2` (branch: `main`)
> - **Verified Timestamp**: `2026-10-26T11:19:12+07:00`
> - **Tracked Paths**: `src/`, `config/`, `skills/`
> - **Drift Status**: `verified`

Bab ini mencatat keputusan-keputusan arsitektur penting (*Architecture Decision Records / ADR*) yang telah disepakati dan diimplementasikan di dalam sistem Aina.

---

### ADR-001: Arsitektur Heksagonal (Ports & Adapters) Murni di Rust

- **Status**: Diterima (*Accepted*)
- **Konteks**:
  Aina harus bisa berjalan stabil bertahun-tahun, mudah diuji, dan tidak boleh terikat mati (*vendor lock-in*) pada satu pustaka web tertentu atau satu penyedia WhatsApp tertentu.
- **Keputusan**:
  Mengadopsi pola arsitektur heksagonal murni. Logika bisnis di `src/core/` dilarang mengimpor library luar seperti Axum atau Rusqlite. Semua akses ke sistem luar dihubungkan melalui kontrak trait abstrak (*Ports*).
- **Konsekuensi Positif**:
  Logika inti sangat bersih, mudah diuji secara independen, dan adapter luar dapat diganti sewaktu-waktu tanpa merusak aturan bisnis.
- **Risiko yang Muncul**: RISK-01

---

### ADR-002: Dual-WhatsApp Pipeline (Nomor Bot Resmi + Companion Sensor Bayangan)

- **Status**: Diterima (*Accepted*)
- **Konteks**:
  Meminta admin grup kantor memasukkan nomor bot AI baru sering kali memakan waktu lama, ditolak, atau dilarang oleh regulasi privasi instansi.
- **Keputusan**:
  Menerapkan arsitektur jalur ganda: mendukung nomor bot resmi untuk jalur umum/terbuka, sekaligus mendukung akun WhatsApp pribadi pengguna (*User Companion*) sebagai sensor bayangan yang menyimak grup kantor secara pasif dengan prinsip nol kebocoran privasi.
- **Konsekuensi Positif**:
  Aina dapat langsung membantu pengguna di berbagai grup kantor tempat pengguna sudah bergabung, tanpa perlu menambah kontak bot baru.
- **Risiko yang Muncul**: RISK-02

---

### ADR-003: Kolam Akun Google Antigravity (Multi-Account Pool) dengan Rotasi Kuota

- **Status**: Diterima (*Accepted*)
- **Konteks**:
  Pemakaian intensif model AI pintar sering terhenti di jam kerja karena kuota per akun terkena batas harian (*rate limit HTTP 429*).
- **Keputusan**:
  Membangun sistem pengelola kolam akun (*Account Pool*). Aina memutar akun Google secara bergiliran (*Round-Robin*) pada setiap sesi, dan secara otomatis beralih ke akun cadangan (*Auto-Failover*) jika akun saat ini kehabisan kuota.
- **Konsekuensi Positif**:
  Layanan AI dapat beroperasi 24 jam nonstop tanpa gangguan kegagalan pesan ke pengguna.
- **Risiko yang Muncul**: RISK-03

---

### ADR-004: Pemisahan Inti Netral (Core Upstream) vs Keahlian Khusus (Downstream Custom Skills)

- **Status**: Diterima (*Accepted*)
- **Konteks**:
  Pengguna sering kali membutuhkan alat kerja yang sangat spesifik untuk kantornya (misalnya kueri basis data sensus BPS, format nota dinas, atau aplikasi presensi kantor). Memasukkan kode ini ke repo publik membuat kode berantakan dan membocorkan alur internal kantor.
- **Keputusan**:
  Repositori utama Aina wajib 100% netral dan agnostik organisasi. Seluruh keahlian khusus kantor diisolasi di folder persisten `data/custom-skills` yang dapat dipasang langsung dari repositori Git pribadi menggunakan perintah `aina skill install`.
- **Konsekuensi Positif**:
  Kode inti tetap bersih dan bisa dipakai oleh siapa saja, sementara kebutuhan spesifik kantor tetap terlindungi di repositori privat masing-masing tim.
- **Risiko yang Muncul**: RISK-04

---

### ADR-005: Kurir Kunci Rahasia Tepat Waktu (Runtime Secret Broker) Tanpa Kunci Statis

- **Status**: Diterima (*Accepted*)
- **Konteks**:
  Penyimpanan password dan kunci API di file konfigurasi teks biasa sangat rentan bocor saat kode dicadangkan atau dibagikan.
- **Keputusan**:
  Menggunakan pola *Runtime Secret Broker* melalui alat `secret_tool`. Kunci akses hanya disuntikkan ke memori saat alat dijalankan (*Just-in-Time*), mendukung brankas Infisical, HashiCorp Vault, maupun file lokal terenkripsi.
- **Konsekuensi Positif**:
  Keamanan maksimal, bebas keterikatan penyedia brankas tertentu, dan tidak ada kunci rahasia yang tertinggal di hard disk atau pesan obrolan.
- **Risiko yang Muncul**: RISK-05
