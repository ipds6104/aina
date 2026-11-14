# 🚀 Fase 13: High-Density Concurrency Scaling, Bounded Semaphore, and Token-Aware Account Lease

> **Status**: APPROVED  
> **Komponen Target**: `src/adapters/driven/agy_cli/adapter.rs`, `src/adapters/driven/agy_cli/account_pool.rs`, `src/adapters/driving/web/queue.rs`  
> **Prinsip**: *"Bounded Concurrency Over Process Flooding. 100 Requests in Queue, Max N in Execution."*

---

## 1. Problem Statement & Bottleneck Analysis

Ketika 100 pengguna mengirim pesan di obrolan pribadi (DM) secara simultan:
1. **Ledakan Proses Sistem**: Aina saat ini melakukan `Command::new("agy").output().await` tanpa batas konkurensi. 100 proses biner `agy` serentak membutuhkan $\approx 25\text{ GB RAM}$, memicu CPU 100% dan memanggil Linux OOM Killer.
2. **Badai Rate Limit Upstream (429 Spike)**: 100 request dibagi ke 4 akun Google berarti 25 request paralel per akun, melebihi kuota konkurensi Google Cloud Code dan memicu penguncian *cooldown* massal ke seluruh akun.
3. **Tabrakan File Ruang Kerja**: Sesi-sesi paralel mengeksekusi perintah di direktori kerja yang sama (`workspaces/default`), berisiko menimpa file sementara atau riwayat scratchpad.

---

## 2. Solusi Teknis & Arsitektur

```text
[ 100 Pesan Masuk ]
        │
        ▼
[ FIFO Chat Queues ] (Per chat_jid, debounce active)
        │
        ▼
[ Global Concurrency Governor ] ◄── tokio::sync::Semaphore (Max 6 - 8 in-flight)
   ├── Slot Tersedia? ──► [ Account Lease Selector ] (Pilih akun dengan in-flight < 2)
   │                             │
   │                             ▼
   │                      [ Eksekusi agy ] ──► Workspace Session Sandbox
   │                             │
   │                             ▼
   │                      [ Release Lease & Permit ]
   │
   └── Slot Penuh? ─────► [ Antrean Ramah di WhatsApp ]
                          Kirim notifikasi: "Pesan Mas/Mbak di antrean #3..."
```

### A. Global Concurrency Governor (`tokio::sync::Semaphore`)
Menggantikan pemanggilan `agy` langsung dengan pembatasan izin (*permit*):
- Kapasitas semaphore dinamis: $C = \max(2, N_{\text{accounts}} \times 2)$. Default untuk 4 akun = 8 proses aktif serentak.
- Dapat di-override melalui environment variable: `AINA_MAX_CONCURRENT_AGENTS`.
- Request ke-9 hingga ke-100 secara aman menunggu (*await*) pada antrean Tokio tanpa memicu proses fisik di sistem operasi dan tanpa memakan RAM server.

### B. Account-Aware Token Lease Pattern
Di dalam [src/adapters/driven/agy_cli/account_pool.rs](file:///root/projects/aina/src/adapters/driven/agy_cli/account_pool.rs):
- Setiap akun Google di kolam melacak metrik: `active_in_flight: AtomicUsize`.
- Saat sebuah request membutuhkan akun, pilih akun dengan status `active_in_flight < 2` dan `cooldown_until < now`.
- Ketika request selesai (sukses maupun error), kurangi `active_in_flight` secara atomik (*RAII Guard pattern* via Drop trait).
- Mencegah 1 akun menerima lebih dari 2 request paralel, sehingga **bebas dari error 429 konkurensi upstream**.

### C. Isolasi Sesi Ruang Kerja (*Ephemeral Session Workspace*)
- Setiap pemanggilan agen dengan conversation ID terisolasi diberikan sub-folder konteks sementara:
  `workspaces/default/sessions/<conversation_id>/` atau parameter isolasi agar operasi file dan scratch scripts tidak saling menimpa.

### D. Notifikasi Status Antrean WhatsApp (*Graceful Queue ACK*)
- Jika sebuah pesan harus menunggu di antrean selama lebih dari 8 detik (karena seluruh slot konkurensi sedang sibuk):
  Aina mengirimkan pesan transparan di WhatsApp:
  > *"Pesan diterima! Aina sedang menyelesaikan tugas rekan tim sebelumnya. Pesan Anda berada di antrean #3 dan segera diproses yaa ⏳"*

---

## 3. Rencana Pengujian & Kriteria Selesai (Definition of Done)

1. **Uji Konkurensi Beban (Mock / Stress Test)**:
   - Simulasikan 50–100 request simultan.
   - Pastikan metrik proses fisik `agy` di sistem operasi tidak pernah melebihi angka kapasitas semaphore.
   - Pastikan seluruh 100 request selesai diproses secara bergantian tanpa ada yang hilang (*zero dropped messages*).
2. **Uji Ketahanan Memori**:
   - Penggunaan RAM server tetap stabil di bawah 2.5 GB bahkan saat 100 request membanjiri antrean.
3. **Uji Zero-429 Rate Limit**:
   - Tidak ada akun Google yang terkena *cooldown* 300 detik akibat lonjakan konkurensi serentak.
