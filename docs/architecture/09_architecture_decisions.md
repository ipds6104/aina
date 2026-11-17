# 9. Keputusan Arsitektur (Architecture Decisions)

> [!INFO] **Architecture Provenance & Freshness**
> - **Verified Commit**: `d68eef9` (branch: `main`)
> - **Verified Timestamp**: `2026-11-17T19:02:55+07:00`
> - **Tracked Paths**: `src/core/domain/capability.rs`, `src/core/domain/audit/`, `src/adapters/driven/sqlite_store/audit.rs`
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

---

### ADR-006: Rekonsiliasi Aktivitas Hibrida & Partial Unique Index di SQLite

- **Status**: Diterima (*Accepted*)
- **Konteks**:
  Redeploy kontainer Docker/Coolify sering kali mereset jadwal yang telah diedit pengguna. Di sisi lain, default bawaan pabrik (seperti briefing pagi dan status WhatsApp) harus tetap terpasang secara otomatis jika belum ada.
- **Keputusan**:
  Menerapkan arsitektur hibrida: baseline didefinisikan secara deklaratif di `config/activities.default.json`, sedangkan modifikasi pengguna disimpan di `/app/data/activities/<slug>.json` dan basis data SQLite. Tabel `scheduled_tasks` dilengkapi kolom `slug` dan partial unique index `idx_scheduled_tasks_slug` agar rekonsiliasi bersifat idempoten tanpa duplikasi baris.
- **Konsekuensi Positif**:
  Modifikasi pengguna 100% persisten melintasi proses redeployment, dan pembaruan baseline bawaan pabrik tetap tersinkronisasi mulus.
- **Risiko yang Muncul**: RISK-06

---

### ADR-007: Three-Tier Progressive Disclosure Model (`agentskills.io`) & Auto-Discovery

- **Status**: Diterima (*Accepted*)
- **Konteks**:
  Menaruh seluruh dokumentasi keahlian ke dalam system prompt utama memicu context rot dan pemborosan kuota token yang drastis.
- **Keputusan**:
  Mengadopsi standar terbuka internasional `agentskills.io`: Tier 1 (Discovery metadata di prompt ~40 token), Tier 2 (Aktivasi pembacaan `SKILL.md` saat cocok), dan Tier 3 (Eksekusi skrip Python via subproses bash dengan output format JSON murni). Folder `/app/data/custom-skills` ditautkan ke `.agents/skills` via symlink permanen untuk hot auto-discovery tanpa perlu me-restart kontainer.
- **Konsekuensi Positif**:
  Jendela konteks AI tetap bersih dan tajam, waktu eksekusi instan, serta skalabilitas puluhan skill tanpa degradasi penalaran.
- **Risiko yang Muncul**: RISK-07

---

### ADR-008: Siklus Refleksi Harian 5-Tahap & Grounding Fisik Tanpa Halusinasi

- **Status**: Diterima (*Accepted*)
- **Konteks**:
  Asisten otonom yang menjalankan jadwal rutin rentan mengulang-ulang pola yang sama (*monoton*), atau mengarang kondisi cuaca dan destinasi lokal tanpa dasar fakta riil.
- **Keputusan**:
  Membangun siklus refleksi mandiri 5-tahap (*Gibbs Reflective Cycle* + *Stanford Generative Agents*). Aina mengaudit eksekusi hariannya, mendeteksi *boredom score*, mengambil cuaca faktual Pontianak via Open-Meteo API, merumuskan 1–2 kueri riset web terarah (*gated novelty search*), dan mengeksekusi penyesuaian instruksi aktivitas secara nyata via CLI `aina activity edit --file`.
- **Konsekuensi Positif**:
  Aina menjadi rekan kerja yang hidup, kontekstual, dan dinamis, terbebas dari halusinasi lingkungan khatulistiwa.
- **Risiko yang Muncul**: RISK-08

---

### ADR-009: Zero Implicit Trust, Hirarki Otoritas 4-Tingkat, & Scoped Turn Capabilities

- **Status**: Diterima (*Accepted*)
- **Konteks**:
  Pesan WhatsApp dari sembarang nomor berpotensi memanipulasi kepribadian AI melalui rekayasa prompt (*prompt injection*), meminta data internal kantor, atau berpura-pura menjadi pimpinan satker.
