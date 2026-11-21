# 🚦 Fase 19: Dual-Layer Hexagonal WhatsApp Ingress Throttling & Pre-Auth Quota Defense

> **Status**: COMPLETED (`bf45a79` — Aligned with ADR-001, ADR-013, ADR-018, RISK-17, DEBT-06, QS-15)  
> **Komponen Target**:  
> - `src/core/domain/rate_limiter.rs` *(BARU — pure domain, nol `tokio`, nol I/O, nol crate baru)*  
> - `src/core/domain/mod.rs`  
> - `src/core/usecases/process_message/mod.rs`  
> - `src/adapters/driving/web/controllers/webhook.rs`  
> - `src/adapters/driving/web/queue.rs` & `src/adapters/driving/web/state.rs`  
> **Prinsip**: *"Pure Hexagonal Domain. Stop Floods Before Media Download & AccountPool Lease. 1x Polite Notice, Then Silent Auto-Drop (Anti-Ban)."*

---

## 1. Problem Statement & Bottleneck Analysis

Audit alur pesan masuk WhatsApp (`webhook.rs` $\to$ `queue.rs` $\to$ `ProcessIncomingMessageUseCase` $\to$ `AntigravityCliAdapter`) menunjukkan 3 celah ketahanan beban:

1. **Unbounded Pre-Queue & Media Download DoS (`webhook.rs` & `queue.rs`)**:
   - Sebelum `Gatekeeper` pernah dijalankan, `webhook_handler` langsung mengunduh & menulis lampiran media ke disk (`webhook.rs:192-239`) lalu mendorong pesan ke `mpsc::unbounded_channel` per `chat_jid`. Spammer DM dengan lampiran dapat memenuhi disk dan RAM sebelum lapisan domain melihat pesan.
2. **Zero Authority-Aware DM Rate Limiting (`gatekeeper.rs` & `process_message/mod.rs`)**:
   - `Gatekeeper::evaluate_primary_bot` merespons semua Direct Message (DM) tanpa pembatasan laju. Catatan arsitektur penting: `Gatekeeper::evaluate` adalah **fungsi domain murni stateless** (`ADR-001`) dan **belum mengetahui `Authority` pengirim** (otoritas baru diketahui setelah `SessionStorePort::get_user_profile` dan `is_admin_identity` dievaluasi di UseCase).
3. **Risiko Cascade Cooldown 429 & WhatsApp Ban**:
   - Tanpa throttling pre-LLM, banjir pesan DM tamu memicu penyewaan `AccountLease` berulang di `AccountPool` hingga seluruh akun Google terkunci cooldown 429.
   - Sebaliknya, membalas **setiap** pesan spam tamu di WhatsApp justru memicu outbound message flood yang berisiko menyebabkan pemblokiran (ban) nomor WhatsApp bot.

---

## 2. Solusi Teknis & Arsitektur (Patuh ADR-001 Hexagonal, Zero New Crates)

```text
[ Incoming WhatsApp Webhook (IF-01) ]
                 │
                 ▼
[ LAPIS 1 — Driving Adapter Edge Guard (src/adapters/driving/web/) ]
    • Pre-media & pre-queue burst guard per sender JID (bounded HashMap, max 10k entries)
    • Bounded per-chat queue capacity (ganti unbounded -> bounded mpsc, misal cap 64)
    • Jika raw flood ekstrem (> 20 msg / 10s dari 1 JID): drop langsung sebelum download media!
                 │
                 ▼
[ ProcessIncomingMessageUseCase::execute ]
    1. Gatekeeper::evaluate(...) ──► Tetap pure stateless (Ignore / RecordOnly / Respond)
    2. Resolve Authority pengirim (is_admin_identity + SessionStorePort::get_user_profile)
    3. [ LAPIS 2 — Pure Domain Rate Limiter (src/core/domain/rate_limiter.rs) ]
       Evaluate(sender_jid, authority, now_epoch):
         ├── Authority::Admin ──────────► Allow (Zero throttling)
         ├── Authority::Atasan / Staff ─► Generous Token Bucket (10 tok, +1/6s)
         └── Authority::Guest ──────────► Strict Token Bucket (3 tok, +1/20s, min gap 5s)
                 ├── Allow ──────────────────────► Lanjut ContextPreparer -> AntigravityCliAdapter
                 ├── NotifyOnceAndDrop ──────────► Kirim 1x balasan cooldown sopan (per window)
                 │                                 + Catat ke `whatsapp_action_audits`
                 └── SilentDrop (Burst / Tombstone)
                                                 ► Auto-drop senyap (0 balasan WA, 0 panggilan agy)
                                                 ► Catat insiden ke `whatsapp_action_audits`
```

### A. Lapis 1 — Edge Burst Guard & Bounded Queue (`src/adapters/driving/web/`)
1. Tambahkan `EdgeIngressGuard` ringan berbasis `tokio::sync::Mutex<HashMap<String, SlidingCounter>>` di `WebhookServerState` (menggunakan `std` + `tokio` bawaan, **tanpa `dashmap`**).
   - Batas keras praproses (berlaku sebelum download media & sebelum enqueue): misal maks 20 event per 10 detik per `sender_jid` (dapat dikonfigurasi via env `AINA_EDGE_BURST_LIMIT`).
   - Eviksi otomatis entri kedaluwarsa (> 5 menit) dan hard cap 10.000 entri agar serangan JID acak tidak menyebabkan OOM.
2. Ubah channel antrean per-chat di `src/adapters/driving/web/queue.rs` dari `mpsc::unbounded_channel` ke `mpsc::channel(64)` (`try_send`) agar antrean memori selalu berbatas (*bounded*).

