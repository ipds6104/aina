# 🛡️ Master Roadmap: High-Density Concurrency Scaling & Zero Trust Identity Anti-Impersonation

> **Status**: APPROVED & CANONICAL  
> **Target Release**: Aina v0.3.0  
> **Prinsip Utama**: *"Never Trust, Always Verify. Bounded Concurrency Over Process Flooding."*  
> **Pilar Arsitektur**: Zero Trust Architecture (NIST SP 800-207), LLM Token-Lease Queuing, Identity Lifecycle (UNCONFIRMED $\rightarrow$ TENTATIVE $\rightarrow$ VERIFIED).

---

## Executive Summary & Latar Belakang Masalah

Sebagai *self-hosted Agentic AI Co-Worker* yang terhubung ke WhatsApp dan digerakkan oleh Google Antigravity CLI (`agy`), Aina menghadapi dua tantangan arsitektural krusial saat memasuki lingkungan produksi nyata dengan puluhan hingga ratusan pengguna aktif:

1. **Tantangan Konkurensi Skala Tinggi (3 Pengguna vs 100 Pengguna)**:
   - *Kondisi 3 Pengguna*: Berjalan mulus karena setiap sesi memicu proses `agy` terpisah dan kolam akun (`account_pool`) memutar 4 akun Google secara seimbang.
   - *Kondisi 100 Pengguna*: Memicu pemanggilan 100 proses biner `agy` secara paralel ($100 \times 250\text{ MB} \approx 25\text{ GB RAM}$), menyebabkan *CPU thrashing*, pemutusan kontainer oleh Linux OOM Killer, dan lonjakan error *429 Resource Exhausted* ke Google Cloud Code yang melumpuhkan seluruh akun (*cascade cooldown lockout*).
2. **Tantangan Keamanan Identitas (Impersonation & Social Engineering)**:
   - *Autonomous Profiler* saat ini mengekstrak nama dan peran secara otomatis dari percakapan santai (contoh: *"Aku Hendra IPDS"* atau *"Saya Kepala Kantor"*).
   - *Celah Keamanan*: Pihak asing dapat mengaku-aku (*name claim spoofing*) sebagai Admin ("Ihza Karunia") atau mengklaim jabatan strategis ("Kepala BPS"), memicu eskalasi `role_hierarchy = atasan` yang memanipulasi persona Aina menjadi takzim dan membuka celah pembocoran data sensitif (*pretexting attack*).

Roadmap ini menetapkan solusi komprehensif, deterministik, dan berkinerja tinggi melalui 4 fase berurutan.

---

## 🏛️ Arsitektur 4 Pilar Solusi

```text
┌────────────────────────────────────────────────────────────────────────────────────────┐
│                        AINA HIGH-CONCURRENCY & ZERO TRUST GATEWAY                      │
├───────────────────────────────────────────┬────────────────────────────────────────────┤
│   PILAR 1: BOUNDED CONCURRENCY & LEASE    │    PILAR 2: ZERO TRUST IDENTITY LIFECYCLE  │
│   • Global Tokio Semaphore (Bounded Slots)│    • 3-Tier State: UNCONFIRMED/TENTATIVE/  │
│   • Account-Aware Token Lease Pattern     │      VERIFIED                                  │
│   • WhatsApp Graceful Queue ACK (Anti-Drop│    • Protected Names & Anti-Collision      │
│   • Ephemeral Workspace Isolation         │    • OOB Companion Approval Push           │
├───────────────────────────────────────────┼────────────────────────────────────────────┤
│   PILAR 3: CLI GOVERNANCE & TELEMETRY     │    PILAR 4: BUILT-IN SKILLS & RUNBOOKS     │
│   • aina user verify / unconfirmed / rejec│    • SOP Identitas & Verifikasi Lawan Bicar│
│   • aina status --concurrency / --queue   │    • Prosedur Eskalasi & Otorisasi Manual  │
│   • TurnCapability Policy Verification    │    • Audit Rantai Hash Anti-Sangkalan      │
└───────────────────────────────────────────┴────────────────────────────────────────────┘
```

---

## 🗺️ Roadmap Fase Implementasi

| Fase | Nama Dokumen Spesifikasi | Fokus Utama & Deliverables | Status |
| :---: | :--- | :--- | :---: |
| **Fase 13** | [`PHASE_13_BOUNDED_CONCURRENCY_AND_ACCOUNT_LEASE.md`](phases/PHASE_13_BOUNDED_CONCURRENCY_AND_ACCOUNT_LEASE.md) | Bounded Semaphore global, Account-Aware Lease Pattern, Ephemeral Session Workspace, WhatsApp Queue ACK | READY |
| **Fase 14** | [`PHASE_14_ZERO_TRUST_IDENTITY_AND_ANTI_IMPERSONATION.md`](phases/PHASE_14_ZERO_TRUST_IDENTITY_AND_ANTI_IMPERSONATION.md) | 3-Tier Identity State, Pemisahan `claimed_*` vs `verified_*`, Protected Name Collisions, OOB Approval Push | READY |
| **Fase 15** | [`PHASE_15_IDENTITY_AND_CONCURRENCY_CLI_GOVERNANCE.md`](phases/PHASE_15_IDENTITY_AND_CONCURRENCY_CLI_GOVERNANCE.md) | Native CLI `aina user verify/unconfirmed/reject`, Telemetri Konkurensi `aina status --concurrency` | READY |
| **Fase 16** | [`PHASE_16_BUILTIN_SKILLS_AND_OPERATING_RUNBOOKS.md`](phases/PHASE_16_BUILTIN_SKILLS_AND_OPERATING_RUNBOOKS.md) | Built-in Skill Runbook, Template Konfirmasi Chat Companion, Integrasi Audit Trail SHA-256 | READY |

---

## 🔒 Invarian Mutlak & Aturan Keamanan (Zero-Bypass Rules)

1. **Invarian Konkurensi (No Unbounded Forking)**:
   - Dilarang memicu proses `agy` secara bebas tanpa melalui `tokio::sync::Semaphore`.
   - Kapasitas konkurensi dihitung deterministik: $C = \min(N_{\text{accounts}} \times 2, \text{MAX\_SYSTEM\_SLOTS})$.
2. **Invarian Identitas (Self-Declaration is NEVER Identity)**:
   - Klaim nama atau peran di bubble chat WhatsApp berstatus **`UNCONFIRMED`**.
   - Hak istimewa peran (`staff`, `atasan`, `admin`) **TIDAK PERNAH** diberikan hanya karena seseorang mengaku di chat.
3. **Invarian Perlindungan Nama (Protected Name Rule)**:
   - Nomor WhatsApp yang belum terverifikasi dilarang keras mengaitkan diri dengan nama Admin atau staf yang telah `VERIFIED`.
   - Pelanggaran memicu penolakan seketika dan pencatatan audit *security incident*.
4. **Invarian Persistensi Bersih (Schema Evolution with Backward Compatibility)**:
   - Migrasi SQLite dilakukan non-destruktif (`ALTER TABLE user_profiles ADD COLUMN ...`). Data yang sudah ada secara aman dimigrasikan ke status `VERIFIED` untuk admin dan `UNCONFIRMED` untuk tamu.
