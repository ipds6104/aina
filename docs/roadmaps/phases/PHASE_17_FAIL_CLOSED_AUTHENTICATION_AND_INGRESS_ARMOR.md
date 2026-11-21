# 🛡️ Fase 17: Fail-Closed Web & Webhook Ingress Armor, Constant-Time Auth, and Query Secret Purge

> **Status**: READY (Aligned with ADR-014, ADR-016, RISK-15, DEBT-07, QS-13)  
> **Komponen Target**:  
> - `src/adapters/driving/web/auth.rs`  
> - `src/adapters/driving/web/mod.rs` & `src/adapters/driving/web/state.rs`  
> - `src/adapters/driving/web/controllers/{auth_setup.rs, dashboard.rs, oidc.rs, audit.rs, events.rs, schedule.rs, persona_metacog.rs, webhook.rs, simulate.rs}`  
> - `src/adapters/driving/web/ui/scripts.rs`  
> - `src/main.rs`  
> - `scripts/login_account.py`  
> **Prinsip**: *"Fail-Closed by Default. Header-First Auth. Hybrid Upstream-Compatible Webhook Armor. Zero Extra Crates."*

---

## 1. Problem Statement & Root Cause

Audit kode aktual mengungkap tujuh celah nyata pada lapisan ingress HTTP Axum:

1. **Fail-Open pada Otorisasi Web & Setup**:
   - `is_admin_authorized` (`auth.rs:63-67`) dan `is_api_authorized` (`auth.rs:12-14`) mengembalikan `true` saat `setup_code` / API key kosong.
   - `api_setup_handler` (`auth_setup.rs:163`) dan `api_set_model_handler` (`auth_setup.rs:112`) membandingkan `==`/`!=` langsung tanpa mengecek kekosongan `state.setup_code`.
   - `main.rs:165` memakai `std::env::var("ADMIN_KEY")` tanpa memfilter string kosong (`ADMIN_KEY=""` menyebabkan `setup_code = ""`), dan fallback auto-generate hanya 6 digit PIN (`AINA-xxxxxx`).
2. **Perbandingan Non-Constant-Time & Peleburan Wewenang Kunci Gateway**:
   - `auth.rs` dan `auth_setup.rs` memakai operator `==` (rentan *timing side-channel*), padahal `constant_time_eq` sudah tersedia di `oidc.rs:78`.
   - `is_api_authorized` menerima `whatsmeow_api_key` & `companion_api_key` untuk aksi admin (seperti tambah/hapus akun pool), sehingga kebocoran kunci gateway WhatsApp otomatis menjadi eskalasi admin web.
3. **Hardening Sesi OIDC (`oidc.rs`)**:
   - `resolve_session_secret` (`oidc.rs:141-149`) fallback ke `"{setup_code}:{whatsmeow_api_key}"` (bisa jatuh ke nilai default `"default-secret"` di `settings.rs:226`), membuka pemalsuan cookie `aina_session` dengan `is_admin: true`.
   - Cookie `aina_session` dan `aina_oidc_state` belum menyertakan flag `Secure` saat HTTPS, dan `OIDC_STATE_COOKIE` tidak diverifikasi pada `/auth/callback` (risiko Login CSRF).
4. **Kredensial di Query Parameter URL (`?key=`)**:
   - Admin/auth/audit endpoint dan Web UI (`ui/scripts.rs:84-1121`, `scripts/login_account.py:160`) masih mengirim kunci lewat query URL `?key=`, yang tercatat di log reverse proxy dan riwayat browser.
5. **Endpoint Diagnostik & `/setup` Belum Terlindungi**:
   - `GET /api/auth/accounts`, `/api/schedule/*`, `/api/persona/*`, `/api/metacognition/*` terbuka publik tanpa autentikasi (bahkan `/api/persona/diagnostics` & `/api/persona/journal` men-spawn subprocess `python3` per request).
   - `GET /setup` (`dashboard.rs:33-42`) belum menerapkan redirect SSO ketika `OIDC_ENABLED=true`.
