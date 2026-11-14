# 4. Strategi Solusi (Solution Strategy)

> [!INFO] **Architecture Provenance & Freshness**
> - **Verified Commit**: `f1ae07a` (branch: `main`)
> - **Verified Timestamp**: `2026-11-14T14:14:27+07:00`
> - **Tracked Paths**: `src/core/domain/capability.rs`, `src/core/domain/audit/`, `src/core/usecases/process_message/`
> - **Drift Status**: `verified`

Bab ini menjelaskan strategi rancangan fundamental yang dipilih untuk menyelesaikan masalah-masalah utama dalam Aina. Setiap strategi dipilih dengan pertimbangan praktis agar sistem tangguh, aman, dan mudah dimengerti.

---

## 4.1 Pemisahan Arsitektur Bersih (Hexagonal / Ports & Adapters)

Aina memisahkan kode program menjadi dua bagian besar: **Logika Inti (*Core*)** dan **Colokan Penghubung Luar (*Adapters*)**.

```mermaid
flowchart TD
    subgraph DrivingAdapters ["Colokan Pemicu (Driving Adapters)"]
        WebHook["Webhook Masuk WhatsApp"]
        WebUI["Tampilan Web Setup / Simulator"]
        CliCmd["Perintah Terminal (aina CLI)"]
    end

    subgraph CoreDomain ["Kotak Logika Murni (Core Application)"]
        Gatekeeper["Penjaga Pintu (Evaluasi Pesan)"]
        ContextPrep["Peracik Konteks (Rolling Context)"]
        Persona["Mesin Karakter & Tata Krama"]
        Lifecycle["Pengatur Batas Sesi (Turn Cap)"]
        ActivityDomain["Entitas & Rekonsiliasi Aktivitas"]
    end

    subgraph DrivenAdapters ["Colokan Pelaksana (Driven Adapters)"]
        AgyRunner["Eksekutor Antigravity CLI"]
        SqliteStore["Database SQLite (Penyimpan Memori)"]
        WaSender["Pengirim Pesan Whatsmeow"]
        SecretBrokerTool["Kurir Rahasia (Secret Broker)"]
        EnvAPI["Grounding Cuaca & Web Search"]
    end

    DrivingAdapters -->|"Memicu alur"| CoreDomain
    CoreDomain -->|"Memanggil kontrak interface (Ports)"| DrivenAdapters
```

### Mengapa pola ini dipilih?
1. **Bebas Ketergantungan Luar**: Logika bisnis (misalnya aturan kapan harus membalas, bagaimana merangkum obrolan, dan batasan sesi) murni ditulis tanpa mencampur aduk kode jaringan HTTP atau SQL.
2. **Mudah Diuji**: Kita bisa menguji apakah aturan penjaga gerbang bekerja dengan benar tanpa perlu menyalakan WhatsApp sungguhan.
3. **Mudah Diganti**: Jika suatu saat gerbang WhatsApp diganti dengan penyedia lain, kita hanya perlu mengubah colokan pengirim (*adapter*) tanpa menyentuh otak utama Aina.

---

## 4.2 Strategi Jalur Ganda WhatsApp (Dual-WhatsApp Pipeline)

Untuk mengatasi hambatan birokrasi di mana bot baru dilarang masuk grup kerja, Aina mengadopsi strategi **Jalur Ganda**:

1. **Jalur 1: Nomor Bot Resmi (`PrimaryBot`)**:
   - Diberi nomor khusus.
   - Digunakan untuk melayani pesan pribadi (*DM*) dari siapa saja dan grup-grup yang memang secara resmi mengundang bot.
2. **Jalur 2: Sensor Bayangan Akun Pribadi (`UserCompanion`)**:
   - Terhubung menggunakan nomor WhatsApp pribadi pemilik bot.
   - **Prinsip Nol Kebocoran Privasi**: Obrolan pribadi orang lain dengan pemilik bot diabaikan 100% dan tidak pernah dibaca oleh AI.
   - **Perekam Pasif di Grup**: Aina menyimak percakapan grup kantor tempat pemilik bergabung, mencatat topik penting ke database lokal.
   - **Pemanggilan Khusus**: Jika pemilik mengetik `!aina` di grup, atau mengirim pesan ke nomor sendiri (*Message Yourself*), Aina segera merespon sebagai asisten pribadi.

---

