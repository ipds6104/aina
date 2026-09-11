# 🌸 Aina (あいな)

**Aina** adalah persona asisten AI *self-hosted* yang terintegrasi ke WhatsApp (melalui gateway Whatsmeow) dan ditenagai langsung oleh mesin agentik **Google Antigravity CLI (`agy`)**.

Aina dirancang untuk bertindak seperti **rekan kerja baru (junior engineer / tech staff)** di grup kerja maupun percakapan pribadi (DM): cekatan, solutif, ramah, to-the-point, dan proaktif meminta klarifikasi jika suatu instruksi multitafsir agar tidak salah arah.

---

## 🚀 Getting Started & Panduan Deploy ke Coolify (First Deploy)

Repo ini sudah dilengkapi dengan **`Dockerfile` multi-stage**, **`docker-entrypoint.sh`**, dan dukungan penuh **Environment Variables**. Pada saat pertama kali dideploy ke **Coolify** (dengan auto-deploy GitHub App), Aina dapat langsung aktif dan berjalan lancar jika kamu telah menyiapkan konfigurasi berikut:

### 1. Prasyarat Sebelum Deploy
Pastikan kamu sudah menyiapkan:
1. **Instance Whatsmeow**: Sudah berjalan dan memiliki:
   - Base URL (contoh: `http://whatsmeow-host:3000`)
   - API Key / Secret Token
   - Nomor WhatsApp bot yang sudah tersambung (JID: `628xxxxxxxxxx@s.whatsapp.net`)
2. **Kredensial Antigravity CLI**:
   - Salin isi dari berkas OAuth lokal kamu di `~/.gemini/antigravity-cli/antigravity-oauth-token` (untuk diisi ke env `AINA_OAUTH_TOKEN`), **ATAU**
   - Gunakan `GEMINI_API_KEY` jika menggunakan akses developer key.

---

### 2. Langkah Deploy di Coolify

1. **Hubungkan Repository ke Coolify**:
   - Di Dashboard Coolify, pilih **Projects** -> Pilih Environment -> Klik **+ New Resource** -> Pilih **Application**.
   - Pilih **GitHub App**, lalu pilih repositori **`aina`** ini.
   - Branch: `main` atau `master`.
   - Build Pack: Pilih **Dockerfile** (Coolify akan otomatis mendeteksi [`Dockerfile`](Dockerfile) yang sudah disediakan).

2. **Atur Environment Variables di Coolify**:
   Buka tab **Environment Variables** di Coolify, lalu tambahkan variabel dasar berikut (lihat template di [`.env.example`](.env.example)):

   ```ini
   PORT=8090
   SERVER_PORT=8090

   # Whatsmeow Gateway
   WHATSMEOW_BASE_URL=http://<IP_ATAU_DOMAIN_WHATSMEOW>:3000
   WHATSMEOW_API_KEY=<SECRET_API_KEY_KAMU>
   WHATSMEOW_BOT_JID=628xxxxxxxxxx@s.whatsapp.net
   WHATSMEOW_BOT_NAME=Aina

   # Antigravity Model & Sandbox
   AGENT_MODEL=gemini-3.8-flash-high
   AGENT_WORKSPACE=/app/workspace
   DATABASE_PATH=/app/data/aina.db
   ```
   *(Opsional: Kamu juga bisa langsung mengisi `AINA_OAUTH_TOKEN` di sini jika tidak ingin melewati web wizard).*

3. **Atur Persistent Storage (Volumes) di Coolify**:
   Agar riwayat chat, database SQLite, dan token sesi tidak ter-reset setiap kali ada commit/re-deploy otomatis dari GitHub, buka tab **Storages / Persistent Storage** di Coolify dan tambahkan mount volume berikut:

   | Destination Path | Keterangan |
   | :--- | :--- |
   | `/app/data` | Menyimpan SQLite database (`aina.db`) & mapping percakapan |
   | `/root/.gemini` | Menyimpan token autentikasi & cache sesi bawaan Antigravity |
   | `/app/workspace` | Sandbox folder tempat Aina membuat file/skrip jika diminta koding |

4. **Klik Deploy**:
   - Klik tombol **Deploy** di Coolify. Coolify akan otomatis mengompilasi binary Rust dan menyiapkan binary Antigravity CLI.

---

### 3. Autentikasi Pertama Kali via Web Wizard (`/setup`)

Setelah aplikasi Coolify selesai di-deploy dan berstatus **Healthy**:

1. Buka URL aplikasi kamu di browser:
   ```
   https://aina.domainkamu.com (atau http://IP_SERVER:8090)
   ```
