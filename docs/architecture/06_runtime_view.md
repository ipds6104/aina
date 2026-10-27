# 6. Alur Waktu Eksekusi (Runtime View)

> [!INFO] **Architecture Provenance & Freshness**
> - **Verified Commit**: `e9b88a2` (branch: `main`)
> - **Verified Timestamp**: `2026-10-26T11:19:12+07:00`
> - **Tracked Paths**: `src/`, `config/`, `skills/`
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
    UC->>DB: Catat pesan ke tabel riwayat (FTS5)
    Note over UC,GW: Tidak ada balasan chat dikirim ke grup (Grup tetap tenang)

    Anggota->>GW: Kirim pertanyaan: "@Aina bagaimana ringkasan rapat tadi?"
    GW->>Web: Webhook HTTP POST (IF-01)
    Web->>UC: Teruskan pesan masuk
    UC->>UC: Evaluasi Gatekeeper: Ada tag @Aina -> Wajib dijawab
    UC->>WA: Kirim status "Sedang mengetik..." (IF-02)
    UC->>DB: Ambil 15 pesan terakhir grup (Rolling Context) (IF-04)
    DB-->>UC: Riwayat obrolan rapat
    UC->>AGY: Jalankan agen dengan konteks obrolan (IF-03)
    AGY-->>UC: Hasil ringkasan rapat
    UC->>WA: Kirim balasan teks rapi ke grup WhatsApp (IF-02)
    UC->>DB: Simpan sesi dan jawaban Aina (IF-04)
```

---

## 6.2 Skenario 2: Rotasi Akun Multi-Account & Penanganan Otomatis Limit Kuota (429)

Skenario ini menunjukkan keandalan Aina saat menghadapi batas pemakaian harian pada akun AI:

```mermaid
sequenceDiagram
    autonumber
    participant UC as ProcessIncomingMessageUseCase
    participant Adapter as AntigravityCliAdapter
    participant Pool as Account Pool (Round-Robin)
    participant CLI as Binary Antigravity (agy)

    UC->>Adapter: Minta eksekusi pesan
    Adapter->>Pool: Ambil akun aktif giliran saat ini (Akun A)
    Pool-->>Adapter: Token Akun A
    Adapter->>CLI: Eksekusi dengan Akun A (IF-03)
    CLI-->>Adapter: Error: HTTP 429 Quota Exhausted / Rate Limit!
    
    Note over Adapter,Pool: Deteksi cerdas: Akun A kehabisan kuota
    Adapter->>Pool: Tandai Akun A istirahat, pilih Akun B (Auto-Failover)
    Pool-->>Adapter: Token Akun B
    Adapter->>CLI: Ulangi eksekusi dengan Akun B (IF-03)
    CLI-->>Adapter: Sukses! Jawaban selesai diproses
    Adapter-->>UC: Kembalikan jawaban ke pengguna (Nol kegagalan ke chat)
```

---

## 6.3 Skenario 3: Eksekusi Keahlian Khusus & Pengambilan Kunci Rahasia JIT

Skenario ini menggambarkan bagaimana alat kerja di sandbox meminta kunci rahasia secara aman tanpa ada kunci tersimpan statis:

```mermaid
sequenceDiagram
    autonumber
    participant AGY as Binary Antigravity (agy)
    participant Skill as Custom Skill Script (data/custom-skills/)
    participant Broker as SecretBroker (secret_tool)
    participant Vault as Brankas Rahasia (Infisical / Vault)
    participant Cloud as Layanan Luar (Google Drive / VLM)

    AGY->>Skill: Jalankan perintah (misal: ambil file Google Drive)
    Skill->>Broker: Minta kredensial JIT: `secret_tool run -- ...` (IF-06)
    Broker->>Vault: Minta token akses sesi terenkripsi
    Vault-->>Broker: Kredensial sementara
    Broker->>Cloud: Eksekusi aksi ke API luar dengan token sementara (IF-07)
    Cloud-->>Broker: Data hasil unduhan
    Broker-->>Skill: Kembalikan hasil olahan data
    Note over Broker,Skill: Kredensial langsung dibersihkan dari memori
    Skill-->>AGY: Sajikan kesimpulan data
```

---

## 6.4 Skenario 4: Pembatasan Giliran Sesi (*Turn Cap*) & Perangkuman Cerdas

Skenario ini menunjukkan bagaimana Aina menjaga efisiensi token dan mencegah agen menjadi lemot pada obrolan berhari-hari:

```mermaid
sequenceDiagram
    autonumber
    participant UC as ProcessIncomingMessageUseCase
    participant Lifecycle as SessionLifecycle
    participant DB as SqliteSessionStore
    participant AGY as AntigravityCliAdapter

    UC->>DB: Periksa jumlah giliran sesi obrolan saat ini
    DB-->>UC: Giliran ke-30 (Batas Turn Cap tercapai!)
    UC->>Lifecycle: Pemicu pergantian sesi baru
    Lifecycle->>AGY: Minta rangkum inti kesepakatan obrolan lama (IF-03)
    AGY-->>Lifecycle: Rangkuman padat (Lean Resumed Prompt)
    Lifecycle->>DB: Arsipkan sesi lama, buat ID sesi baru dengan rangkuman awal (IF-04)
    UC->>AGY: Lanjutkan giliran pengguna pada sesi baru yang bersih
    AGY-->>UC: Jawaban cepat tanpa beban memori masa lalu yang membengkak
```
