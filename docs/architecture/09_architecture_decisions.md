# 9. Keputusan Arsitektur (Architecture Decisions)

> [!INFO] **Architecture Provenance & Freshness**
> - **Verified Commit**: `17f951a` (branch: `main`)
> - **Verified Timestamp**: `2026-11-21T10:53:50+07:00`
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

---

### ADR-016: Fail-Closed Web Auth, Hybrid Webhook Armor, & Header-First API (Fase 17)

- **Status**: Diterima (*Accepted*)
- **Konteks**:
  Audit keamanan mendeteksi bahwa `is_admin_authorized`, `is_api_authorized`, dan beberapa handler `auth_setup.rs` bersifat fail-open saat `setup_code` kosong, memakai perbandingan string non-constant-time (`==`), menerima rahasia via query URL (`?key=`), membiarkan endpoint diagnostik (`/api/auth/accounts`, `/api/schedule/*`, `/api/persona/*`, `/api/metacognition/*`) tanpa auth, menggunakan fallback secret OIDC lemah, serta membiarkan `POST /webhook` rentan SSRF (`download_url`) dan path traversal (`msg.id`). Sebagai repo upstream minimalis, pengamanan harus tanpa crate Rust baru dan tidak memutus setup Whatsmeow internal.
- **Keputusan**:
  1. Menegakkan *Fail-Closed* + `constant_time_eq` pada seluruh fungsi otorisasi web (`auth.rs`, `auth_setup.rs`), CSPRNG setup token 128-bit (`main.rs`), pemisahan `whatsmeow_api_key` dari admin API, dan pengerasan sesi OIDC (ephemeral CSPRNG fallback secret, flag `Secure`, verifikasi `OIDC_STATE_COOKIE`).
  2. Menghapus penuh `?key=` pada semua admin/auth/audit API (disertai migrasi Web UI & `login_account.py` ke Header `X-Admin-Key`), sementara `/api/v1/events` bersifat header-first dengan opt-in eksplisit `AINA_EVENTS_ALLOW_QUERY_KEY=true` untuk webhook legacy.
  3. Menutup endpoint diagnostik & `/setup` (saat OIDC aktif) di balik autentikasi, serta menambahkan IP lockout sadar reverse proxy (`ConnectInfo` + opt-in `AINA_TRUST_PROXY_HEADERS`) pada `/api/setup` dan `/api/auth/verify`.
  4. Mengamankan `POST /webhook` secara hybrid kompatibel upstream: wajib SSRF origin-lock pada `download_url` dan sanitasi `msg.id`, sedangkan autentikasi header `AINA_WEBHOOK_SECRET` bersifat opsional (fail-closed saat disetel).
- **Konsekuensi Positif**:
  Menutup seluruh vektor bypass autentikasi web, SSRF/kebocoran API key, dan kebocoran kunci di log proxy tanpa memutus kompatibilitas upstream maupun menambah crate eksternal.
- **Risiko yang Muncul**: RISK-15

---

### ADR-017: Canonical Path Sandboxing & 100% Env-Var First Zero Hardcoded Secrets (Fase 18)

- **Status**: Diterima (*Accepted*)
- **Konteks**:
  Parameter `session_id` pada `oauth.rs` dan `oauth_helper.py` digabungkan langsung ke jalur direktori (bahkan dipanggil dengan `remove_dir_all`) tanpa sanitasi maupun canonical check. Selain itu, ditemukan kredensial plaintext `INFISICAL_CLIENT_ID`/`INFISICAL_CLIENT_SECRET` hardcoded di `skills/infisical/scripts/secret_tool.py` serta byte-array OAuth XOR (`_decode_key`) dan domain privat di `scripts/login_account.py` & `oauth_helper.py`. Image Docker upstream juga tidak memasang biner `infisical` secara wajib.
