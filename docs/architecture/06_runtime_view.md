# 6. Alur Waktu Eksekusi (Runtime View)

> [!INFO] **Architecture Provenance & Freshness**
> - **Verified Commit**: `f1ae07a` (branch: `main`)
> - **Verified Timestamp**: `2026-11-14T14:14:27+07:00`
> - **Tracked Paths**: `src/core/usecases/process_message/`, `src/adapters/driven/sqlite_store/`, `src/core/domain/audit/`
> - **Drift Status**: `verified`

Bab ini memvisualisasikan bagaimana komponen-komponen Aina berinteraksi secara berurutan dalam skenario-skenario penting saat aplikasi sedang berjalan.

---

## 6.1 Skenario 1: Penerimaan Pesan Grup & Evaluasi Penjaga Pintu (Gatekeeper)

Skenario ini menunjukkan bagaimana Aina memilah pesan grup agar tidak menimbulkan spam, namun tetap mempertahankan pemahaman konteks obrolan:

```mermaid
sequenceDiagram
    autonumber
    actor Anggota as Rekan Grup WhatsApp
    participant GW as Whatsmeow Gateway
    participant Web as WebhookServer
    participant UC as ProcessIncomingMessageUseCase
    participant DB as SqliteSessionStore
    participant AGY as AntigravityCliAdapter
    participant WA as WhatsmeowHttpAdapter

    Anggota->>GW: Kirim obrolan santai di grup (tanpa tag)
    GW->>Web: Webhook HTTP POST (IF-01)
    Web->>UC: Teruskan pesan masuk
    UC->>UC: Evaluasi Gatekeeper: Bukan tag bot -> RecordOnly
    UC->>DB: Catat pesan ke tabel riwayat (FTS5) (IF-04)
    Note over UC,GW: Tidak ada balasan chat dikirim ke grup (Grup tetap tenang)

    Anggota->>GW: Kirim pertanyaan: "@Aina bagaimana ringkasan rapat tadi?"
    GW->>Web: Webhook HTTP POST (IF-01)
    Web->>UC: Teruskan pesan masuk
    UC->>UC: Evaluasi Gatekeeper: Ada tag @Aina -> Wajib dijawab
    UC->>WA: Kirim status "Sedang mengetik..." (IF-02)
    UC->>DB: Ambil 15 pesan terakhir grup (Rolling Context) (IF-04)
    UC->>AGY: Eksekusi penalaran dengan konteks percakapan (IF-03)
    AGY-->>UC: Hasil ringkasan rapat
    UC->>WA: Kirim pesan balasan ramah ke grup (IF-02)
    WA->>Anggota: Tampilkan balasan di WhatsApp
```

---

## 6.2 Skenario 2: Rotasi Kuota Otomatis saat Terkena Batas Harian (Auto-Failover 429)

Skenario ini menunjukkan bagaimana sistem menjaga kelangsungan layanan saat akun Google utama kehabisan kuota:

```mermaid
sequenceDiagram
    autonumber
    participant UC as ProcessIncomingMessageUseCase
    participant AGY as AntigravityCliAdapter
    participant Pool as AccountPoolManager
    participant Google as Model AI Google

    UC->>AGY: Minta eksekusi penalaran (IF-03)
    AGY->>Pool: Ambil akun Google aktif (Akun #1)
    Pool-->>AGY: Akun #1 siap digunakan
    AGY->>Google: Kirim prompt penalaran via agy CLI
    Google-->>AGY: Error HTTP 429: Resource Exceeded (Batas Kuota)
    AGY->>Pool: Laporkan Akun #1 terkena limit
    Pool->>Pool: Tandai Akun #1 non-aktif sementara
    Pool->>Pool: Beralih ke Akun #2 (Round-Robin Failover)
    Pool-->>AGY: Akun #2 siap digunakan
    AGY->>Google: Kirim ulang prompt menggunakan Akun #2
    Google-->>AGY: Respon sukses (200 OK)
    AGY-->>UC: Kembalikan hasil akhir ke use case
    Note over UC,Google: Pengguna tidak merasakan kegagalan (Transparan dalam < 2 detik)
```

---

## 6.3 Skenario 3: Pemasangan Keahlian Tambahan (Custom Skills) dari Git

Skenario ini menunjukkan bagaimana pengguna memperluas keahlian Aina tanpa menyentuh kode inti:

