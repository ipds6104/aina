# 10. Persyaratan Mutu (Quality Requirements)

> [!INFO] **Architecture Provenance & Freshness**
> - **Verified Commit**: `e9b88a2` (branch: `main`)
> - **Verified Timestamp**: `2026-10-26T11:19:12+07:00`
> - **Tracked Paths**: `src/`, `config/`, `skills/`
> - **Drift Status**: `verified`

Bab ini memuat skenario-skenario mutu terukur untuk menguji dan membuktikan bahwa sasaran kualitas yang ditetapkan pada Bab 1 benar-benar terpenuhi secara nyata.

---

## 10.1 Pohon Mutu (Quality Tree)

Hubungan antara sasaran mutu utama dan skenario pengujian dipetakan dalam pohon mutu berikut:

- Kualitas Sistem Aina
  ├── Ketersediaan & Keandalan
  │   └── **QS-01**: Rotasi kuota otomatis saat batas harian 429 tercapai (#reliability)
  ├── Privasi & Keamanan
  │   └── **QS-02**: Nol kebocoran pesan pribadi dan penyuntikan kunci JIT (#security)
  ├── Kemudahan Perawatan & Modul
  │   └── **QS-03**: Pemasangan skill Git kustom tanpa menyentuh kode inti (#maintainability)
  ├── Efisiensi & Kecepatan Respon
  │   └── **QS-04**: Pencarian memori riwayat chat selesai dalam < 5 milidetik (#efficiency)
  └── Kenyamanan & Tata Krama Pengguna
      └── **QS-05**: Format teks ramah ponsel dan klarifikasi terarah (#usability)

---

## 10.2 Skenario Mutu Terukur (Quality Scenarios)

### QS-01: Rotasi Kuota Otomatis saat Terkena Limit 429
- **Kategori Mutu**: #reliability
- **Kondisi Awal**: Sistem berjalan dengan 3 akun Google yang terdaftar di Account Pool. Akun pertama telah mencapai batas kuota harian.
- **Pemicu**: Pengguna mengirim pesan pertanyaan baru di grup WhatsApp.
- **Respon Sistem**: `AntigravityCliAdapter` mendeteksi error HTTP 429 dari akun pertama, mencatat status limit, dan otomatis memindahkan sesi ke akun kedua dalam waktu < 2 detik.
- **Ukuran Keberhasilan**: Pengguna menerima jawaban lengkap di WhatsApp tanpa melihat pesan gagal (*zero failure rate*).

---

### QS-02: Perlindungan Privasi Pesan Pribadi Sensor Bayangan
- **Kategori Mutu**: #security
- **Kondisi Awal**: Sesi sensor bayangan (*User Companion*) aktif terhubung ke akun WhatsApp pribadi pengguna.
- **Pemicu**: Teman atau keluarga mengirim pesan pribadi (DM) ke akun WhatsApp pengguna.
- **Respon Sistem**: Logika `Gatekeeper` langsung mengabaikan pesan tersebut (100% diabaikan). Pesan tidak disimpan ke database dan tidak pernah diteruskan ke model AI.
- **Ukuran Keberhasilan**: 0% obrolan pribadi non-bot masuk ke database SQLite atau bocor ke ruang publik.

---

### QS-03: Pemasangan Skill Eksternal Tanpa Mengubah Kode Inti
- **Kategori Mutu**: #maintainability
- **Kondisi Awal**: Pengguna ingin menambahkan alat pengolah data khusus kantor yang disimpan di repositori Git privat.
- **Pemicu**: Pengguna menjalankan perintah `aina skill install https://github.com/kantor/skill-data.git`.
- **Respon Sistem**: Sistem mengunduh repositori ke folder `data/custom-skills/`, memvalidasi file `SKILL.md`, dan langsung mendaftarkan alat tersebut ke dalam katalog agen.
- **Ukuran Keberhasilan**: Fitur baru langsung dapat digunakan dalam obrolan tanpa perlu kompilasi ulang kode Rust inti ataupun me-restart kontainer.

---

### QS-04: Kecepatan Pencarian Riwayat Obrolan Masa Lalu
- **Kategori Mutu**: #efficiency
- **Kondisi Awal**: Database SQLite telah menampung lebih dari 50.000 pesan obrolan grup selama 6 bulan terakhir.
- **Pemicu**: Pengguna meminta Aina mengingat notulen rapat minggu lalu: `aina archive search "rapat evaluasi" --since 7d`.
- **Respon Sistem**: Mesin pencari SQLite FTS5 melakukan pencarian indeks teks penuh dan penyaringan tanggal secara lokal.
- **Ukuran Keberhasilan**: Hasil pencarian ditemukan dan disajikan dalam waktu < 5 milidetik dengan penggunaan CPU < 5%.

---

### QS-05: Format Teks Sesuai Layar Ponsel & Verifikasi Tabayyun
- **Kategori Mutu**: #usability
- **Kondisi Awal**: Pengguna meminta ringkasan perbandingan data yang biasanya disajikan dalam bentuk tabel besar.
- **Pemicu**: Pengguna mengirim instruksi ringkas di WhatsApp.
- **Respon Sistem**: Aina menyusun balasan menggunakan poin-poin tebal bertitik (`•`), huruf tebal native WhatsApp (`*tebal*`), dan menghindari tabel Markdown lebar. Bila instruksi kurang jelas, Aina terlebih dahulu menanyakan maksud pengguna dengan sopan (*Tabayyun*).
- **Ukuran Keberhasilan**: Teks balasan nyaman dibaca di layar ponsel tanpa terpotong horizontal, dan tidak ada kesalahpahaman instruksi.

---

## 10.3 Matriks Ringkasan Skenario Mutu

| ID Skenario | Sasaran Mutu | Target Ukuran | Kondisi Terverifikasi Saat Ini |
|:---|:---|:---|:---|
| **QS-01** | #reliability | Waktu failover < 2 detik, 0 kegagalan pesan | Lulus (Terverifikasi di unit test `account_pool.rs`) |
| **QS-02** | #security | 100% pesan pribadi sensor diabaikan | Lulus (Terverifikasi di `process_message/context_preparer.rs`) |
| **QS-03** | #maintainability | Waktu pasang skill < 10 detik tanpa restart | Lulus (Terverifikasi di `skill_scaffolder.py`) |
| **QS-04** | #efficiency | Waktu pencarian FTS5 < 5 milidetik | Lulus (Terverifikasi di `sqlite_store/session.rs`) |
| **QS-05** | #usability | 0 tabel rusak di WhatsApp, klarifikasi terarah | Lulus (Terverifikasi di `domain/persona.rs`) |
