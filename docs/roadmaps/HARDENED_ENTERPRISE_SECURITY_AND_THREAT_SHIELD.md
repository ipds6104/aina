# 🛡️ Master Roadmap: Hardened Enterprise Security & Threat Shield Architecture

> **Status**: APPROVED & CANONICAL (Aligned with arc42 & Upstream Minimal-Dependency Doctrine)  
> **Target Release**: Aina v0.3.0 *(baseline saat ini: `Cargo.toml` v0.2.0)*  
> **Prinsip Utama**: *"Fail Closed by Default. Minimal Mandatory Dependencies. Pure Hexagonal Boundaries. Zero Implicit Ambient Trust."*  
> **Pilar Arsitektur**: Fail-Closed Web & Webhook Armor, Canonical Path Sandboxing & Env-Var First Secrets, Dual-Layer Hexagonal WhatsApp Throttling, Nonce Prompt Isolation & Least-Privilege Turn Binding.

---

## Executive Summary & Latar Belakang Masalah

Aina adalah repositori **upstream self-hosted AI Co-Worker** yang dirancang tetap ringan, portabel, dan **seminimal mungkin dependensi wajib** (0 crate Rust tambahan, 0 kewajiban external vault/OIDC/database eksternal, siap pakai di Docker/Coolify maupun Linux lokal).

Berdasarkan audit keamanan mendalam (Cloudflare Security Audit Harness + audit struktural lintas-arc42 terhadap kode aktual), inti Aina telah memiliki pondasi kuat pada `TurnCapability`, rantai audit SHA-256 (`agent_tool_executions`), serta isolasi profil 3-tingkat. Namun, terdapat beberapa celah permukaan masuk (*ingress surface*), penanganan rahasia & jalur berkas, pembatasan laju WhatsApp, serta isolasi konteks ambien yang perlu dikeraskan secara deterministik tanpa melanggar kemurnian arsitektur heksagonal (`ADR-001`) maupun netralitas upstream (`ADR-004`, `ADR-005`):

1. **Celah Autentikasi Fail-Open, Query-Param Leak, & Permukaan Ingress Web/Webhook**:
   - `is_admin_authorized` (`auth.rs:63-67`) dan `is_api_authorized` (`auth.rs:12-14`) bersifat **fail-open** (`return true`) ketika `setup_code` / kunci API kosong. Hal serupa terjadi pada `api_setup_handler` dan `api_set_model_handler` di `auth_setup.rs`.
   - `main.rs:165` tidak menolak `ADMIN_KEY=""` (kosong), dan fallback-nya hanya 6 digit PIN (`AINA-xxxxxx`).
   - Perbandingan kunci masih memakai `==`/`!=` (rentan *timing side-channel*), dan banyak endpoint admin/audit/UI masih menerima rahasia via query URL (`?key=`).
   - `oidc.rs` fallback session secret ke `"{setup_code}:{whatsmeow_api_key}"` (rentan pemalsuan cookie sesi jika default/kosong), tanpa flag `Secure` dan tanpa verifikasi `OIDC_STATE_COOKIE`.
   - Beberapa endpoint diagnostik/manajemen (`/api/auth/accounts`, `/api/schedule/*`, `/api/persona/*`, `/api/metacognition/*`) masih terbuka tanpa autentikasi, termasuk yang men-spawn subprocess `python3` per request.
   - `POST /webhook` (`webhook.rs`) rentan **SSRF + kebocoran `WHATSMEOW_API_KEY`** melalui `download_url` eksternal, **path traversal** via `msg.id`, dan belum menyediakan opsi autentikasi shared-secret `AINA_WEBHOOK_SECRET`.

2. **Celah Path Traversal & Hardcoded Secrets pada Skrip Pendukung**:
   - `get_oauth_session_dir` (`src/adapters/driven/agy_cli/oauth.rs`) dan `get_base_dir` (`scripts/oauth_helper.py`) menggabungkan `session_id` mentah tanpa validasi ketat maupun verifikasi canonical root — berisiko penulisan maupun penghapusan (`remove_dir_all`) direktori arbitrer.
   - `skills/infisical/scripts/secret_tool.py` (baris 109–110) masih menyimpan **`INFISICAL_CLIENT_ID` & `INFISICAL_CLIENT_SECRET` plaintext hardcoded** beserta domain/project ID privat (pelanggaran langsung Bab 2.2 & ADR-004/005).
   - `scripts/login_account.py` dan `scripts/oauth_helper.py` menyimpan kredensial OAuth Google ter-obfuscate XOR (`_decode_key`) beserta URL privat hardcoded.