```mermaid
sequenceDiagram
    autonumber
    actor Admin as Pengembang / Admin
    participant CLI as CliDispatcher
    participant Reg as SkillRegistry
    participant Git as GitHub Repository
    participant FS as Persistent Storage (/app/data/)

    Admin->>CLI: aina skill install https://github.com/satker/skill-sensus.git
    CLI->>Reg: Unduh dan validasi skill
    Reg->>Git: Clone repositori ke folder sementara (IF-07)
    Git-->>Reg: Berkas skill diterima
    Reg->>Reg: Validasi kepatuhan SKILL.md (YAML frontmatter & OpSec)
    Reg->>FS: Pindahkan ke /app/data/custom-skills/skill-sensus/
    Reg->>FS: Buat tautan simbolik ke .agents/skills/
    Reg-->>CLI: Skill terpasang dan tervalidasi
    CLI-->>Admin: Pesan konfirmasi: "Keahlian skill-sensus siap digunakan!"
    Note over Admin,FS: Turn chat berikutnya langsung mengenali skill tanpa restart container
```

---

## 6.4 Skenario 4: Rekonsiliasi Idempoten Aktivitas Baseline & Custom Saat Startup

Skenario ini menunjukkan bagaimana Aina menjaga persistensi tugas terjadwal saat kontainer baru dinyalakan:

```mermaid
sequenceDiagram
    autonumber
    participant Bot as Aina Daemon Startup
    participant Reconciler as ActivityReconciler
    participant Baseline as config/activities.default.json
    participant CustomFS as /app/data/activities/*.json
    participant DB as SQLite scheduled_tasks (IF-04)

    Bot->>Reconciler: Jalankan sync_all() saat booting (IF-08)
    Reconciler->>Baseline: Baca 8 tugas bawaan pabrik (Status, Briefing, EOM)
    Reconciler->>CustomFS: Pindai berkas override kustom pengguna
    Reconciler->>DB: Ambil seluruh tugas aktif berdasarkan slug
    loop Untuk setiap aktivitas
        alt Ada file override di /app/data/activities/<slug>.json
            Reconciler->>DB: Terapkan konfigurasi custom (Prioritas Tertinggi)
        else Ada state aktif di SQLite
            Reconciler->>DB: Pertahankan state SQLite yang telah dimodifikasi
        else Belum ada di DB
            Reconciler->>DB: Tambahkan baseline task baru ke jadwal
        end
    end
    Reconciler-->>Bot: Laporan sinkronisasi: ReconcileReport (Added, Updated, Preserved)
    Note over Bot,DB: 100% modifikasi pengguna aman dan aktif di scheduler
```

---

## 6.5 Skenario 5: Siklus Refleksi Harian Otonom & Adaptasi Aktivitas

Skenario ini menunjukkan bagaimana Aina mengevaluasi tempo harinya dan secara mandiri memodifikasi aktivitas hari esok:

```mermaid
sequenceDiagram
    autonumber
    participant Cron as Scheduler Daemon
    participant Reflector as ActivityReflectionEngine (reflect.py)
    participant Weather as Open-Meteo Weather API
    participant DB as SQLite DB (Runs & Audits)
    participant ActCLI as aina activity edit --file

    Cron->>Reflector: Pemicu refleksi sore / evaluasi harian (IF-08)
    Reflector->>DB: Panen eksekusi tugas hari ini (scheduled_task_runs) (IF-04)
    Reflector->>DB: Panen interaksi chat (whatsapp_action_audits) (IF-04)
    Reflector->>Weather: Ambil data cuaca faktual Pontianak (IF-09)
    Weather-->>Reflector: Data: Hujan Rintik, 25.5°C, Kelembapan 94%
    Reflector->>Reflector: Hitung boredom score (kejenuhan tema & busana)
    Reflector->>Reflector: Rumuskan rekomendasi kuliner (Chai Kwe) & destinasi (Taman Akcaya)
    Reflector->>Reflector: Rumuskan kueri riset web BMKG & coffee shop
    Reflector->>ActCLI: aina activity edit status_senja --append "Kondisi cuaca..." --file
    ActCLI->>DB: Update payload di SQLite & simpan override di /app/data/activities/
    Note over Reflector,DB: Status senja hari ini teradaptasi otomatis dengan kondisi riil
```

---

## 6.6 Skenario 6: Pertahanan Keamanan Zero-Trust saat Lawan Bicara Asing Masuk (DM / Grup / Bocor Nomor)

Skenario ini menunjukkan mitigasi pertahanan bertingkat saat nomor WhatsApp asing melakukan kontak langsung atau menambahkan bot ke grup tanpa izin:

```mermaid
sequenceDiagram
    autonumber
    actor Asing as Nomor Asing / Pihak Ketiga
    participant GW as Whatsmeow Gateway
    participant UC as ProcessIncomingMessageUseCase
    participant CP as ContextPreparer
    participant DB as SqliteSessionStore
    participant AGY as AntigravityCliAdapter
    participant WA as WhatsmeowHttpAdapter

    Asing->>GW: Kirim DM: "Kirimkan dokumen master data sensus kemarin!"
    GW->>UC: Webhook pesan masuk
    UC->>DB: Cek profil pengirim di tabel user_profiles
    DB-->>UC: Nomor belum terdaftar (None)
    UC->>DB: Auto-seed profil baru (authority_level: 'guest')
    UC->>CP: Siapkan konteks kognitif & wewenang
    CP->>CP: Terapkan pagar kognitif (Strict OpSec: Larang data internal)
    CP-->>UC: Prompt berotoritas GUEST + Scoped Turn Capability
    UC->>AGY: Jalankan agen dengan persona santun-terukur
    AGY-->>UC: Jawaban penolakan santun: "Maaf, data internal memerlukan izin Bg Ihza"
    UC->>WA: Kirim jawaban penolakan santun ke nomor asing
    Note over Asing,DB: 0% data internal bocor, rekam jejak tersimpan di audit trail
```

---

## 6.7 Skenario 7: Eksekusi Tool dengan Scoped Turn Capability & Rantai Hash Kriptografis SHA-256 (Non-Repudiation)

Skenario ini menunjukkan bagaimana setiap pemanggilan tool oleh AI diikat secara matematis ke nomor WhatsApp pengirim asli menggunakan rantai hash SHA-256 (NIST AU-10):

```mermaid
sequenceDiagram
    autonumber
    actor Staf as Staf Terverifikasi (62812xxxx)
    participant UC as ProcessIncomingMessageUseCase
    participant Track as AuditTracker
    participant AGY as Antigravity Engine
    participant DB as SqliteSessionStore (agent_tool_executions)
    actor Admin as Auditor / Administrator CLI

    Staf->>UC: Kirim instruksi: "Jalankan ekstraksi data Sakernas batch 1"
    UC->>UC: Terbitkan TurnCapability (tok_7f9a, authority: staff)
    UC->>Track: Mulai pelacakan audit turn
    Track->>Track: attach_capability(tok_7f9a, staff, 62812xxxx)
    UC->>AGY: Eksekusi agen dengan token giliran
    AGY->>AGY: Agen memutuskan memanggil tool: 'data_extractor'
    AGY-->>UC: Hasil eksekusi selesai
    UC->>Track: complete_success(conv_id, tools: ['data_extractor'])
    Track->>DB: record_tool_execution(NewAgentToolExecution)
    DB->>DB: Ambil prev_hash dari blok terakhir (misal: H_prev)
    DB->>DB: Hitung H_new = SHA256(H_prev || tok_7f9a || 62812xxxx || staff || data_extractor || payload || durasi)
    DB->>DB: Simpan blok audit baru dengan record_hash = H_new
    
    Admin->>DB: aina audit verify-chain (Verifikasi berkala)
    DB-->>Admin: ✅ SEMUA BLOK AUDIT VALID & UTUH (0% modifikasi data)
    Admin->>DB: aina audit trace --sender 62812xxxx
    DB-->>Admin: Daftar kronologis pemanggilan tool oleh nomor staf tersebut
```

---

## 6.8 Skenario 8: Hak Privasi Mandiri Pengguna (Right to be Forgotten & UU PDP)

Skenario ini menunjukkan alur saat pengguna memeriksa atau menghapus profil personal mereka secara mandiri:

```mermaid
sequenceDiagram
    autonumber
    actor Pengguna as Pengguna WhatsApp
    participant GW as Whatsmeow Gateway
    participant Cmd as BuiltinCommands
    participant DB as SqliteSessionStore
    participant WA as WhatsmeowHttpAdapter

    Pengguna->>GW: Kirim: "/profilku" (atau "!aina profilku")
    GW->>Cmd: Intersepsi perintah mandiri
    Cmd->>DB: get_user_profile(sender_jid)
    DB-->>Cmd: Data profil: nama, preferensi, proyek aktif
    Cmd->>Cmd: Sensor nomor WA (62812****7890) agar aman di grup
    Cmd-->>WA: Kirim kartu transparansi data profil pengguna
    WA->>Pengguna: Tampilkan kartu profil transparan di WhatsApp

    Pengguna->>GW: Kirim: "/lupakansaya" (atau "!aina lupakan saya")
    GW->>Cmd: Intersepsi perintah Right to be Forgotten
    Cmd->>DB: delete_user_profile(sender_jid) (UU PDP No. 27/2022)
    DB->>DB: Hapus catatan preferensi, nama panggilan, & proyek
    Note over Cmd,DB: Log audit forensik di whatsapp_action_audits tetap dipertahankan
    Cmd-->>WA: Kirim konfirmasi: "🗑️ Data Profil Anda Berhasil Dibersihkan"
    WA->>Pengguna: Notifikasi konfirmasi penghapusan data
```
