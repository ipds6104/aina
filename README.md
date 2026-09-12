# 🌸 Aina (あいな) - Self-Hosted Agentic AI Co-Worker

[![Rust](https://img.shields.io/badge/Language-Rust_2024-orange.svg)](https://www.rust-lang.org/)
[![Engine](https://img.shields.io/badge/Agent_Engine-Google_Antigravity_CLI_(agy)-blue.svg)](https://antigravity.google)
[![Gateway](https://img.shields.io/badge/WhatsApp-Whatsmeow_HTTP-25D366.svg)](https://github.com/tulir/whatsmeow)
[![Architecture](https://img.shields.io/badge/Architecture-Hexagonal_/_Clean_Architecture-9cf.svg)]()
[![License](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)

**Aina** adalah persona asisten AI *self-hosted* yang terintegrasi secara langsung ke WhatsApp (melalui gateway Whatsmeow) dan ditenagai oleh mesin agentik **Google Antigravity CLI (`agy`)**.

Aina dirancang bukan sebagai bot CS yang kaku, melainkan sebagai **rekan kerja teknis (software engineer / staf data)** di grup WhatsApp maupun percakapan pribadi:
- ⚡ **Cekatan & Solutif**: Memberikan solusi konkret, siap pakai, dan mampu mengeksekusi kode secara nyata di terminal.
- 💬 **Basa-Basi Seperlunya**: *Low-noise*, to-the-point, santun, dan bersahabat.
- 🧭 **Proactive Clarification & Tabayyun**: Bertanya dan meminta klarifikasi terarah jika instruksi multitafsir, serta menahan diri (*Tawaqquf*) bila ditanya data sensitif oleh pihak luar sebelum meminta izin User Companion.
- 🛡️ **Gatekeeper Cerdas**: Tidak *spamming* di grup kantor (hanya menjawab jika di-tag/disebut, dan mencatat percakapan pasif sebagai konteks).
- 👥 **Dynamic Profiling Memory**: Mengenali peran dan wewenang rekan kerja (`admin`, `staff`, `guest`) dengan memori SQLite permanen (`aina user`), mencegah *context loss* meski berbulan-bulan tidak berinteraksi.
- 🔍 **Instant Temporal Search (<5ms)**: Pencarian riwayat percakapan SQLite FTS5 BM25 + filter waktu presisi (`--since 7d`, `--from YYYY-MM-DD --to YYYY-MM-DD`) untuk mencegah halusinasi waktu (*anachronism*).
- 📱 **Dual-WhatsApp Pipeline (Multi-Session)**: Mendukung nomor bot khusus (`PrimaryBot`) sekaligus akun pribadi pengguna (`UserCompanion` / *Shadow Sensor*). Aina dapat mencerna konteks puluhan grup kantor tanpa perlu meminta admin memasukkan nomor bot baru.
- 🧠 **Dynamic Model Switching**: Bawaan cepat & cerdas dengan **Google Gemini** (5–15 detik), dengan opsi eskalasi ke **Claude Opus** khusus tugas kompleks melalui CLI native `aina model`.
- 📁 **Workspace-Agnostic & Structured Knowledge Base**: Basis pengetahuan tumbuh secara organik per proyek/instansi tanpa tercampur baur.

---

## 📑 Daftar Isi (Table of Contents)
- [📱 Dual-WhatsApp Pipeline: Bot Resmi & Companion Sensor](#-dual-whatsapp-pipeline-bot-resmi--companion-sensor)
  - [📲 Panduan Praktis Menghubungkan Sensor WhatsApp](#-panduan-praktis-menghubungkan-sensor-whatsapp-getting-started)
- [🏗️ Alur Konteks ke Knowledge Base (Pipeline Architecture)](#️-alur-konteks-ke-knowledge-base-pipeline-architecture)
- [🚀 Panduan Memulai Cepat (Getting Started)](#-panduan-memulai-cepat-getting-started-in-5-minutes)
  - [📋 Prasyarat Sistem (Prerequisites)](#-prasyarat-sistem-prerequisites)
  - [Langkah 1: Pilih Sumber Knowledge Base](#langkah-1-pilih-sumber-knowledge-base-anda)
  - [Langkah 2: Deploy & Jalankan Aina (Coolify / Docker / Lokal)](#langkah-2-deploy--jalankan-aina)
- [🔐 Setup Autentikasi Pertama Kali (`/setup`)](#-setup-autentikasi-pertama-kali-setup)
- [🧪 Web Simulator (Real End-to-End Testing)](#-web-simulator-real-end-to-end-testing)
- [👥 Profiling Memory & Manajemen Rekan Kerja (`aina user`)](#-profiling-memory--manajemen-rekan-kerja-aina-user)
- [🤖 Manajemen Model AI (Gemini-First Priority & Native CLI)](#-manajemen-model-ai-gemini-first-priority--native-cli)
- [📁 Agnostic Workspaces & Structured Knowledge Base](#-agnostic-workspaces--structured-knowledge-base)
- [⚡ Unified Native Rust CLI (`aina`)](#-unified-native-rust-cli-aina)
- [🧹 Linter Kerapian & Closed-Loop Auto-Healing (`kb_linter.py`)](#-linter-kerapian--closed-loop-auto-healing-kb_linterpy)
- [🛡️ Audit Trail CLI Antigravity (`audit_agent.py`)](#️-audit-trail-cli-antigravity-audit_agentpy)
- [📦 Engine Arsip & Ekspor Chat WhatsApp (Temporal Search & Import)](#-engine-arsip--ekspor-chat-whatsapp-temporal-search--import)
- [🔒 Keamanan & Privasi Data (Security & Privacy)](#-keamanan--privasi-data-security--privacy)
- [⚙️ Ringkasan Environment Variables](#️-ringkasan-environment-variables)
- [🏛️ Arsitektur Kode (Clean Architecture)](#️-arsitektur-kode-clean-architecture)
- [❓ Tanya Jawab & Troubleshooting (FAQ)](#-tanya-jawab--troubleshooting-faq)
- [📜 Lisensi & Panduan Kontribusi](#-lisensi--panduan-kontribusi)

---

## 📱 Dual-WhatsApp Pipeline: Bot Resmi & Companion Sensor

Salah satu tantangan terbesar asisten AI WhatsApp di dunia nyata adalah **birokrasi grup kerja**: meminta admin memasukkan nomor bot baru ke grup kantor, klien, atau kepanitiaan sering kali lambat, sulit, atau dilarang oleh regulasi privasi.

Aina memecahkan masalah ini secara revolusioner melalui **Dual-WhatsApp Architecture**:

```text
┌────────────────────────────────────────────────────────────────────────────────────────┐
│                                DUAL-WHATSAPP PIPELINE                                  │
├──────────────────────────────────────────┬─────────────────────────────────────────────┤
│  SESI 1: BOT RESMI (Dedicated Bot)       │  SESI 2: COMPANION SENSOR (Akun Pribadi)    │
│  Nomor: WHATSMEOW_BOT_JID                │  Nomor: WHATSMEOW_COMPANION_JID             │
├──────────────────────────────────────────┼─────────────────────────────────────────────┤
│  • Melayani DM dari siapa saja           │  • Zero Privacy Leakage:                    │
│  • Melayani grup yang mengundang bot     │    DM personal dari kontak diabaikan 100%   │
│  • Respon ramah dan profesional          │  • Shadow Sensor (Perekam Pasif):           │
│  • Cocok untuk kontak publik & kantor    │    Mencatat obrolan grup kantor ke SQLite   │
│                                          │  • Explicit Invocation:                     │
│                                          │    Owner panggil `!aina` di grup -> Balas   │
│                                          │  • Chat to Self ("Message Yourself"):       │
│                                          │    Ketik catatan ke nomor sendiri -> Balas  │
└──────────────────────────────────────────┴─────────────────────────────────────────────┘
```

Periksa status kedua sesi kapan saja via terminal:
```bash
aina status
# atau
aina whatsapp status --json
```

### 📲 Panduan Praktis Menghubungkan Sensor WhatsApp (Getting Started)

Aina memisahkan **Gateway WhatsApp (Whatsmeow)** dan **Otak Agentik (Aina)**. Whatsmeow menangani koneksi soket WhatsApp web, sedangkan Aina menangani pemikiran, privasi, dan kristalisasi pengetahuan.

#### 1. Setup Sesi 1: Bot Utama Resmi (`PrimaryBot`)
1. **Buat Sesi di Whatsmeow Gateway**:
   Buka Whatsmeow instance Anda, buat sesi baru (misal ID sesi: `default`).
2. **Scan QR Code Bot**:
   Di HP nomor khusus bot: buka **WhatsApp** -> **Perangkat Tertaut (Linked Devices)** -> **Tautkan Perangkat** -> scan QR Code yang muncul di Whatsmeow.
3. **Konfigurasi Environment Variable**:
   Salin JID bot yang muncul (format: `628xxxxxxxxxx@s.whatsapp.net`) ke konfigurasi `.env` / Coolify:
   ```ini
   WHATSMEOW_BASE_URL=https://wa.domainkamu.com
   WHATSMEOW_API_KEY=secret_key_kamu
   WHATSMEOW_BOT_JID=6289625345646@s.whatsapp.net
   WHATSMEOW_BOT_NAME=Aina
   WHATSMEOW_BOT_SESSION_ID=default
   ```

#### 2. Setup Sesi 2: User Companion (Nomor Pribadi / Shadow Sensor) [Opsional]
1. **Buat Sesi Companion di Whatsmeow**:
   Buat sesi kedua dengan ID unik (contoh: `companion`).
2. **Scan QR Code dari WhatsApp Pribadi Anda**:
   Di WhatsApp HP pribadi Anda (yang sudah bergabung di puluhan grup kantor): buka **Perangkat Tertaut (Linked Devices)** -> **Tautkan Perangkat** -> scan QR code sesi `companion`.
3. **Tambahkan Konfigurasi Companion di Aina**:
   ```ini
   WHATSMEOW_COMPANION_JID=628111222333@s.whatsapp.net
   WHATSMEOW_COMPANION_NAME=Ihza (Personal)
   WHATSMEOW_COMPANION_SESSION_ID=companion
   ```

#### 3. Arahkan Webhook Whatsmeow ke Aina
Pada konfigurasi Whatsmeow Gateway Anda (baik sesi bot maupun companion), arahkan webhook URL ke:
```text
https://aina.domainkamu.com/webhook (atau http://IP_SERVER:8090/webhook)
```
Aina otomatis membedakan pesan dari bot resmi vs akun pribadi Anda.

#### 4. Uji dan Verifikasi Seketika
1. Jalankan di server: `aina status` untuk memastikan kedua sesi berstatus **Aktif & Terhubung**.
2. Buka dashboard web `https://aina.domainkamu.com`, masuk ke **Simulator Percakapan**, dan pilih mode **👥 Sesi Companion** untuk menguji alur privasi secara langsung tanpa HP!

---

## 🏗️ Alur Konteks ke Knowledge Base (Pipeline Architecture)

Aina memproses percakapan WhatsApp menjadi basis pengetahuan terstruktur yang rapi, terisolasi, dan mudah dicari (*high retrievability*):

```text
[WhatsApp DM / Grup / Web Simulator / Companion Sensor]
                 │
                 ▼
     [1. Gatekeeper & Epistemic Filter] ──(Bukan Tag / Ambient)──> [Simpan Pasif di SQLite FTS5]
                 │ (Di-mention / Pesan Pribadi / Command !aina)
                 ▼
    [2. Dynamic Workspace Router]
                 ├── /workspace <nama> (Eksplisit)
                 ├── Deteksi Topik / Nama Grup (Inferensi)
                 └── Percakapan Harian ──> [workspaces/default/]
                 │
                 ▼
  [3. Antigravity Agent Execution (Tri-Track Extractor)]
      ├── Jalur 1: Balasan Teks WhatsApp (Anti-Tabel Markdown, Ramah HP)
      ├── Jalur 2: Kristalisasi Pengetahuan (knowledge/facts.md & procedures.md)
      └── Jalur 3: Penyimpanan Berkas & Skrip (data/*.xlsx, *.csv & scripts/*.py)
                  │
                  ▼
  [4. In-Process Knowledge Grooming Engine (aina kb groom / Background Scheduler)]
       └── Pembaruan Indeks Otomatis (knowledge/index.md) -> Progressive Retrieval
```

---

### 🚀 Panduan Memulai Cepat (Getting Started in 5 Minutes)

Repositori ini sepenuhnya **Agnostic & Clone-Ready**. Aina memisahkan secara bersih antara **Aplikasi Engine (`aina`)** dan **Penyimpanan Knowledge Base (`workspaces`)**.

```text
┌──────────────────────────────────────────────────┐        ┌──────────────────────────────────────────────────┐
│      1. AINA ENGINE (Stateless Git Repo)         │        │    2. KNOWLEDGE BASE VAULT (Stateful Storage)    │
│  /root/projects/aina (Kode Rust, Webhook, CLI)   │ ────►  │  /var/lib/aina/workspaces/ atau Repo Git Tim     │
│  Bebas `git pull` kapan saja tanpa merusak data! │        │  knowledge/ (Fakta, SOP) & data/ (SQLite FTS5)   │
└──────────────────────────────────────────────────┘        └──────────────────────────────────────────────────┘
```

---

### 📋 Prasyarat Sistem (Prerequisites)

Sebelum memulai, pastikan lingkungan Anda memenuhi spesifikasi berikut:
- **Sistem Operasi**: Linux (`x86_64` atau `aarch64` / ARM64, termasuk Ubuntu di VPS/Cloud maupun container), macOS, atau Windows (via WSL2 / Docker).
- **Runtime Server / Kontainer**: Docker & Docker Compose (v2+), atau [Coolify](https://coolify.io) untuk deployment produksi *zero-downtime*.
- **WhatsApp Gateway**: Instance aktif gateway [Whatsmeow](https://github.com/tulir/whatsmeow) HTTP REST API untuk mengelola koneksi soket WhatsApp.
- **AI Agent Engine**: [Google Antigravity CLI (`agy`)](https://antigravity.google) terpasang atau siapkan string OAuth Token untuk diinput via Web Setup Wizard (`/setup`).
- **Rust Toolchain (Khusus Kompilasi dari Source / Local Dev)**: Rust versi `1.85+` (Edition 2024).

---

### Langkah 1: Pilih Sumber Knowledge Base Anda

Sebelum menjalankan Aina, tentukan bagaimana basis pengetahuan Anda akan disimpan:

#### Skenario A: Memulai dari Nol (Fresh Workspace)
Aina secara otomatis menyiapkan starter template default (`GEMINI.md`, `facts.md`, `procedures.md`) di dalam folder workspace yang ditunjuk. Anda tidak perlu setup manual.

#### Skenario B: Menghubungkan Repositori GitHub Knowledge Base yang Sudah Ada
Jika tim/organisasi Anda sudah memiliki repositori GitHub berisi dokumentasi/SOP (contoh: `https://github.com/ipds6104/knowledge-base.git`):
```bash
# Clone repositori knowledge base ke folder penyimpanan server
aina clone https://github.com/ipds6104/knowledge-base.git /var/lib/aina/workspaces/ipds
# Aina otomatis memvalidasi struktur dokumen dan mengompilasi katalog `knowledge/index.md`!
```

#### Skenario C: Workspace Lokal Di-publish ke GitHub Baru via GitHub CLI (`gh`)
Jika Anda sudah memiliki workspace lokal dan ingin langsung membuat repositori GitHub pribadi/organisasi dengan 1 perintah:
```bash
# Cek autentikasi GitHub CLI
aina workspace gh-status

# Buat repo GitHub baru dari workspace aktif dan otomatis push
aina workspace gh-create knowledge-base-ipds
```

---

### Langkah 2: Deploy & Jalankan Aina

#### Opsi 1: Deploy di Coolify (Paling Direkomendasikan untuk Produksi)

1. **Buat Resource Baru di Coolify**:
   - Pilih **Projects** -> Environment -> Klik **+ New Resource** -> **Application**.
   - Pilih **GitHub App** -> Pilih repositori **`aina`** Anda -> Branch `main`.
   - Build Pack: Pilih **Dockerfile**.
2. **Atur Environment Variables** (Tab *Environment Variables* di Coolify):
   ```ini
   PORT=8090
   SERVER_PORT=8090
   
   # Sesi 1: Bot Utama Resmi (Dedicated Bot)
   WHATSMEOW_BASE_URL=https://wa.domainkamu.com
   WHATSMEOW_API_KEY=secret_key_kamu
   WHATSMEOW_BOT_JID=62896xxxxxxxx@s.whatsapp.net
   WHATSMEOW_BOT_NAME=Aina
   WHATSMEOW_BOT_SESSION_ID=default

   # Sesi 2: User Companion (Nomor WhatsApp Pribadi / Shadow Sensor) [Opsional]
   # WHATSMEOW_COMPANION_JID=628111222333@s.whatsapp.net
   # WHATSMEOW_COMPANION_NAME=Ihza (Personal)
   # WHATSMEOW_COMPANION_SESSION_ID=companion

   AGENT_MODEL=gemini-3.8-flash-medium
   AGENT_WORKSPACE=/app/workspaces/default
   DATABASE_PATH=/app/data/aina.db
   ```
3. **Atur Persistent Storage (Volumes)**:
   Di tab **Storages**, tambahkan persistent storage:
   | Volume Name | Destination Path | Keterangan |
   | :--- | :--- | :--- |
   | `aina_data` | `/app/data` | Database SQLite (`aina.db`) & riwayat pesan |
   | `aina_gemini` | `/root/.gemini` | Kredensial OAuth Antigravity & cache CLI |
   | `aina_workspaces`| `/app/workspaces` | Wadah knowledge base & arsip obrolan |
   *(Atau arahkan Destination Path `/app/workspaces/default` langsung ke host bind-mount dari repo knowledge base tim).*
4. **Klik Deploy**: Coolify akan mengompilasi dan menjalankan Aina secara otomatis.

---

#### Opsi 2: Menggunakan Docker Compose (VPS / Server Mandiri)

1. **Klon Repositori Engine**:
   ```bash
   git clone https://github.com/ipds6104/aina.git
   cd aina
   ```
2. **Salin Template Konfigurasi**:
   ```bash
   cp .env.example .env
   # Sesuaikan URL whatsmeow, BOT_JID, dan AGENT_WORKSPACE Anda
   ```
3. **Jalankan Aplikasi**:
   ```bash
   docker compose up -d
   docker compose logs -f aina
   ```

---

#### Opsi 3: Menjalankan di Komputer Lokal (Local Development)

```bash
# 1. Pastikan Rust dan Antigravity CLI (agy) terpasang
curl -fsSL https://antigravity.google/cli/install.sh | bash

# 2. Cek status workspace aktif
aina workspace info

# 3. Jalankan server lokal
cargo run
```
Akses dashboard lokal di browser: `http://localhost:8090`.

---

## 🔐 Setup Autentikasi Pertama Kali (`/setup`)

Setelah server Aina aktif dan sehat (*healthy*):

1. Buka browser ke URL aplikasi Anda:
   ```text
   https://aina.domainkamu.com (atau http://IP_SERVER:8090)
   ```
2. Anda akan disambut oleh **Web Setup Onboarding Wizard**:
   - **Cek Kode Setup**: Buka terminal log deployment Anda (di Coolify tab *Logs* atau `docker compose logs`), temukan:
     ```text
     🔐 SETUP / ADMIN CODE: AINA-XXXXXX
     ```
   - **Ambil Token Antigravity**: Di terminal laptop lokal Anda yang sudah login `agy`, jalankan:
     ```bash
     cat ~/.gemini/antigravity-cli/antigravity-oauth-token
     ```
   - Tempelkan kode setup dan seluruh JSON token pada form wizard, lalu klik **"Verifikasi & Simpan Token"**.
3. Aina akan memverifikasi token secara *real-time*. Endpoint setup otomatis terkunci, dan dashboard berubah menjadi **"ONLINE & TERAUTENTIKASI"**!
4. **Daftarkan Profil User Companion (Pemilik Utama)**:
   Di terminal server, daftarkan nomor WhatsApp pribadi Anda sebagai Administrator utama:
   ```bash
   aina user set <NOMOR_WHATSAPP_ANDA@s.whatsapp.net> --name "Nama Anda" --role "Owner & Lead" --authority admin --notes "Penanggung jawab sistem utama"
   ```
   Dengan langkah ini, Aina langsung mengenali Anda sebagai Companion terpercaya dengan hak wewenang penuh tanpa konfirmasi berulang!

---

## 🧪 Web Simulator (Real End-to-End Testing)

Di dashboard web (`https://aina.domainkamu.com`), tersedia kartu **Simulator Percakapan WhatsApp (Real Test)**:
- **Zero Mocks**: Pesan diuji melalui logika *Gatekeeper*, *Persona*, dan dieksekusi secara nyata oleh biner `agy`.
- **Pengujian Multi-Skenario**: Uji pesan pribadi (DM), obrolan grup dengan mention `@Aina`, maupun obrolan grup tanpa tag.
- **Pilihan Model AI Dinamis**: Pilih model per pengujian langsung dari dropdown simulator.
- **Salin Jawaban (📋)**: Terdapat tombol salin lengkap untuk memindahkan jawaban Aina dengan format rapi.

---

## 👥 Profiling Memory & Manajemen Rekan Kerja (`aina user`)

Untuk mencegah fenomena *context loss* (lupa profil rekan kerja atau izin yang pernah diberikan setelah berminggu-minggu), Aina dilengkapi sistem **Profiling Memory** lokal berbasis SQLite (`user_profiles`).

Aina membedakan 3 tingkatan otoritas (*Authority Matrix*):
* **`admin`**: Pemilik/User Companion utama dengan akses wewenang penuh.
* **`staff`**: Rekan kerja internal yang berhak meminta pembuatan script, analisis data, dan tugas kantor rutin.
* **`guest`**: Pihak eksternal / nomor baru yang belum diverifikasi (*Strict OpSec & Tabayyun*).

```bash
# 1. Daftarkan / Perbarui profil dan wewenang rekan kerja
aina user set 6289625345646@s.whatsapp.net \
  --name "Mas Companion" \
  --role "Owner & Lead Architect" \
  --authority admin \
  --notes "Penanggung jawab sistem utama, akses penuh"

# 2. Cek profil pengguna spesifik
aina user get 6289625345646@s.whatsapp.net
aina user get 6289625345646@s.whatsapp.net --json

# 3. Cari rekan kerja berdasarkan kata kunci
aina user search "Architect"

# 4. Tampilkan seluruh daftar rekan kerja terdaftar
aina user list
aina user list --json
```

> [!TIP]
> **Adab Tabayyun di Grup WhatsApp**: Bila orang berstatus `guest` meminta data sensitif atau instruksi sistem, Aina akan menahan diri (*Tawaqquf*) dan meminta izin kepada User Companion via japri (DM). Setelah diizinkan, Aina menyimpannya via `aina user set` sehingga Aina mengingat izin tersebut secara permanen tanpa bertanya ulang di masa depan!

---

## 🤖 Manajemen Model AI (Gemini-First Priority & Native CLI)

Aina mengutamakan efisiensi dan kecepatan respons dengan memprioritaskan keluarga model **Google Gemini** sebagai *default*:

| Model | Karakteristik | Estimasi Latensi | Penggunaan Terbaik |
| :--- | :--- | :--- | :--- |
| **`gemini-3.8-flash-medium`** | **Default / Rekomendasi** | **5 – 15 detik** | Cepat, seimbang, pemikiran mendalam, bebas timeout. |
| **`gemini-3.8-flash-high`** | Penalaran Tinggi | 15 – 35 detik | Arsitektur kompleks, refaktor besar, analisis data rumit. |
| **`gemini-3.8-flash-low`** | Respons Kilat | < 3 detik | Sapaan santai, konfirmasi cepat, obrolan kasual. |
| **`gemini-3.1-pro-high`** | Deep Coding | 20 – 45 detik | Debugging kode tingkat lanjut, penulisan script besar. |
| **`claude-opus-4-6-thinking`** | **Khusus Eksplisit** | 60 – 120 detik | Hanya aktif jika user meminta: *"Aina, pakai model opus"*. |
| **`claude-sonnet-4-6`** | Penalaran Menengah-Tinggi | 30 – 60 detik | Alternatif penalaran Claude untuk analisis mendalam. |

### Cara Mengganti Model AI:
1. **Via Native Rust CLI (Rekomendasi)**:
   ```bash
   aina model get                           # Tampilkan model aktif saat ini
   aina model list                          # Daftar model yang didukung & status aktif
   aina model set gemini-3.8-flash-high     # Ganti model seketika di runtime
   ```
2. **Via Chat WhatsApp / Simulator**:
   - Cek model aktif: `/model status` atau `/model list`
   - Ganti model instan: `/model <nama_model>` (contoh: `/model gemini-3.8-flash-high` atau `/model opus`)
3. **Via Web Dashboard**:
   - Klik **⚡ Ganti** pada info *Model AI Aktif* di dashboard utama (`https://aina.domainkamu.com`).
4. **Via Skrip Python (Legacy Fallback)**:
   ```bash
   python3 scripts/model_control.py set gemini-3.8-flash-high
   ```
5. **Via REST API (Automasi / CI)**:
   ```bash
   curl -X POST http://localhost:8090/api/model \
     -H "Content-Type: application/json" \
     -H "X-Admin-Key: AINA-XXXXXX" \
     -d '{"model": "gemini-3.8-flash-high"}'
   ```

---

## 📁 Agnostic Workspaces & Structured Knowledge Base

Aina tidak membatasi pengguna pada satu struktur proyek yang kaku. Setiap proyek atau tim kerja dapat memiliki workspace dan basis pengetahuan mandiri yang tumbuh secara organik.

### Struktur Standar Setiap Workspace:
```text
workspaces/<nama_workspace>/
├── GEMINI.md                    # 🛡️ Aturan domain, batasan etika, format data
├── knowledge/                   # 🧠 Knowledge Base terstruktur
│   ├── index.md                 # Katalog ringkasan topik (diperbarui oleh grooming)
│   ├── facts.md                 # Kumpulan fakta, keputusan rapat, parameter kunci
│   ├── procedures.md            # SOP, alur kerja, panduan langkah-demi-langkah
│   └── kegiatan/                # 🗓️ Arsip proyek & kegiatan berkala (berbasis waktu)
│       └── [nama-kegiatan]/
│           └── [periode]/       # Contoh: 2026-09, 2026-Q3, 2026
│               └── README.md    # Metadata YAML frontmatter (status, deadlines)
├── data/                        # 💾 Penyimpanan data tabular/dokumen (.csv, .xlsx, .json)
└── scripts/                     # ⚙️ Skrip automasi & data pipeline
    ├── model_control.py         # Utilitas kontrol model
    └── workspace_manager.py     # Utilitas manajemen workspace, kegiatan, & grooming
```

## ⚡ Unified Native Rust CLI (`aina`)

Selain berfungsi sebagai HTTP Webhook server daemon, biner utama `aina` juga menyediakan antarmuka CLI (*Command-Line Interface*) deterministik berkinerja tinggi (<3ms, 0 overhead runtime Python, 0 kuota token LLM). Seluruh mesin ini terintegrasi secara **in-process** di dalam satu biner mandiri (*single self-contained binary*):

```bash
# 1. Menjalankan daemon server Aina (Default)
aina
# atau: aina server / aina daemon

# 2. Status Pipeline Multi-Session WhatsApp (Bot Resmi & Companion Sensor)
aina status
aina whatsapp status --json

# 3. Pencarian Arsip Chat Super Cepat & Filter Temporal (FTS5 BM25, <5ms)
aina archive search "laporan" --since 7d
aina archive search "anggaran" --from 2026-09-01 --to 2026-09-10 --limit 5
aina archive search "kurikulum" --json

# 4. Impor & Statistik Arsip Obrolan WhatsApp (.zip / .txt)
aina archive import /path/ke/chat.zip --slug "tim-proyek"
aina archive stats --workspace default

# 5. Profiling Memory & Manajemen Wewenang Rekan Kerja (SQLite user_profiles)
aina user set 6289625345646@s.whatsapp.net --name "Mas Doni" --role "Data Analyst" --authority staff
aina user get 6289625345646@s.whatsapp.net
aina user list
aina user search "Analyst"

# 6. Kontrol & Pengalihan Model AI Runtime
aina model get
aina model list
aina model set gemini-3.8-flash-high

# 7. Validasi Kerapian Basis Pengetahuan (Linter & Auto-Heal)
aina kb lint --workspace default
aina kb lint --workspace default --auto-heal

# 8. Kompilasi Ulang Katalog `knowledge/index.md` Deterministik
aina kb groom --workspace default

# 9. Ringkasan Jadwal & Tenggat Waktu (Deadlines)
aina kb schedule --workspace default

# 10. Audit Trail Jejak Eksekusi Antigravity CLI
aina audit --limit 10
aina audit --query "git" --errors-only

# 11. Informasi Status & Metadata Workspace
aina workspace info
aina workspace info --json

# 12. Inisialisasi Workspace Baru Secara Deterministik
aina workspace init /var/lib/aina/workspaces/keuangan --title "Divisi Keuangan & Anggaran"

# 13. Sinkronisasi Git Dua Arah Otomatis (Pull Rebase + Safe Push)
aina sync
# atau: aina workspace sync --workspace default --message "docs: update SOP cuti"

# 14. Hubungkan Workspace ke Git Remote (GitHub/GitLab)
aina link https://github.com/ipds6104/knowledge-base.git
# atau: aina workspace link https://github.com/ipds6104/knowledge-base.git --workspace default

# 15. Manajemen GitHub CLI (`gh`) & Autentikasi Non-Interaktif / OAuth Device Flow
aina workspace gh-status
aina gh-device                             # Request kode Device Flow (misal: ABCD-1234)
aina gh-poll                               # Cek status verifikasi browser secara otomatis
aina gh-login ghp_xxxxxxxxxxxxxxxxxxxx     # Atau login langsung via Personal Access Token
aina workspace gh-create knowledge-base-ipds
aina workspace gh-create knowledge-base-public --public

# 16. Clone Knowledge Base Repositori Luar & Auto-Groom
aina clone https://github.com/ipds6104/knowledge-base.git /var/lib/aina/workspaces/ipds
```

> [!TIP]
> **In-Process Scheduler**: Background scheduler Aina (`ScheduledTickUseCase`) secara otomatis menjalankan pemindaian kerapian dan *auto-healing* basis pengetahuan secara internal di memori setiap 1 jam sekali tanpa perlu melakukan *forking child process*.

---

### Perintah Alternatif Skrip Python (Opsional / Scripting):
- **Lihat Seluruh Workspace**:
  ```bash
  python3 scripts/workspace_manager.py list
  ```
- **Buat Workspace Baru**:
  ```bash
  python3 scripts/workspace_manager.py create bps --title "Badan Pusat Statistik" --domain "Pengolahan data sensus & indikator makro"
  ```
- **Buat Kegiatan / Proyek Berkala**:
  ```bash
  python3 scripts/workspace_manager.py create-activity bps "Sakernas Agustus" "2026-08" \
    --kategori survey \
    --deadline "2026-08-10:Batas Pemutakhiran Rumah Tangga" \
    --deadline "2026-08-31:Batas Akhir Pencacahan CAPI"
  ```
- **Lacak Jadwal & Deadline (Deterministik)**:
  ```bash
  # Tampilkan seluruh jadwal
  python3 scripts/workspace_manager.py schedule
  
  # Filter deadline minggu ini atau bulan ini
  python3 scripts/workspace_manager.py schedule --week
  python3 scripts/workspace_manager.py schedule --month
  
  # Cek deadline yang terlewat (overdue)
  python3 scripts/workspace_manager.py schedule --overdue
  ```
- **Rapikan Knowledge Base (Grooming Routine)**:
  ```bash
  python3 scripts/workspace_manager.py groom bps
  ```
  *Skrip ini akan menyisir seluruh dokumen umum dan sub-kegiatan, mengekstrak metadata YAML frontmatter, menyusun matriks kegiatan aktif, dan mengompilasi agenda tenggat waktu terdekat ke dalam `knowledge/index.md` secara otomatis.*

---

## 🧹 Linter Kerapian & Closed-Loop Auto-Healing (`kb_linter.py`)

Aina dilengkapi detektor deterministik ultra-cepat (<50ms, **0 kuota token LLM**) untuk memvalidasi kriteria kerapian basis pengetahuan:
- ✅ Format penamaan folder kegiatan & periode (`kegiatan/<slug>/<periode>/`)
- ✅ Kelengkapan frontmatter YAML & validitas tanggal `deadlines`
- ✅ Deteksi keusangan indeks (`knowledge/index.md` stale check)
- ✅ Deteksi file nyasar/berkas sampah (`*.tmp`, `.DS_Store`, file biner di luar `data/`)

```bash
# Periksa kepatuhan kerapian seluruh workspace
python3 scripts/kb_linter.py

# Periksa workspace tertentu
python3 scripts/kb_linter.py default

# Jalankan dengan mode Closed-Loop Auto-Healing
# (Jika ada inkonsistensi, otomatis memicu grooming lalu verifikasi ulang hingga 100% rapi)
python3 scripts/kb_linter.py --auto-heal
```

> [!TIP]
> **Rekomendasi Cron Otomatis (Tiap 1 Jam)**:
> Anda dapat memasang cron job di server untuk menjalankan deteksi berkala tanpa membebani LLM:
> ```cron
> 0 * * * * python3 /app/scripts/kb_linter.py --auto-heal > /dev/null 2>&1
> ```

---

## 🛡️ Audit Trail CLI Antigravity (`audit_agent.py`)

Seluruh tindakan fisik Antigravity CLI (eksekusi perintah shell, pengeditan berkas, pencarian web) dicatat secara presisi di log native sistem (`transcript.jsonl` dan SQLite). Gunakan skrip `audit_agent.py` untuk mengaudit seluruh aktivitas Aina melalui terminal:

```bash
# 1. Tampilkan 20 aksi terakhir secara kronologis
python3 scripts/audit_agent.py --limit 20

# 2. Cari aksi berdasarkan kata kunci (perintah shell, path berkas, dsb.)
python3 scripts/audit_agent.py --query "git"
python3 scripts/audit_agent.py --query "workspace"

# 3. Filter berdasarkan jenis alat (run_command, replace_file_content, write_to_file)
python3 scripts/audit_agent.py --type run_command
python3 scripts/audit_agent.py --type replace_file_content

# 4. Filter rentang waktu (misal: 30 menit terakhir atau 24 jam terakhir)
python3 scripts/audit_agent.py --since 30m
python3 scripts/audit_agent.py --since 24h

# 5. Filter kegagalan / error saja
python3 scripts/audit_agent.py --errors-only

# 6. Ekspor hasil ke JSON untuk integrasi SIEM / dashboard pemantauan
python3 scripts/audit_agent.py --since 24h --json > audit_harian.json
```

---

## 📦 Engine Arsip & Ekspor Chat WhatsApp (Temporal Search & Import)

Karena batasan protokol resmi Meta yang hanya menyinkronkan pesan-pesan terkini ke perangkat pendamping (*linked device*), riwayat percakapan bertahun-tahun (1 s.d. 3+ tahun) dari grup kantor sering kali diekspor langsung dari ponsel berupa berkas `.zip` atau `.txt`.

Aina menyediakan engine impor berkinerja tinggi (*streaming regex* & SQLite FTS5) terintegrasi langsung di CLI biner untuk membedah ratusan ribu baris pesan dalam beberapa detik saja:

```bash
# 1. Impor berkas ekspor chat WhatsApp (.zip / .txt) ke workspace via Native CLI
aina archive import /path/ke/chat.zip --slug "ipds-6104"
# Atau lewati ekstraksi media:
aina archive import /path/ke/chat.txt --slug "tim-proyek" --no-media

# 2. Pencarian teks super cepat (<5ms) & Filter Temporal (Anti-Halusinasi Waktu)
# Cari pesan 7 hari terakhir:
aina archive search "reimbursement" --since 7d

# Cari pesan dalam kurun tanggal tertentu:
aina archive search "SOP pencacahan" --from 2026-09-01 --to 2026-09-10

# Output format JSON terstruktur untuk diproses agent:
aina archive search "anggaran" --json

# 3. Tampilkan statistik & anggota grup teraktif selama bertahun-tahun
aina archive stats --workspace default

# 4. Alternatif skrip Python (Legacy Fallback):
python3 scripts/chat_importer.py import /path/ke/chat.zip --workspace default --name "ipds-6104"
python3 scripts/chat_importer.py search "ipds-6104" --query "reimbursement"
python3 scripts/chat_importer.py links "ipds-6104" --domain "sheets"
```

---

## 🔒 Keamanan & Privasi Data (Security & Privacy)

Aina didesain dengan prinsip **Privacy-First & Zero Leakage** untuk lingkungan kerja dan organisasi:

1. **100% Self-Hosted & Local Storage**:
   - Seluruh basis data riwayat percakapan, ringkasan rapat, dan indeks pencarian tersimpan di database SQLite lokal di server Anda (`/app/data/aina.db`).
   - Tidak ada data percakapan yang dikirim ke server analitik atau telemetri pihak ketiga.
2. **Zero Privacy Leakage pada Companion Sensor**:
   - Pesan pribadi (DM) dari kontak keluarga, teman, atau pihak luar yang masuk ke nomor pribadi Anda (Sesi Companion) **diabaikan seketika** oleh `Gatekeeper` dan tidak pernah dimasukkan ke LLM maupun disimpan ke basis data.
   - Sesi Companion hanya membaca obrolan grup kerja sebagai sensor konteks pasif (*ambient recording*) atau merespons jika Anda secara eksplisit memanggilnya dengan awalan `!aina`.
3. **Kredensial & Token Terisolasi**:
   - Token OAuth Google Antigravity CLI disimpan pada volume penyimpanan persisten yang terisolasi (`/root/.gemini/`) dengan hak akses terbatas.
   - Web Setup Wizard dilindungi oleh kode acak sekali pakai (*one-time setup code*) dan otomatis terkunci permanen setelah token terverifikasi.
4. **Audit Trail Deterministik**:
   - Seluruh tindakan eksekusi terminal atau modifikasi berkas dicatat transparan di log sistem dan dapat diaudit sewaktu-waktu melalui perintah `aina audit`.

---

## ⚙️ Ringkasan Environment Variables

| Variabel | Default | Deskripsi |
| :--- | :--- | :--- |
| `SERVER_PORT` / `PORT` | `8090` | Port HTTP listening server Aina |
| `SERVER_HOST` | `0.0.0.0` | Host bind server |
| `WHATSMEOW_BASE_URL` | `http://localhost:3000` | Base URL REST API instance Whatsmeow |
| `WHATSMEOW_API_KEY` | `default-secret` | API Key autentikasi ke Whatsmeow Gateway |
| `WHATSMEOW_BOT_JID` | - | JID WhatsApp bot resmi (contoh: `62896xxxx@s.whatsapp.net`) |
| `WHATSMEOW_BOT_NAME` | `Aina` | Nama panggilan bot di obrolan WhatsApp |
| `WHATSMEOW_BOT_SESSION_ID` | `default` | ID sesi Whatsmeow gateway untuk bot utama |
| `WHATSMEOW_COMPANION_JID` | - | *(Opsional)* JID WhatsApp pribadi untuk Companion Sensor |
| `WHATSMEOW_COMPANION_NAME` | `Personal Account` | *(Opsional)* Label nama akun companion di dashboard & simulator |
| `WHATSMEOW_COMPANION_SESSION_ID` | `companion` | *(Opsional)* ID sesi Whatsmeow gateway untuk akun companion |
| `WHATSMEOW_SEND_ENDPOINT` | `/api/v1/messages/send-text` | Endpoint kirim pesan teks Whatsmeow (fallback: `/send/message`) |
| `WHATSMEOW_PRESENCE_ENDPOINT` | `/send/presence` | Endpoint penanda *typing presence* Whatsmeow |
| `AGENT_BINARY_PATH` | `agy` | Lokasi biner Antigravity CLI (otomatis mendeteksi PATH) |
| `AGENT_MODEL` | `gemini-3.8-flash-medium` | Model AI default bawaan |
| `AGENT_WORKSPACE` | `./workspaces/default` | Direktori kerja aktif bawaan agen (`/app/workspaces/default` di container) |
| `AGENT_TIMEOUT_SECONDS` | `300` | Batas waktu timeout eksekusi agent dalam detik |
| `DATABASE_PATH` | `data/aina.db` | Path berkas SQLite database (`/app/data/aina.db` di container) |
| `ADMIN_KEY` / `AINA_ADMIN_KEY` | *(Auto-generated)* | Kunci rahasia untuk membuka kunci Web Simulator & Wizard |
| `ADMIN_JID` / `AINA_ADMIN_JID` | - | Nomor WhatsApp pemilik/admin dengan hak wewenang penuh |
| `AINA_OAUTH_TOKEN` | - | *(Opsional)* Token OAuth Antigravity CLI mentah (JSON) untuk auto-injeksi di Coolify/Docker |
| `AINA_PERSONA_TEXT` | - | *(Opsional)* Teks persona kustom untuk meng-override isi `config/persona.md` |
| `AINA_ORGANIZATION_TEXT` | - | *(Opsional)* Konteks organisasi kustom untuk meng-override `config/organization.md` |
| `SCHEDULER_ENABLED` | `true` | Mengaktifkan pemindaian berkala in-process otomatis |
| `SCHEDULER_INTERVAL_SECONDS` | `60` | Interval pemeriksaan scheduler berkala dalam detik |
| `TZ` / `AINA_TIMEZONE` | `Asia/Jakarta` | Zona waktu operasional (WIB: UTC+7) |
| `AINA_LOCALE` | `id-ID` | Standar locale bahasa dan penanggalan |

---

## 🏛️ Arsitektur Kode (Clean Architecture)

```text
src/
├── core/                        # CORE BUSINESS LOGIC (Murni, Bebas Framework)
│   ├── domain/                  # Entitas & Domain Logic
│   │   ├── message.rs           # Pesan, Pengirim, SenderType, TargetType
│   │   ├── sensor.rs            # Multi-Session WhatsApp (PrimaryBot, UserCompanion, Shadow Sensor)
│   │   ├── gatekeeper.rs        # Evaluasi Mention, DM, & Aturan Privasi Kompanion
│   │   ├── persona.rs           # Persona Engine, Anti-Markdown-Table Formatting
│   │   ├── knowledge.rs         # Linter, Grooming, & GitHub Device Code Flow
│   │   ├── archive.rs           # SQLite FTS5 Full-Text Search BM25 Engine
│   │   └── audit.rs             # Audit Trail Jejak Eksekusi Antigravity
│   ├── ports/                   # Port Interfaces (Dependency Inversion)
│   │   ├── agent_engine.rs      # Trait AgentEnginePort (LLM Invocation & Models)
│   │   ├── whatsapp.rs          # Trait WhatsAppPort (Multi-Session Dispatch & Presence)
│   │   ├── session_store.rs     # Trait SessionStorePort (Chat History & Active Jobs)
│   │   └── ingestion.rs         # Trait KnowledgeIngestionPort & SourceRegistryPort
│   └── usecases/                # Orchestration Use Cases
│       ├── process_message.rs   # ProcessIncomingMessageUseCase (Dual-Session Pipeline)
│       └── scheduled_tick.rs    # ScheduledTickUseCase (Background Grooming & Auto-Healing)
├── adapters/
│   ├── driving/                 # Driving / Inbound Adapters
│   │   ├── webhook.rs           # Axum HTTP Server, Web Simulator, REST API (/api/*)
│   │   ├── scheduler.rs         # In-Process Background Scheduler Runner
│   │   └── cli.rs               # Unified Rust CLI Dispatcher (`aina [subcommand]`)
│   └── driven/                  # Driven / Outbound Adapters
│       ├── agy_cli.rs           # Google Antigravity CLI Adapter (`agy`)
│       ├── whatsmeow_http.rs    # Whatsmeow Multi-Session REST Client
│       └── sqlite_store.rs      # SQLite Persistence Adapter + FTS5 Search
├── config/                      # Pengaturan aplikasi & pemuat persona
└── main.rs                      # Composition Root & Dependency Injection
```

---

## ❓ Tanya Jawab & Troubleshooting (FAQ)

<details>
<summary><b>1. Aina tidak membalas pesan saya di grup WhatsApp. Mengapa?</b></summary>
Pastikan Anda memanggil atau me-mention Aina:
- Pada <b>Sesi Bot Utama</b>: Sebut nama <code>@Aina</code> atau tag nomor bot. Jika Anda hanya mengirim obrolan biasa tanpa mention, Aina bertindak sebagai <i>Gatekeeper</i> cerdas dan mencatatnya sebagai konteks pasif agar tidak mengganggu alur grup kerja.
- Pada <b>Sesi Companion (Nomor Pribadi)</b>: Awali pesan dengan <code>!aina</code> (contoh: <code>!aina tolong rangkum poin rapat di atas</code>) atau kirim pesan ke nomor sendiri (<i>Message Yourself</i>).
</details>

<details>
<summary><b>2. Bagaimana cara menemukan kode setup jika terminal log terlewat?</b></summary>
Jika Anda belum menyelesaikan wizard <code>/setup</code>, periksa log container dengan perintah:
<pre><code>docker compose logs aina | grep "SETUP / ADMIN CODE"</code></pre>
Atau jika menggunakan Coolify, periksa tab <b>Logs</b> pada aplikasi Aina Anda.
</details>

<details>
<summary><b>3. Muncul pesan status "Unauthenticated" pada Dashboard. Bagaimana solusinya?</b></summary>
Pastikan volume persistent <code>aina_gemini</code> terpasang di <code>/root/.gemini</code> pada Docker/Coolify. Buka <code>https://aina.domainkamu.com/setup</code> dan tempelkan token JSON dari <code>~/.gemini/antigravity-cli/antigravity-oauth-token</code> di laptop lokal Anda.
</details>

<details>
<summary><b>4. Apakah saya butuh kartu kredit atau API key LLM berbayar pihak ketiga?</b></summary>
Tidak. Aina memanfaatkan antarmuka langsung <b>Google Antigravity CLI (<code>agy</code>)</b> yang terhubung dengan akun Google pengembang Anda, sehingga model-model canggih seperti <b>Gemini 3.8 Flash</b> dan <b>Gemini 3.1 Pro</b> dapat langsung beroperasi.
</details>

<details>
<summary><b>5. Bagaimana jika Whatsmeow mengalami putus koneksi (disconnected)?</b></summary>
Cek status koneksi seketika melalui CLI server:
<pre><code>aina status</code></pre>
Jika salah satu sesi terputus, buka antarmuka gateway Whatsmeow Anda dan lakukan tautkan ulang (<i>re-link device</i>) dengan scan QR code baru.
</details>

---

## 📜 Lisensi & Panduan Kontribusi

Proyek ini adalah perangkat lunak sumber terbuka (*open source*) yang dilisensikan di bawah naungan [MIT License](LICENSE).

Kami sangat menyambut kontribusi dari komunitas! Baik berupa perbaikan bug, integrasi sensor data baru, maupun penyempurnaan persona. Silakan baca panduan kontribusi lengkap di **[CONTRIBUTING.md](CONTRIBUTING.md)** sebelum mengajukan *Pull Request*.

```bash
# Menjalankan pengujian regresi lokal sebelum membuat Pull Request:
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```