- **Keputusan**:
  1. Menegakkan validasi ganda di Rust (`oauth.rs` mengembalikan `Result<PathBuf>`) dan Python (`oauth_helper.py`): whitelist `^[a-zA-Z0-9_-]{16,64}$` dan verifikasi canonical containment (`starts_with` / `commonpath`) sebelum penulisan maupun penghapusan direktori.
  2. Menghapus 100% kredensial statis dan domain/path privat dari `secret_tool.py`, `oauth_helper.py`, `login_account.py`, dan `oauth.rs`.
  3. Mengadopsi pendekatan *100% Env-Var First* (`GOOGLE_CLIENT_ID` / `GOOGLE_CLIENT_SECRET` via `os.environ` & `Command::env`) tanpa mengimpor modul internal Infisical; brankas eksternal (Infisical/Vault/Doppler) tetap 100% opsional melalui kontrak `IF-06` (`ADR-005`).
- **Konsekuensi Positif**:
  Menutup celah path traversal & penghapusan direktori arbitrer, menghilangkan seluruh secret hardcoded dari repositori Git, dan menjaga image upstream 100% netral tanpa dependensi vault wajib.
- **Risiko yang Muncul**: RISK-16

---

### ADR-018: Dual-Layer Hexagonal WhatsApp Throttling, Nonce Prompt Boundary, & Least-Privilege Turn (Fase 19–20)

- **Status**: Diterima (*Accepted*)
- **Konteks**:
  Banjir pesan DM WhatsApp dari nomor tak dikenal dapat memicu download media/antrean tak terbatas di `webhook.rs` & `queue.rs` serta menguras `AccountPool` (429 cooldown). Di sisi lain, penempatan state rate limiter di `core/domain/gatekeeper.rs` akan melanggar `ADR-001` (Hexagonal murni) karena `Gatekeeper` stateless dan belum mengenal `Authority`, sedangkan membalas setiap pesan spam berisiko menyebabkan ban WhatsApp. Pada jalur prompt grup, riwayat ambien, *On-Demand Recall*, `quoted_message`, dan `sender.name` disisipkan tanpa nonce anti-breakout, `TurnCapability` di grup belum membatasi target mutasi user ke pesan aktif Admin, dan `adapter.rs` menyuntikkan `WHATSMEOW_API_KEY` ke subprocess agen bahkan pada turn `Guest`.
- **Keputusan**:
  1. **Dual-Layer Hexagonal Throttling (Fase 19)**: Edge burst guard & bounded `mpsc` queue di `src/adapters/driving/web/` (sebelum download media), ditambah pure domain `TokenBucket` di `src/core/domain/rate_limiter.rs` (nol `tokio`/I/O/crate baru) yang diorkestrasi di `ProcessIncomingMessageUseCase` setelah resolve `Authority`. Saat Guest melewati batas: kirim maks 1x notifikasi sopan per window, lalu silent auto-drop + isolasi sementara (*tombstone*), dicatat ke `whatsapp_action_audits`.
  2. **Nonce Prompt Boundary & Deterministic Target Binding (Fase 20)**: Bungkus seluruh data untrusted (`ambient_history`, `ON-DEMAND RECALL`, `quoted_message`, `sender.name`) dalam tag XML bernonce acak per tur (`<untrusted_ambient_history nonce="...">` & `<untrusted_recalled_history nonce="...">`) dengan escaping tag penutup; ikat `TurnCapability.explicit_targets` secara deterministik dari pesan aktif Admin agar `GrantUser`/`RevokeUser` di grup hanya sah bagi target yang secara eksplisit di-mention/disebut/di-reply di pesan aktif.
  3. **Least-Privilege Subprocess Env (Fase 20)**: Hentikan penyuntikan `WHATSMEOW_API_KEY` & `WHATSMEOW_COMPANION_KEY` ke subprocess `agy` pada turn `Authority::Guest` (dan `WHATSMEOW_COMPANION_KEY` hanya untuk `Authority::Admin`).