- **Keputusan**:
  Menerapkan paradigma *Zero Implicit Trust* dengan hirarki 4-tingkat (`Admin`, `Atasan`, `Staff`, `Guest`). Otoritas `Admin` terisolasi 100% pada konfigurasi statis `admin_jid` (nomor Companion). Setiap giliran pemrosesan menerbitkan tiket kapabilitas bertenggat waktu (`TurnCapability`, TTL 10 menit). Seluruh perintah administratif dan pemanggilan alat istimewa wajib memvalidasi tiket kapabilitas ini.
- **Konsekuensi Positif**:
  Kebal terhadap serangan eskalasi wewenang via teks obrolan, aman saat nomor WhatsApp bot bocor ke publik, dan terjaminnya kepatuhan *Least Privilege*.
- **Risiko yang Muncul**: RISK-09

---

### ADR-010: Rantai Hash Kriptografis SHA-256 untuk Audit Trail Non-Repudiasi Tool

- **Status**: Diterima (*Accepted*)
- **Konteks**:
  Di lingkungan instansi pemerintah (BPS), setiap tindakan yang diambil oleh AI atas permintaan pengguna harus dapat dipertanggungjawabkan tanpa sangkalan (*non-repudiation*), serta tahan terhadap manipulasi basis data lokal.
- **Keputusan**:
  Mengimplementasikan pencatatan eksekusi tool di tabel `agent_tool_executions` dengan rantai hash kriptografis SHA-256 bergaya blockchain ($H_n = \text{SHA256}(H_{n-1} \parallel \text{fields})$) yang selaras dengan NIST SP 800-53 AU-10 dan ISO/IEC 42001. Menyediakan CLI `aina audit verify-chain` untuk audit deterministik dan `aina audit trace --sender <jid>` untuk audit per nomor pengirim.
- **Konsekuensi Positif**:
  Bukti matematis anti-manipulasi yang tak terbantahkan, transparansi akuntabilitas penuh, dan kesiapan audit hukum forensik.
- **Risiko yang Muncul**: RISK-10

---

### ADR-011: Hak Privasi Mandiri Pengguna (*Self-Service Privacy & Right to be Forgotten*)

- **Status**: Diterima (*Accepted*)
- **Konteks**:
  Penyimpanan profil lawan bicara secara permanen tanpa mekanisme kendali pengguna melanggar UU No. 27 Tahun 2022 tentang Pelindungan Data Pribadi (UU PDP Indonesia) dan EU GDPR Article 17 (*Right to Erasure*).
- **Keputusan**:
  Menyediakan perintah mandiri langsung di WhatsApp: `/profilku` untuk inspeksi data profil personal transparan (dengan penyamaran nomor telepon), serta `/lupakansaya` untuk pembersihan profil dan preferensi pengguna dari basis data SQLite secara instan, dengan tetap mempertahankan log audit forensik legal secara terpisah.
- **Konsekuensi Positif**:
  Kepatuhan regulasi UU PDP 100%, peningkatan kepercayaan pengguna, dan kendali privasi mandiri (*self-sovereign privacy*).
- **Risiko yang Muncul**: RISK-11
---

### ADR-012: Siklus Hidup Identitas 3-Tingkat & Pertahanan Anti-Impersonasi Profiler

- **Status**: Diterima (*Accepted*)
- **Konteks**:
  Pihak asing atau nomor baru dapat mencoba rekayasa sosial dengan mengaku sebagai admin, rekan kerja tepercaya, atau pimpinan satker ("Saya Kepala BPS") melalui teks obrolan WhatsApp.
- **Keputusan**:
  Menerapkan siklus hidup identitas 3-tingkat (`UNCONFIRMED`, `TENTATIVE`, `VERIFIED`). Klaim nama terlindungi (*Protected Names*) dan klaim hirarki otomatis diisolasi ke kolom `claimed_name` dan `claimed_role` dengan status `UNCONFIRMED`. Evaluasi kepatuhan pimpinan `is_atasan` wajib bernilai `false` hingga Companion melakukan verifikasi sah melalui perintah CLI `aina user verify`.
- **Konsekuensi Positif**:
  Kebal terhadap rekayasa profil sosial, integritas wewenang terjamin, dan adanya pengawasan administratif eksplisit (*auditable verification*).
- **Risiko yang Muncul**: RISK-12

---

### ADR-013: Skalabilitas Konkurensi Kepadatan Tinggi & Token Lease Aware