6. **Celah Kritis pada `POST /webhook` (`webhook.rs`)**:
   - **SSRF + Leak API Key** (`webhook.rs:107-149`): `download_url` eksternal langsung di-`GET` sambil mengirim `Authorization: Bearer <WHATSMEOW_API_KEY>` dan `X-API-Key`.
   - **Path Traversal** (`webhook.rs:198-202`): `msg.id` ditulis langsung ke `media_dir.join(format!("{}.{}", msg.id, ext))` tanpa sanitasi.
   - Belum ada dukungan opsi autentikasi shared-secret webhook (`AINA_WEBHOOK_SECRET`) untuk deployment publik.
7. **Ketiadaan Rate Limiting pada Verifikasi Kunci Admin**:
   - `/api/setup` dan `/api/auth/verify` tidak memiliki pembatas percobaan gagal per IP yang sadar reverse proxy (Coolify/Traefik).

---

## 2. Solusi Teknis & Arsitektur (Zero New Crates)

```text
[ Incoming HTTP Request ]
           │
           ▼
[ Axum Server w/ ConnectInfo<SocketAddr> + Opt-in Proxy Header Policy ]
           │
           ├── POST /webhook ──► [ Optional AINA_WEBHOOK_SECRET (fail-closed if set) ]
           │                     [ Mandatory SSRF Origin Lock on download_url ]
           │                     [ Mandatory msg.id Whitelist + Canonical Media Path ]
           │
           ├── POST /api/setup & /api/auth/verify
           │        └──► [ AuthAttemptLimiter (std/tokio Mutex<HashMap<IpAddr, Bucket>>) ]
           │             (Max 5 gagal/menit + backoff -> 429 Too Many Requests)
           │
           ├── Admin / Auth / Audit / Diagnostics API
           │        └──► [ Fail-Closed Auth Validator ]
           │             ├── setup_code kosong? ──► REJECT 401 (kecuali OIDC Admin Session sah)
           │             ├── Query ?key= ─────────► DIABAIKAN / DITOLAK (Header / Cookie only)
           │             └── Header X-Admin-Key / Authorization: Bearer / OIDC Cookie
           │                 ──► constant_time_eq(&cand, &expected)
           │
           └── POST /api/v1/events & /api/events
                    └──► Header-first (X-Events-Key / X-API-Key / Bearer)
                         + Opt-in ?api_key= HANYA jika AINA_EVENTS_ALLOW_QUERY_KEY=true
```

### A. Fail-Closed & Constant-Time Auth Terpadu (`auth.rs`, `auth_setup.rs`, `main.rs`)
1. Promosikan `pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool` ke `src/adapters/driving/web/auth.rs` dan gunakan di seluruh evaluasi kunci (`auth.rs`, `auth_setup.rs`, `oidc.rs`).
2. **Fail-Closed Mutlak**:
   - `is_admin_authorized(headers, expected_code)`: jika `expected_code.trim().is_empty()`, wajib `return false`.
   - `is_api_authorized(headers, state)`: hapus parameter `query_key`; pisahkan kunci gateway WhatsApp dari wewenang admin web — hanya menerima `setup_code` (via `X-Admin-Key`, `X-API-Key`, `Authorization: Bearer`) atau sesi OIDC admin sah. Jika `setup_code` kosong, hanya sesi OIDC admin sah yang dapat lulus (jika tidak ada sesi OIDC -> `return false`).
   - `api_setup_handler`, `api_set_model_handler`, `api_verify_admin_handler`: gunakan helper fail-closed + `constant_time_eq` (menolak jika `state.setup_code.trim().is_empty()`).
3. **CSPRNG Setup Code (`main.rs`)**:
   - Filter `ADMIN_KEY` / `AINA_ADMIN_KEY` dengan `.ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())`.
   - Jika tidak disetel, hasilkan token 128-bit CSPRNG (`AINA-<32 hex>`) menggunakan crate `rand` yang sudah ada.