3. **Potensi Kehabisan Kuota & Disk/RAM DoS via WhatsApp Direct Message (Pre-Auth Flood)**:
   - `webhook.rs` mengunduh media ke disk dan memasukkan pesan ke `UnboundedSender` sebelum evaluasi otoritas, dan `Gatekeeper::evaluate_primary_bot` menerima seluruh DM dari nomor asing tanpa throttling.
   - Banjir spam DM dapat memenuhi disk/antrean serta memicu eksekusi `agy` massal yang mengunci seluruh akun di `AccountPool` ke status cooldown 429.

4. **Potensi Indirect Prompt Injection di Grup & Over-Privileged Subprocess Environment**:
   - `context_preparer.rs` menyuntikkan 10 pesan riwayat grup terakhir, hasil *On-Demand Recall*, `quoted_message`, dan `sender.name` tanpa delimiter nonce yang kebal tag breakout.
   - Saat Admin menandai `@Aina` di grup, `TurnCapability` Admin diterbitkan dan `aina policy grant-user` dapat dipanggil untuk target JID mana pun meski hanya berasal dari riwayat ambien tamu.
   - `adapter.rs:252-259` menyuntikkan `WHATSMEOW_API_KEY` dan `WHATSMEOW_COMPANION_KEY` ke environment subprocess `agy --dangerously-skip-permissions` **bahkan pada turn `Authority::Guest`**.

Roadmap ini menyelesaikan seluruh celah di atas melalui **Fase 17 hingga Fase 20** dengan **0 crate Rust baru** dan **0 layanan eksternal wajib**.

---

## 🏛️ Arsitektur 4 Pilar Solusi Keamanan (Selaras arc42)

```text
┌─────────────────────────────────────────────────────────────────────────────────────────────┐
│                AINA UPSTREAM THREAT SHIELD (ZERO EXTRA CRATES / ZERO LOCK-IN)               │
├──────────────────────────────────────────────┬──────────────────────────────────────────────┤
│  PILAR 1: FAIL-CLOSED WEB & WEBHOOK ARMOR    │  PILAR 2: CANONICAL PATH & ENV-FIRST SECRETS │
│  • Fail-Closed + constant_time_eq semua gate │  • Purge hardcoded secrets di secret_tool.py │
│  • CSPRNG setup token & OIDC secret harden   │  • Purge XOR _decode_key (Env-Var First)     │
│  • Purge ?key= di admin API + migrasi Web UI │  • Infisical/Vault tetap 100% opsional       │
│  • Auth endpoint diagnostik & /setup SSO     │  • Strict [a-zA-Z0-9_-]{16,64} + canonical   │
│  • /webhook SSRF-lock, msg.id, opt-in secret │  • Validasi ganda Rust (oauth.rs) & Python   │
│  • Proxy-aware IP lockout /api/setup & verify│  • Zero hardcoded tenant domain/local paths  │
├──────────────────────────────────────────────┼──────────────────────────────────────────────┤
│  PILAR 3: DUAL-LAYER HEXAGONAL THROTTLING    │  PILAR 4: NONCE BOUNDARY & LEAST PRIVILEGE   │
│  • Edge pre-queue guard & media cap (web/)   │  • Per-turn random nonce XML + tag escaping  │
│  • Pure domain TokenBucket (core/domain/)    │  • Cakup ambient, recall, quote, sender.name │
│  • UseCase orchestration pasca-resolve auth  │  • Deterministic Target Binding di grup      │
│  • 1x polite notice/window -> silent drop    │  • Stop suntik WHATSMEOW_API_KEY turn Guest  │
│  • Bounded Map eviction & audit di SQLite    │  • Pisahkan gateway key dari web admin auth  │
└──────────────────────────────────────────────┴──────────────────────────────────────────────┘
```

---

## 🗺️ Roadmap Fase Implementasi & Pemetaan arc42