## 4.3 Strategi Kolam Akun & Rotasi Kuota Otomatis

Untuk mencegah AI berhenti bekerja saat kuota harian habis (*rate limit 429*):
- Pengguna dapat menghubungkan banyak akun Google (misalnya 5 hingga 10 akun) ke dalam sistem.
- Aina menjalankan sistem **Giliran Berputar (*Round-Robin*)**: setiap sesi percakapan baru dilayani oleh akun bergantian.
- Jika sebuah akun mendadak terkena batas kuota di tengah proses, sistem otomatis mendeteksi error tersebut dan segera mengalihkan tugas ke akun berikutnya (*Auto-Failover*) tanpa membuat balasan pesan gagal.

---

## 4.4 Strategi Penjaga Pintu & Konteks Obrolan Berjalan

Agar Aina tidak membuat kebisingan di grup kantor namun tetap cerdas saat diajak bicara:
1. **Penyaringan Pesan Masuk (*Gatekeeper*)**:
   - Jika pesan masuk dari obrolan pribadi: Selalu diproses dan dijawab.
   - Jika pesan masuk dari grup: Diperiksa apakah menyebut `@Aina`, mengutip pesan Aina, atau diawali tanda seru `!aina`. Jika tidak, statusnya adalah `RecordOnly` (hanya dicatat diam-diam di database lokal).
2. **Konteks Berjalan (*Rolling Context*)**:
   - Saat ada orang yang memanggil `@Aina` di grup, sistem otomatis menarik 10–20 pesan terakhir sebelum pemanggilan tersebut.
   - Konteks obrolan disisipkan ke dalam instruksi agen sehingga Aina langsung memahami duduk persoalan tanpa meminta orang tersebut mengulang ceritanya.

---

## 4.5 Strategi Batasan Sesi & Pemulihan Mandiri (Turn Cap & Self-Healing)

Percakapan yang terlalu panjang berisiko membuat model AI boros token, lambat, atau kehilangan fokus.
- **Batas Giliran Sesi (*Deterministic Session Turn Cap*)**: Setiap sesi obrolan dibatasi hingga jumlah giliran tertentu. Jika batas tercapai, sistem secara otomatis merangkum inti obrolan lama menjadi ringkasan padat (*lean resumed prompt*) dan membuka sesi baru di balik layar.
- **Pemulihan Aliran Putus (*Self-Healing Stream Fallback*)**: Jika aliran jaringan antara server dan model AI terputus tiba-tiba, sistem menangkap sisa teks yang berhasil keluar dan meminta model melanjutkan bagian yang terpotong secara otomatis.

---

## 4.6 Strategi Pemisahan Keahlian & Kurir Kunci Rahasia (Custom Skills & Secret Broker)

1. **Inti Program yang Netral**: Repositori utama bebas dari kode spesifik kantor. Siapa pun dapat menggunakan Aina untuk keperluan apa pun.
2. **Keahlian Khusus Modular (`data/custom-skills`)**: Fitur khusus kantor (seperti kueri data internal, cek disposisi surat, atau ekstraksi dokumen dinas) ditempatkan di folder terpisah. Aina menyediakan perintah `aina skill install <repo-git>` untuk memasang keahlian tambahan secara instan.
3. **Penyuntikan Rahasia Tepat Waktu (*Just-in-Time Secret Broker*)**: Kunci API atau password tidak disimpan di kode atau lingkungan sistem permanen. Saat skrip membutuhkan kunci, skrip meminta izin ke Secret Broker. Setelah eksekusi selesai, kredensial segera dibersihkan dari memori.

---

## 4.7 Strategi Rekonsiliasi Aktivitas Hibrida (Declarative Baseline & Persistent Custom Activities)

Untuk memecahkan masalah hilangnya jadwal rutin saat kontainer Docker/Coolify di-redeploy:
- **Baseline Activities**: Didefinisikan secara deklaratif di `config/activities.default.json` (termasuk briefing pagi, status WhatsApp harian, pengingat akhir bulan).
- **Custom Activities & Modifikasi**: Disimpan di volume persisten `/app/data/activities/<slug>.json` dan basis data SQLite.
- **Mesin Rekonsiliator (*ActivityReconciler*)**: Dijalankan otomatis saat bot booting (`ActivityReconciler::sync_all`). Aturan resolusi prioritas:
  $$\text{Custom File Override} > \text{SQLite Active State} > \text{Declarative Baseline}$$