### B. Hardening OIDC Session & Cookie (`oidc.rs`, `state.rs`)
1. Tambah field `ephemeral_session_secret: String` (256-bit CSPRNG dibangkitkan sekali saat boot di `main.rs`) pada `WebhookServerState`.
2. `resolve_session_secret`: gunakan `OIDC_SESSION_SECRET` jika ada; jika tidak, gunakan `ephemeral_session_secret` (TIDAK pernah fallback ke `"default-secret"` atau `whatsmeow_api_key`).
3. Tambahkan atribut `Secure` pada cookie `aina_session` dan `aina_oidc_state` bila request datang via HTTPS (`x-forwarded-proto: https` atau `OIDC_REDIRECT_URI` berawalan `https://`).
4. Verifikasi kecocokan `OIDC_STATE_COOKIE` dengan parameter `state` pada `oidc_callback_handler` menggunakan `constant_time_eq`.

### C. Migrasi Header-Only & Kebijakan Khusus `/api/v1/events`
1. Hapus penerimaan `?key=` / `?api_key=` dari seluruh handler di `auth_setup.rs` dan `audit.rs` serta perbarui pesan error 401 agar tidak lagi menyarankan `?key=`.
2. **Migrasi Klien Bawaan (Zero Regression)**:
   - Perbarui seluruh `fetch(...)` di `src/adapters/driving/web/ui/scripts.rs` (baris 84, 182, 287, 335, 391, 561, 663, 1121) agar mengirim kunci via header `X-Admin-Key` (tanpa `?key=` di URL).
   - Perbarui `scripts/login_account.py:160` agar mengirim `X-Admin-Key` header, bukan `?key=`.
3. **Kompatibilitas `/api/v1/events` & `/api/events`**:
   - Default (`AINA_EVENTS_ALLOW_QUERY_KEY=false`): hanya menerima header (`X-Events-Key`, `X-API-Key`, `Authorization: Bearer`) dengan `constant_time_eq` dan fail-closed bila kunci kosong.
   - Jika operator mengaktifkan `AINA_EVENTS_ALLOW_QUERY_KEY=true` untuk webhook legacy (Prometheus/Grafana/Alertmanager), terima `?api_key=`/`?token=` khusus pada endpoint events dengan `warn!` log.

### D. Proteksi Endpoint Diagnostik & `/setup`
1. Lindungi dengan `is_api_authorized`:
   - `GET /api/auth/accounts` (`auth_setup.rs`)
   - `GET /api/schedule/tasks`, `/api/schedule/runs`, `/api/schedule/diagnostics` (`schedule.rs`)
   - `GET /api/persona/diagnostics`, `/api/persona/journal`, `/api/metacognition/*` (`persona_metacog.rs`)
2. Pada `GET /setup` (`dashboard.rs`), terapkan redirect OIDC `/auth/login` jika `state.oidc_config.enabled && session.is_none() && !is_api_authorized(...)` (konsisten dengan `GET /`).
3. Pertahankan `GET /health` dan `GET /api/status` (metadata publik minimal non-sensitif) agar Docker `HEALTHCHECK` dan bootstrap UI tetap bekerja mulus.

### E. Hybrid Upstream-Compatible Hardening pada `POST /webhook` (`webhook.rs`)
1. **Optional Shared-Secret Auth (`AINA_WEBHOOK_SECRET`)**:
   - Jika env `AINA_WEBHOOK_SECRET` disetel (non-empty): `webhook_handler` wajib memverifikasi header `X-Webhook-Secret` / `X-API-Key` / `Authorization: Bearer` via `constant_time_eq` (fail-closed -> `401 Unauthorized`).
   - Jika `AINA_WEBHOOK_SECRET` tidak disetel: tetap izinkan trafik masuk (kompatibel out-of-the-box dengan setup upstream Whatsmeow di jaringan internal Docker) dengan log peringatan saat startup.