2. Halaman akan menampilkan **Web Onboarding Wizard** yang dilindungi standar keamanan industri:
   - **Lihat Kode Setup**: Buka tab **Logs** di Coolify, kamu akan melihat banner:
     ```text
     🔐 SETUP / ADMIN CODE: AINA-XXXXXX
     ```
     *(Atau gunakan nilai `ADMIN_KEY` jika kamu sudah mengaturnya di Environment Variables).*
   - **Ambil Token dari Laptop**: Di terminal laptop lokalmu yang sudah login Antigravity, jalankan:
     ```bash
     cat ~/.gemini/antigravity-cli/antigravity-oauth-token
     ```
   - Masukkan **Kode Setup** dan tempelkan seluruh teks JSON token ke form web wizard.
   - Klik tombol **"Verifikasi & Simpan Token"**.
3. Aina akan memverifikasi token ke Antigravity secara real-time. Jika valid, endpoint setup otomatis terkunci demi keamanan, dan dashboard langsung berubah menjadi **"ONLINE & TERAUTENTIKASI"**!

---

### 4. Uji Coba Cepat via Simulator Web (100% Real, Zero Mocks)

Di dashboard web (`/`), terdapat fitur **Simulator Percakapan WhatsApp**:
- Kamu bisa menguji langsung bagaimana Aina merespon pesan pribadi (DM) maupun obrolan grup kerja.
- Uji fitur **Gatekeeper**: Coba kirim pesan grup tanpa tag (@Aina), dan lihat bagaimana Aina menyimak (*RecordOnly*) tanpa membuat kegaduhan/spam di grup.
- Uji kemampuan koding: Minta Aina membuat script atau cek status teknis, dan perhatikan respon agentik aslinya di layar browser sebelum menghubungkannya ke nomor WhatsApp asli.

---

### 5. Hubungkan Webhook Whatsmeow ke Aina

1. Buka konfigurasi instance Whatsmeow milikmu.
2. Atur URL Webhook masuk ke:
   ```
   POST https://aina.domainkamu.com/webhook
   ```
3. Selesai! Coba kirim WhatsApp ke nomor bot (DM):
   > *"Halo Aina, salam kenal!"*
   
   Aina akan segera membalas layaknya rekan kerja baru!

---

## 🏛️ Arsitektur Sistem (Hexagonal / Ports & Adapters)

Proyek ini dibangun menggunakan bahasa **Rust** dengan arsitektur heksagonal murni:

```text
src/
├── core/                        # DOMAIN & BUSINESS LOGIC (Murni tanpa dependensi HTTP/DB)
│   ├── domain/
│   │   ├── message.rs           # Entitas Pesan, Pengirim, Tipe Chat (DM/Group)
│   │   ├── gatekeeper.rs        # Logika filter: Kapan Aina harus menjawab vs menyimak
│   │   └── persona.rs           # Prompt builder dengan penyuntikan persona & konteks
│   ├── ports/
│   │   ├── agent_engine.rs      # Trait: Kontrak eksekusi ke Antigravity CLI
│   │   ├── whatsapp.rs          # Trait: Kontrak pengiriman pesan & status typing
│   │   └── session_store.rs     # Trait: Kontrak mapping Chat JID <-> UUID percakapan
│   └── usecases/
│       ├── process_message.rs   # Alur orkestrator pemrosesan pesan masuk
│       └── scheduled_tick.rs    # Rutinitas terjadwal / cron background
│
├── adapters/
│   ├── driving/                 # PRIMARY ADAPTERS
│   │   ├── webhook.rs           # Axum HTTP Server (/webhook, /health) penerima whatsmeow
│   │   └── scheduler.rs         # Tokio background ticker
│   └── driven/                  # SECONDARY ADAPTERS
│       ├── agy_cli.rs           # Tokio process runner memanggil `agy` secara headless
│       ├── whatsmeow_http.rs    # Reqwest HTTP client ke REST API Whatsmeow
│       └── sqlite_store.rs      # Rusqlite untuk persistensi sesi & riwayat lokal
│
├── config/                      # Pengaturan aplikasi & pemuat persona
│   ├── config.yaml              # Konfigurasi gateway, port, dan model
│   └── persona.md               # Definisi kepribadian Aina (dapat diubah user kapan saja)
│
├── workspace/                   # Sandbox folder tempat Aina membuat skrip & mengeksekusi kode
└── main.rs                      # Composition root (Dependency Injection wiring)
```

---

## 🧠 Pemanfaatan Fitur Native Antigravity CLI (`agy`)

Daripada membuat sistem memori, parser LLM, atau runtime sandbox dari nol, Aina memanfaatkan mekanisme bawaan Antigravity:

1. **Native Session Memory (`--conversation <uuid>`)**:
   Antigravity CLI menyimpan riwayat percakapan dalam SQLite di `~/.gemini/antigravity-cli/conversations/<uuid>.db`. Aina cukup memetakan `WhatsApp Chat JID -> UUID`, sehingga Antigravity secara native mengingat semua konteks obrolan masa lalu.
2. **Headless Execution (`-p` & `--output-format json`)**:
   Aina memanggil `agy` non-interaktif dengan `--dangerously-skip-permissions`, memungkinkan eksekusi agentik otomatis (membuat skrip, menjalankan bash, dsb.) dengan hasil JSON terstruktur.
3. **Agentic Capabilities**:
   Jika diminta membuat kode atau memeriksa bug, Aina dapat mengeksekusi bash dan memverifikasi script langsung di folder `workspace/`.

---

## 🎭 Kustomisasi Personality & Konteks Multi-Workspace

Pengguna dapat mengubah identitas, sektor instansi, serta kepribadian Aina **tanpa perlu mengubah kode program**:

1. **Konteks Organisasi / Instansi**:
   👉 [`config/organization.md`](config/organization.md) (atau set via env `AINA_ORGANIZATION_TEXT`)
   - Mendefinisikan sektor instansi (misalnya: Badan Pusat Statistik / BPS, startup teknologi, instansi dinas, atau perusahaan logistik).
   - Menentukan peran spesifik Aina (misalnya: Mitra Pengolahan Data Statistik, Junior DevOps, dsb.).
   - Dilengkapi template adaptasi instansi yang siap digunakan.

2. **Persona & Gaya Komunikasi**:
   👉 [`config/persona.md`](config/persona.md) (atau set via env `AINA_PERSONA_TEXT`)
   - **Cekatan & Solutif**: Memberikan solusi konkret yang siap pakai.
   - **Basa-basi Seperlunya**: Low-noise, to-the-point, santun namun akrab.
   - **Proactive Clarification**: Jika instruksi ambigu, meminta klarifikasi terarah sebelum bertindak.

---

## 🧭 Hierarki Epistemik Utama & Profiling Wewenang (OpSec)

Aina beroperasi di bawah 4 pilar epistemik kritis untuk menjamin kehati-hatian, mencegah *hallucination*, dan menangkal eksploitasi *social engineering*:

```
[1. TABAYYUN (QS. Al-Hujurat: 6)] ──> Filter Masuk: Verifikasi kebenaran klaim & kredibilitas sebelum bereaksi.
         │
         ▼
[2. TAWAQQUF (QS. Al-Isra: 36)]   ──> Anti-Spekulasi: Tahan diri membuat asumsi jika bukti belum kuat.
         │
         ▼
[3. OPSEC & PROFILING]            ──> Batas Wewenang: Identifikasi lawan bicara & batasan perintahnya.
         │
         ▼
[4. AHLUDZ-DZIKRI (QS. An-Nahl: 43)] ─> Solusi Otoritatif: Konsultasi ke dokumentasi primer jika ragu.
```

### 👤 Profiling Memori Pengguna di SQLite
Aina secara otonom mengingat lawan bicaranya ke dalam tabel `user_profiles` di SQLite:
- **`ADMIN`** (Dapat diset via env `ADMIN_JID=628xxxx@s.whatsapp.net`): Memiliki wewenang penuh atas konfigurasi dan penugasan strategis.
- **`STAFF`**: Rekan kerja kantor. Berhak meminta bantuan coding, pengolahan data, pembuatan script, dan monitoring.
- **`GUEST` / `EXTERNAL`**: Pihak luar / nomor asing. Aina bersikap adil dan sopan (*al-qist*), tetapi memiliki perimeter keamanan ketat (**Strict OpSec**): dilarang keras membocorkan token rahasia, data internal kantor, kredensial, atau mengeksekusi perintah destruktif sepihak.

---

## 🛡️ Etika Grup & Gatekeeper

Aina tidak akan menjadi spammer di grup kerja Anda:
- **Private Chat (DM)**: Selalu dijawab secara personal.
- **Grup WhatsApp**: Hanya menjawab jika di-mention (`@Aina`), di-reply kutipannya (*quoted message*), atau namanya dipanggil secara jelas. Pesan lainnya hanya dicatat secara pasif (*ambient context*) agar Aina paham konteks tim tanpa menyela obrolan.

---

## 💻 Pengujian Lokal (Development)

Jika ingin menjalankan Aina di komputer lokal:

```bash
# 1. Jalankan unit test
cargo test

# 2. Jalankan aplikasi secara langsung
cargo run
```
Endpoint lokal akan berjalan di `http://127.0.0.1:8090/health`.