### B. Lapis 2 — Pure Domain `TokenBucket` & `IngressRateLimiter` (`src/core/domain/rate_limiter.rs`)
Mematuhi **ADR-001** dan `GEMINI.md` Bab 2 (nol `tokio`, nol `std::time::Instant` tersembunyi, 100% deterministik & mudah diuji unit):
- Struktur domain murni yang menerima `now_epoch: i64` sebagai parameter:
  ```rust
  pub enum ThrottleDecision {
      Allow,
      NotifyOnceAndDrop { retry_after_secs: u64 },
      SilentDrop { retry_after_secs: u64, consecutive_hits: u32, tombstoned: bool },
  }
  ```
- **Profil Kuota per Otoritas** (dapat disesuaikan via `RateLimitConfig::from_env()`):
  - `Authority::Admin`: bypass penuh (`Allow`).
  - `Authority::Atasan` / `Authority::Staff`: kapasitas 10 token, isi ulang 1 token per 6 detik.
  - `Authority::Guest` (DM & trigger grup): kapasitas 3 token, isi ulang 1 token per 20 detik, jeda minimum antar pesan 5 detik.
- **Mekanisme Anti-Ban WhatsApp (`NotifyOnceAndDrop` $\to$ `SilentDrop` + Tombstone)**:
  - Saat batas laju pertama kali tercapai dalam satu jendela cooldown, kembalikan `NotifyOnceAndDrop` (menandai `notified_in_window = true`).
  - Seluruh pesan burst berikutnya dari JID yang sama dalam jendela tersebut mengembalikan `SilentDrop` (nol balasan WhatsApp keluar!).
  - Bila pelanggaran berturut-turut mencapai $\ge 5$ kali (`consecutive_hits >= 5`), aktifkan **isolasi sementara (*tombstone*)** selama 15 menit (900 detik) dengan `SilentDrop { tombstoned: true }`.
- **Bounded Memory Sweep**: metode `prune_expired(now_epoch)` otomatis dijalankan periodik saat entri melampaui batas (misal 2.000 JID).

### C. Orkestrasi di `ProcessIncomingMessageUseCase` & Audit SQLite Nyata
1. `ProcessIncomingMessageUseCase` menyimpan `rate_limiter: Arc<tokio::sync::Mutex<IngressRateLimiter>>`.
2. Pada cabang `GatekeeperDecision::Respond`, tepat sebelum memanggil `ContextPreparer` / `AntigravityCliAdapter`:
   - Resolve `authority` menggunakan `is_admin_identity` dan `session_store.get_user_profile(&msg.sender.jid)`.
   - Evaluasi `rate_limiter.check(&msg.sender.jid, authority, now_epoch)`.
   - Jika `NotifyOnceAndDrop { retry_after_secs }`:
     - Kirim **1x** pesan singkat sopan bebas spam/emoji berlebih:
       > *"Pesan diterima. Aina sedang memproses permintaan lain; mohon tunggu sekitar {retry_after_secs} detik sebelum mengirim pesan berikutnya ya."*
     - Catat ke tabel nyata **`whatsapp_action_audits`** via `AuditTracker::record_terminal_event` (`decision: "rate_limited"`, `status: "throttled"`, `usecase: Some("security_rate_limit")`).
     - Langsung `return Ok(())` (tidak memanggil `AntigravityCliAdapter`, tidak menyewa `AccountLease`).
   - Jika `SilentDrop`:
     - **Tidak** mengirim balasan WhatsApp (anti-ban).
     - Catat kejadian (terutama saat transisi ke `tombstoned = true`) ke **`whatsapp_action_audits`** dengan reason `INGRESS_RATE_LIMIT_TOMBSTONED` / `INGRESS_RATE_LIMIT_DROPPED`.
     - Langsung `return Ok(())`.

---

## 3. Rencana Pengujian & Kriteria Selesai (Definition of Done)

1. **Uji Pure Domain Token Bucket (`rate_limiter.rs`)**:
   - 5 pesan Guest dalam 2 detik (memakai `now_epoch` deterministik): pesan 1–3 `Allow`, pesan ke-4 `NotifyOnceAndDrop`, pesan ke-5 `SilentDrop` (1x notice saja).
   - Pelanggaran berulang $\ge 5$ kali memicu `tombstoned = true` selama 900 detik.
   - 20 pesan Admin berturut-turut semuanya `Allow`.
   - Verifikasi `prune_expired` menjaga ukuran map tetap terbatas.
2. **Uji Kemurnian Hexagonal (`ADR-001`)**:
   - Pastikan `src/core/domain/rate_limiter.rs` dan `src/core/domain/gatekeeper.rs` memiliki **0 impor** `tokio`, `axum`, `rusqlite`, atau `reqwest`.
3. **Uji Integrasi Zero Token Pool Bleed & Anti-Ban (`process_message/mod.rs`)**:
   - Simulasikan 50 DM spam dari JID Guest terhadap `ProcessIncomingMessageUseCase`:
     - `MockAgentEngine` dipanggil maksimal 3 kali (sisanya 0 panggilan ke `agy` / `AccountPool`).
     - `TestRecordingWhatsApp` mengirim maksimal **1** pesan cooldown notice (bukan 47 balasan!).
     - Event rate-limit tercatat di `whatsapp_action_audits`.
4. **Uji Edge Pre-Queue Guard (`web/mod.rs`)**:
   - Banjir webhook media ekstrem dari 1 JID dihentikan sebelum menulis media ke disk atau menyebabkan OOM.
5. **Zero New Crates**:
   - `Cargo.toml` tetap tanpa crate baru; `cargo test` lulus penuh.