- **CLI Terpadu**: Pengguna dapat memodifikasi aktivitas kapan saja via terminal (`aina activity edit <slug> --append "..." --file`).

---

## 4.8 Strategi Penemuan Keahlian Bertingkat (Three-Tier Progressive Disclosure)

Mengadopsi standar terbuka internasional **`agentskills.io`**:
1. **Tier 1 (Discovery)**: Saat inisialisasi, hanya metadata nama dan deskripsi YAML frontmatter yang dimuat (~40 token per skill).
2. **Tier 2 (Activation)**: Ketika instruksi pengguna cocok dengan deskripsi skill, agen secara mandiri membaca isi lengkap berkas `SKILL.md`.
3. **Tier 3 (Execution)**: Skrip eksekutabel di folder `scripts/` dijalankan di subproses bash dengan output terstruktur (`--json`), mencegah kode mentah mencemari jendela konteks LLM.
4. **Hot Container Auto-Discovery**: Symlink otomatis dari `/app/data/custom-skills` ke `.agents/skills/` membuat skill baru langsung terdeteksi pada turn berikutnya tanpa perlu me-restart kontainer.

---

## 4.9 Strategi Refleksi Pengalaman & Grounding Lingkungan (Experiential Reflection)

Terinspirasi dari *Gibbs' Reflective Cycle* dan arsitektur refleksi *Stanford Generative Agents*:
1. **Pemanenan Bukti**: Mengaudit riwayat tugas terjadwal (`aina activity runs`), jurnal visual (`data/status_journal.jsonl`), dan aksi chat.
2. **Deteksi Kejenuhan (*Boredom Engine*)**: Menghitung frekuensi pengulangan tema dan pakaian; memicu variasi jika skor kebosanan $\ge 0.5$.
3. **Grounding Fisik Khatulistiwa**: Mengambil data cuaca real-time Pontianak (suhu, hujan, kelembapan) via Open-Meteo API tanpa menebak.
4. **Riset Web Terarah (*Gated Novelty Web Search*)**: Memformulasikan 1–2 kueri pencarian tajam untuk memperkaya latar sensorik kuliner dan destinasi lokal.
5. **Adaptasi Otonom**: Menyuntikkan hasil refleksi ke aktivitas terjadwal via CLI (`aina activity edit <slug> --append "..." --file`).

---

## 4.10 Strategi Pertahanan Berlapis & Audit Anti-Sangkalan (Zero Implicit Trust & Non-Repudiation)

Untuk menjamin keamanan kelas enterprise di lingkungan instansi pemerintah (BPS):
1. **Model Otoritas 4-Tingkat Terisolasi Konfigurasi**:
   - Status `ADMIN` terikat pada konfigurasi nomor WhatsApp Companion, bukan pada basis data atau teks pesan masuk.
   - Hak istimewa pimpinan satker (`ATASAN`) dan rekan kerja (`STAFF`) dipisahkan secara tegas dari tamu luar (`GUEST`).
2. **Tiket Kapabilitas Sementara (*Scoped Turn Capability*)**:
   - Setiap pesan masuk menerbitkan tiket otorisasi unik dengan masa berlaku terbatas (TTL 10 menit) yang mengikat `principal_jid`, `authority`, dan `message_id`. Seluruh eksekusi tool memverifikasi tiket ini.
3. **Audit Trail Rantai Hash SHA-256 Bergaya Blockchain (NIST AU-10 & ISO 42001)**:
   - Setiap pemanggilan tool oleh AI dienkapsulasi dengan hash SHA-256 yang merantai blok sebelumnya ($H_n = \text{SHA256}(H_{n-1} \parallel \dots)$), menjamin integritas bukti forensik anti-manipulasi.
4. **Pertahanan Kebocoran Nomor WhatsApp (*Phone Leakage Cognitive Fence*)**:
   - Nomor tak dikenal otomatis diperlakukan sebagai tamu (`GUEST`), dibatasi kuotanya (*anti-abuse rate limiting*), dan dilarang mengakses dokumen internal instansi.
5. **Kepatuhan Privasi Mandiri UU PDP (*Right to be Forgotten*)**:
   - Pengguna memiliki kendali transparansi penuh atas data profilnya (`/profilku`) serta hak penghapusan mandiri seketika (`/lupakansaya`).