- **Konsekuensi Positif**:
  Mencegah DoS kuota/disk/RAM dan risiko ban WhatsApp sembari menjaga kemurnian domain `ADR-001`, serta membentengi wewenang Admin dan kredensial gateway secara deterministik terhadap *indirect prompt injection*.
- **Risiko yang Muncul**: RISK-17, RISK-18

---

### ADR-019: Non-Blocking Concurrent Scheduler, Process-Group Hard Timeout, `DirectCommand`, & Persistent Browser/CLI Runtime (Fase 22)

- **Status**: Diterima (*Accepted*)
- **Konteks**:
  Pada loop scheduler sebelumnya, `TaskExecutor::execute_due_tasks` mengeksekusi tugas jatuh tempo secara sekuensial (`for task in due_tasks`), sedangkan `AntigravityCliAdapter` menggunakan default `timeout_seconds = 0` (`--print-timeout 24h`). Ketika satu tugas terjadwal menggantung (misalnya `root agent idle; waiting up to 24h0m0s for 1 background task(s)` selama 73 menit), seluruh antrean scheduler termasuk notifikasi cepat `DirectNotification` ikut lumpuh (*head-of-line blocking*). Selain itu, tugas skrip deterministik 1 baris terpaksa melewati LLM `agy`, cache Patchright (`/root/.cache/ms-playwright`) dan symlink CLI custom skill (`/usr/local/bin/`) masih berada di layer kontainer ephemeral, serta `cp -r` pada `docker-entrypoint.sh` menimbulkan direktori bersarang rekursif saat target `.agents/skills/<name>` berupa symlink.
- **Keputusan**:
  1. **Non-Blocking Concurrent Scheduler & Anti-Overlap Guard**: `SchedulerRunner` memanggil `ScheduledTickUseCase::execute_non_blocking()` yang men-spawn tiap tugas jatuh tempo secara paralel dengan bounded semaphore (`SCHEDULER_MAX_CONCURRENT_TASKS`, default `3`), jalur cepat tanpa antre untuk `DirectNotification`, dan guard `in_flight_tasks` anti-tumpang-tindih.
  2. **Per-Task Hard Timeout & Process-Group Termination**: Memisahkan batas waktu chat interaktif (tetap unlimited) dari tugas cron otomatis melalui `SCHEDULER_TASK_TIMEOUT_SECONDS` (default `600s`) dan `SCHEDULER_COMMAND_TIMEOUT_SECONDS` (default `180s`), dijalankan dalam dedicated Unix process group (`process_group(0)`) dengan terminasi menyeluruh via POSIX `kill(-pgid, SIGKILL)` saat timeout (tanpa retry berlapis pada timeout terjadwal).
  3. **Tipe Tugas `DirectCommand` Khusus Admin**: Menambahkan varian `ScheduledTaskType::DirectCommand` (bypass LLM) yang dikunci secara fail-closed hanya untuk `Authority::Admin` di `evaluate_schedule()`, dengan laporan kegagalan eksklusif ke Admin DM.
  4. **Persistensi Runtime & Atomic Symlink**: Menetapkan `PLAYWRIGHT_BROWSERS_PATH=/app/data/ms-playwright`, `UV_CACHE_DIR=/app/data/cache/uv`, dan memasukkan `/app/data/bin` di `PATH` pada `Dockerfile` serta `docker-entrypoint.sh`; mengganti `cp -r` custom skills menjadi atomic symlink `ln -sfn`; serta menjadikan `--append` refleksi harian idempotent melalui marker `[Konteks Harian]`.
- **Konsekuensi Positif**:
  Menghilangkan *head-of-line blocking* dan subproses yatim piatu pada scheduler, memangkas biaya token & latensi tugas skrip statis hingga >90%, serta menjamin 100% persistensi browser Patchright dan CLI custom skill lintas redeploy Coolify.
- **Risiko yang Muncul**: RISK-19 (disiplin pemisahan perintah Admin pada `DirectCommand`, dimitigasi oleh gate fail-closed `only_admin_may_schedule_direct_command`)