2. **Mandatory SSRF Origin Lock pada `download_url`**:
   - Hanya izinkan relative path (`/api/v1/media/...` tanpa `..`) ATAU URL absolut yang scheme + host + port-nya sama persis dengan `state.whatsmeow_url` atau `state.companion_base_url`.
   - Tolak & log peringatan untuk URL host asing — **dilarang keras** mengirim `WHATSMEOW_API_KEY` ke origin yang tidak cocok.
3. **Mandatory Sanitasi `msg.id` untuk Penulisan Media**:
   - Validasi/sanitasi `msg.id` sebelum membentuk nama file (`^[a-zA-Z0-9_-]{1,128}$`, fallback hash SHA-256 prefix jika mengandung karakter non-whitelist) + pastikan `target_path` tetap berada di dalam `media_dir` kanonikal.

### F. Proxy-Aware IP Lockout untuk `/api/setup` & `/api/auth/verify`
- Jalankan server dengan `app.into_make_service_with_connect_info::<SocketAddr>()` di `main.rs`.
- Ekstrak IP klien dari `ConnectInfo<SocketAddr>`, atau dari `X-Forwarded-For` (entri paling kanan yang tak terpercaya / entri pertama) **hanya** bila `AINA_TRUST_PROXY_HEADERS=true` (untuk deployment Coolify/Traefik).
- Simpan failure bucket di `WebhookServerState` menggunakan `Arc<tokio::sync::Mutex<HashMap<IpAddr, AuthFailureWindow>>>` (maks 5 gagal/menit, lockout 60 detik, sweep entri kedaluwarsa otomatis dengan batas kapasitas map maks 10.000 entri untuk cegah OOM).

---

## 3. Rencana Pengujian & Kriteria Selesai (Definition of Done)

1. **Unit & Integration Test Fail-Closed (`cargo test`)**:
   - `is_admin_authorized(&headers, "") == false` dan `is_api_authorized` menolak saat `setup_code` kosong tanpa sesi OIDC.
   - `api_setup_handler`, `api_set_model_handler`, `api_verify_admin_handler` mengembalikan `401` bila `setup_code` kosong atau tidak cocok.
   - `whatsmeow_api_key` ditolak untuk endpoint admin web.
2. **Uji Query Parameter Purge & Events Opt-In**:
   - Request `?key=<valid>` ke `/api/audit/*` dan `/api/auth/*` mengembalikan `401`; request dengan header `X-Admin-Key` mengembalikan `200`.
   - `/api/v1/events?api_key=<valid>` ditolak saat `AINA_EVENTS_ALLOW_QUERY_KEY=false` dan diterima `202` saat opt-in `true`.
3. **Uji Proteksi Endpoint Diagnostik & OIDC Hardening**:
   - Request tanpa auth ke `/api/auth/accounts`, `/api/schedule/tasks`, `/api/persona/diagnostics`, `/api/metacognition/diagnostics` mengembalikan `401`.
   - Cookie sesi OIDC palsu ditandatangani dengan `":default-secret"` ditolak; callback dengan `OIDC_STATE_COOKIE` tidak cocok ditolak.
4. **Uji Hardening `/webhook`**:
   - Payload dengan `download_url: "http://169.254.169.254/latest/meta-data"` diblokir (0 HTTP request keluar).
   - Payload dengan `id: "../../etc/evil"` tidak pernah menulis berkas di luar `workspace/media/`.
   - Saat `AINA_WEBHOOK_SECRET` disetel, request tanpa header sah ditolak `401`; saat tidak disetel, tetap `200 OK`.
5. **Uji IP Lockout**:
   - 6 percobaan gagal berturut-turut ke `/api/auth/verify` dari IP yang sama mengembalikan `429 Too Many Requests`.
6. **Zero Regression**:
   - Seluruh `cargo check` dan `cargo test` lulus penuh tanpa warning baru dan tanpa crate baru di `Cargo.toml`.