- **Status**: Diterima (*Accepted*)
- **Konteks**:
  Ketika 50 hingga 100 pengguna mengirim pesan WhatsApp secara bersamaan, proses agen biner `agy` dapat menghabiskan memori RAM server (*OOM*) atau memicu pembatasan laju panggilan upstream Google (*HTTP 429*).
- **Keputusan**:
  Menerapkan *Bounded Concurrency Semaphore* (kapasitas default 8) via Tokio dan *Token-Aware Account Lease* yang membatasi maksimal 2 *in-flight requests* per akun Google secara atomik melalui RAII guard `AccountLease`.
- **Konsekuensi Positif**:
  Server stabil bebas OOM, antrean pesan tertib, dan bebas dari lonjakan error batas kuota 429.
- **Risiko yang Muncul**: RISK-13

---

### ADR-014: Integrasi Single Sign-On (SSO) OpenID Connect (OIDC) Agnostik Kerangka Kerja

- **Status**: Diterima (*Accepted*)
- **Konteks**:
  Akses ke antarmuka web dashboard dan simulator (`/`, `/setup`, `/api/simulate`) membutuhkan mekanisme autentikasi terpusat berstandar industri untuk karyawan dan administrator satker (misal BPS Mempawah via rIDM). Namun sebagai kerangka kerja upstream sumber terbuka, Aina dilarang memuat dependensi keras (*hardcoded*) terhadap domain, instansi, atau penyedia identitas tertentu.
- **Keputusan**:
  Mengimplementasikan klien OpenID Connect (OIDC) Authorization Code Flow dengan PKCE (RFC 7636) dan session cookie bertanda tangan kriptografis HMAC-SHA256 (RFC 2104) yang 100% agnostik. Seluruh parameter identitas dikonfigurasikan via variabel lingkungan (`OIDC_ENABLED`, `OIDC_ISSUER_URL`, `OIDC_CLIENT_ID`, `OIDC_CLIENT_SECRET`, `OIDC_REDIRECT_URI`, `OIDC_PROVIDER_NAME`, `OIDC_ADMIN_USERS`, `OIDC_ALLOWED_DOMAINS`). Di lingkungan deployment BPS Mempawah, Aina dihubungkan ke penyedia identitas rIDM (`https://auth.dvlpid.my.id/t/bps`) menggunakan client `aina-dashboard`.
- **Konsekuensi Positif**:
  Integrasi SSO mulus bagi pegawai instansi tanpa kata sandi manual, keamanan sesi berbasis HMAC-SHA256 tanpa dependensi library eksternal berlebih, dan kode sumber upstream tetap portabel untuk Keycloak, Authentik, Okta, maupun Google Workspace.
- **Risiko yang Muncul**: RISK-14

---

### ADR-015: Standar Zero-Emoticon dan Higienitas Desain UI Dasbor Web

- **Status**: Diterima (*Accepted*)
- **Konteks**:
  Dasbor web Aina (`/`) sebelumnya memuat berbagai karakter unicode emoticon dan emoji (`🌸`, `🟢`, `🛡️`, `🚀`, `🔒`, dll.) pada tajuk, status badge, pesan dialog, opsi select, dan simulator. Hal ini menimbulkan inkonsistensi rendering antar sistem operasi/font, menurunkan kesan enterprise profesional, dan melanggar panduan *UI Design Hygiene*.
- **Keputusan**:
  Menerapkan kebijakan *Zero-Emoticon* 100% pada seluruh antarmuka web Aina (`src/adapters/driving/web/ui/` dan error handler `oidc.rs`):
  1. Seluruh emoji dihapus atau digantikan dengan teks semantik deterministik, CSS status pills (misal `ENGINE ONLINE`, `Aktif`, `[Perhatian]`), atau ikon vektor SVG inline standar.
  2. Tajuk bot distandarisasi menjadi teks tipografi bersih: `Aina (あいな)` tanpa bunga/simbol unicode.
  3. Respons pesan internal simulator (seperti `/reset` dan `/model`) dibersihkan dari emoji teks.
- **Konsekuensi Positif**:
  Tampilan visual sangat rapi, profesional, konsisten di seluruh browser dan platform tanpa ketergantungan paket font emoji OS, serta mematuhi standar higienitas antarmuka perusahaan.
- **Risiko yang Muncul**: Tidak ada.