| Fase | Spesifikasi | Fokus Utama & Deliverables | Pemetaan arc42 | Status |
| :---: | :--- | :--- | :--- | :---: |
| **Fase 17** | [`PHASE_17_FAIL_CLOSED_AUTHENTICATION_AND_INGRESS_ARMOR.md`](phases/PHASE_17_FAIL_CLOSED_AUTHENTICATION_AND_INGRESS_ARMOR.md) | Fail-closed auth & `constant_time_eq`, CSPRNG setup code, OIDC secret & cookie hardening, auth endpoint diagnostik, migrasi UI ke Header (purge `?key=`, opt-in `/api/v1/events`), `/webhook` SSRF-lock + sanitasi `msg.id` + opsional `AINA_WEBHOOK_SECRET`, proxy-aware IP lockout | ADR-014, ADR-016, RISK-15, DEBT-07, QS-13, IF-01, IF-05 | READY |
| **Fase 18** | [`PHASE_18_FILE_PATH_HARDENING_AND_DYNAMIC_SECRETS.md`](phases/PHASE_18_FILE_PATH_HARDENING_AND_DYNAMIC_SECRETS.md) | Hapus hardcoded Infisical credentials & domain privat di `secret_tool.py`, hapus XOR `_decode_key` di skrip OAuth (100% Env-Var First, Vault opsional), sanitasi `session_id` (`{16,64}`) + canonical boundary check di Rust & Python | ADR-004, ADR-005, ADR-017, RISK-16, DEBT-05, QS-14, IF-06 | READY |
| **Fase 19** | [`PHASE_19_WHATSAPP_INGRESS_THROTTLING_AND_QUOTA_DEFENSE.md`](phases/PHASE_19_WHATSAPP_INGRESS_THROTTLING_AND_QUOTA_DEFENSE.md) | Edge pre-queue guard + media cap di `web/`, pure domain `TokenBucket` di `core/domain/rate_limiter.rs` (0 crate baru, patuh ADR-001), orkestrasi di `ProcessIncomingMessageUseCase`, 1x cooldown notice lalu silent auto-drop + tombstone, audit di `whatsapp_action_audits` | ADR-001, ADR-013, ADR-018, RISK-17, DEBT-06, QS-15 | READY |
| **Fase 20** | [`PHASE_20_AMBIENT_CONTEXT_ISOLATION_AND_PROMPT_DEFENSE.md`](phases/PHASE_20_AMBIENT_CONTEXT_ISOLATION_AND_PROMPT_DEFENSE.md) | Per-turn randomized nonce XML + escaping untuk seluruh konteks untrusted (`ambient`, `recall`, `quote`, `sender.name`), *Deterministic Target Binding* pada `TurnCapability` untuk mutasi grup, stop injeksi `WHATSMEOW_API_KEY`/`COMPANION_KEY` pada turn `Guest` | ADR-009, ADR-018, RISK-18, DEBT-08, QS-16 | READY |

---

## 🔒 Invarian Mutlak & Aturan Keamanan (Zero-Bypass Rules)

1. **Invarian Minimal Dependency & Upstream Neutrality**:
   - Dilarang menambah crate eksternal baru di `Cargo.toml` jika dapat diselesaikan bersih dengan `std`/`tokio` yang sudah ada.
   - Dilarang mewajibkan layanan eksternal (Infisical, Vault, Redis, IdP OIDC) atau menanam domain/project ID instansi mana pun di repo upstream (`ADR-004`, `ADR-005`, `ADR-014`).
2. **Invarian Fail-Closed & Constant-Time Verification**:
   - Jika kunci rahasia (`setup_code`, `ADMIN_KEY`, `events_api_key`, atau `AINA_WEBHOOK_SECRET` saat diaktifkan) kosong/spasi, fungsi otorisasi **WAJIB** menolak akses (`return false`).
   - Seluruh perbandingan rahasia/token wajib menggunakan `constant_time_eq` (tanpa operator `==` / `!=`).
3. **Invarian Canonical Path Sandbox**:
   - Seluruh input eksternal pembentuk jalur berkas (`session_id`, `msg.id`) wajib lolos whitelist sintaksis ketat DAN verifikasi canonical containment (`starts_with(&canonical_root)`) sebelum operasi tulis maupun hapus.
4. **Invarian Kemurnian Domain Heksagonal (`ADR-001`)**:
   - Logika domain di `src/core/domain/` (termasuk `gatekeeper.rs` dan `rate_limiter.rs`) wajib tetap murni tanpa `tokio`, tanpa wall-clock internal, dan tanpa I/O. State konkurensi dan proteksi edge diletakkan di lapisan UseCase/Driving Adapter.
5. **Invarian Isolasi Konteks Ambien & Least-Privilege Subprocess**:
   - Seluruh teks riwayat/recall/quote/nama pengirim dienkapsulasi dalam batas XML bernonce acak per tur dengan escaping tag penutup.
   - Mutasi otorisasi grup (`grant-user`/`revoke-user`) diikat deterministik ke target yang eksplisit disebut/di-mention/di-quote pada pesan aktif Admin, dan kredensial gateway WhatsApp dilarang disuntikkan ke subprocess agen pada turn `Authority::Guest`.
