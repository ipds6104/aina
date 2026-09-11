# 🌸 Aina (あいな) - Self-Hosted Agentic AI Co-Worker

[![Rust](https://img.shields.io/badge/Language-Rust_2021-orange.svg)](https://www.rust-lang.org/)
[![Engine](https://img.shields.io/badge/Agent_Engine-Google_Antigravity_CLI_(agy)-blue.svg)](https://antigravity.google)
[![Gateway](https://img.shields.io/badge/WhatsApp-Whatsmeow_HTTP-25D366.svg)](https://github.com/tulir/whatsmeow)
[![Architecture](https://img.shields.io/badge/Architecture-Hexagonal_/_Clean_Architecture-9cf.svg)]()
[![License](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)

**Aina** adalah persona asisten AI *self-hosted* yang terintegrasi secara langsung ke WhatsApp (melalui gateway Whatsmeow) dan ditenagai oleh mesin agentik **Google Antigravity CLI (`agy`)**.

Aina dirancang bukan sebagai bot CS yang kaku, melainkan sebagai **rekan kerja teknis (software engineer / staf data)** di grup WhatsApp maupun percakapan pribadi:
- ⚡ **Cekatan & Solutif**: Memberikan solusi konkret, siap pakai, dan mampu mengeksekusi kode secara nyata di terminal.
- 💬 **Basa-Basi Seperlunya**: *Low-noise*, to-the-point, santun, dan bersahabat.
- 🧭 **Proactive Clarification**: Bertanya dan meminta klarifikasi terarah jika instruksi multitafsir sebelum mengambil tindakan.
- 🛡️ **Gatekeeper Cerdas**: Tidak *spamming* di grup kantor (hanya menjawab jika di-tag/disebut, dan mencatat percakapan pasif sebagai konteks).
- 🧠 **Dynamic Model Switching**: Bawaan cepat & cerdas dengan **Google Gemini** (5–15 detik), dengan opsi eskalasi ke **Claude Opus** khusus tugas kompleks.
- 📁 **Workspace-Agnostic & Structured Knowledge Base**: Basis pengetahuan tumbuh secara organik per proyek/instansi tanpa tercampur baur.

---

## 🏗️ Alur Konteks ke Knowledge Base (Pipeline Architecture)

Aina memproses percakapan WhatsApp menjadi basis pengetahuan terstruktur yang rapi, terisolasi, dan mudah dicari (*high retrievability*):

```text
[WhatsApp DM / Grup / Web Simulator]
                 │
                 ▼
     [1. Gatekeeper & Epistemic Filter] ──(Bukan Tag)──> [Simpan Pasif di SQLite]
                 │ (Di-mention / Pesan Pribadi)
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
 [4. Knowledge Grooming Engine (scripts/workspace_manager.py groom)]
      └── Pembaruan Indeks Otomatis (knowledge/index.md) -> Progressive Retrieval
```

---

## 🚀 Panduan Memulai Cepat (Getting Started in 5 Minutes)

Repositori ini sepenuhnya **Agnostic & Clone-Ready**. Anda dapat menjalankannya di Coolify, VPS Docker, maupun komputer lokal.

### Opsi 1: Deploy di Coolify (Paling Direkomendasikan)

1. **Buat Resource Baru di Coolify**:
   - Pilih **Projects** -> Pilih Environment -> Klik **+ New Resource** -> **Application**.
   - Pilih **GitHub App** -> Pilih repositori **`aina`** Anda -> Branch `main`.
   - Build Pack: Pilih **Dockerfile** (otomatis mendeteksi [`Dockerfile`](Dockerfile)).
2. **Atur Environment Variables** (Buka tab *Environment Variables* di Coolify):
   ```ini
   PORT=8090
   SERVER_PORT=8090
   WHATSMEOW_BASE_URL=https://wa.domainkamu.com
   WHATSMEOW_API_KEY=secret_key_kamu
   WHATSMEOW_BOT_JID=628xxxxxxxxxx@s.whatsapp.net
   WHATSMEOW_BOT_NAME=Aina
   AGENT_MODEL=gemini-3.8-flash-medium
   AGENT_WORKSPACE=/app/workspaces/default
   DATABASE_PATH=/app/data/aina.db
   ```
3. **Atur Persistent Storage (Volumes)**:
   Di tab **Storages**, tambahkan 3 persistent storage agar data tidak hilang saat re-deploy:
   | Volume Name | Destination Path | Keterangan |
   | :--- | :--- | :--- |
   | `aina_data` | `/app/data` | Database SQLite (`aina.db`) & mapping percakapan |
   | `aina_gemini` | `/root/.gemini` | Kredensial OAuth Antigravity & cache CLI |
   | `aina_workspaces`| `/app/workspaces` | Wadah seluruh workspace & knowledge base |
4. **Klik Deploy**: Coolify akan mengompilasi dan menjalankan Aina secara otomatis.

---

### Opsi 2: Menggunakan Docker Compose (VPS / Server Mandiri)

1. **Klon Repositori**:
   ```bash
   git clone https://github.com/ipds6104/aina.git
   cd aina
   ```
2. **Salin Template Konfigurasi**:
   ```bash
   cp .env.example .env
   # Edit .env dan sesuaikan URL whatsmeow serta BOT_JID Anda
   ```
3. **Jalankan Aplikasi**:
   ```bash
   docker compose up -d
   ```
4. Periksa log server untuk melihat status dan **Setup Code**:
   ```bash
   docker compose logs -f aina
   ```

---

### Opsi 3: Menjalankan di Komputer Lokal (Local Development)

```bash
# 1. Pastikan Rust dan Antigravity CLI (agy) terpasang
curl -fsSL https://antigravity.google/cli/install.sh | bash

# 2. Jalankan unit test
cargo test

# 3. Jalankan server lokal
cargo run
```
Akses dashboard di browser: `http://localhost:8090`.

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

---

## 🧪 Web Simulator (Real End-to-End Testing)

Di dashboard web (`https://aina.domainkamu.com`), tersedia kartu **Simulator Percakapan WhatsApp (Real Test)**:
- **Zero Mocks**: Pesan diuji melalui logika *Gatekeeper*, *Persona*, dan dieksekusi secara nyata oleh biner `agy`.
- **Pengujian Multi-Skenario**: Uji pesan pribadi (DM), obrolan grup dengan mention `@Aina`, maupun obrolan grup tanpa tag.
- **Pilihan Model AI Dinamis**: Pilih model per pengujian langsung dari dropdown simulator.
- **Salin Jawaban (📋)**: Terdapat tombol salin lengkap untuk memindahkan jawaban Aina dengan format rapi.

---

## 🤖 Manajemen Model AI (Gemini-First Priority)

Aina mengutamakan efisiensi dan kecepatan respons dengan memprioritaskan keluarga model **Google Gemini** sebagai *default*:

| Model | Karakteristik | Estimasi Latensi | Penggunaan Terbaik |
| :--- | :--- | :--- | :--- |
| **`gemini-3.8-flash-medium`** | **Default / Rekomendasi** | **5 – 15 detik** | Cepat, seimbang, pemikiran mendalam, bebas timeout. |
| **`gemini-3.8-flash-high`** | Penalaran Tinggi | 15 – 35 detik | Arsitektur kompleks, refaktor besar, analisis data rumit. |
| **`gemini-3.8-flash-low`** | Respons Kilat | < 3 detik | Sapaan santai, konfirmasi cepat, obrolan kasual. |
| **`gemini-3.1-pro-high`** | Deep Coding | 20 – 45 detik | Debugging kode tingkat lanjut, penulisan script besar. |
| **`claude-opus-4-6-thinking`** | **Khusus Eksplisit** | 60 – 120 detik | Hanya aktif jika user meminta: *"Aina, pakai model opus"*. |

### Cara Mengganti Model AI:
1. **Via Chat WhatsApp / Simulator**:
   - Cek model aktif: `/model status` atau `/model list`
   - Ganti model instan: `/model <nama_model>` (contoh: `/model gemini-3.8-flash-high` atau `/model opus`)
2. **Via Web Dashboard**:
   - Klik **⚡ Ganti** pada info *Model AI Aktif* di dashboard.
3. **Via Skrip Terminal / Aina Otonom**:
   ```bash
   python3 scripts/model_control.py get
   python3 scripts/model_control.py set gemini-3.8-flash-high
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

### Perintah Manajemen Workspace, Kegiatan, & Jadwal:
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

## ⚙️ Ringkasan Environment Variables

| Variabel | Default | Deskripsi |
| :--- | :--- | :--- |
| `SERVER_PORT` / `PORT` | `8090` | Port HTTP listening server Aina |
| `SERVER_HOST` | `0.0.0.0` | Host bind server |
| `WHATSMEOW_BASE_URL` | `http://localhost:3000`| Base URL REST API instance Whatsmeow |
| `WHATSMEOW_API_KEY` | `default-secret` | API Key autentikasi ke Whatsmeow |
| `WHATSMEOW_BOT_JID` | - | JID WhatsApp bot (contoh: `628xxxx@s.whatsapp.net`) |
| `WHATSMEOW_BOT_NAME` | `Aina` | Nama panggilan bot di obrolan |
| `AGENT_BINARY_PATH` | `agy` | Lokasi biner Antigravity CLI (otomatis mendeteksi PATH) |
| `AGENT_MODEL` | `gemini-3.8-flash-medium` | Model AI default bawaan |
| `AGENT_WORKSPACE` | `./workspaces/default` | Direktori kerja aktif bawaan agen |
| `DATABASE_PATH` | `data/aina.db` | Path berkas SQLite database |
| `ADMIN_KEY` / `AINA_ADMIN_KEY` | *(Auto-generated)* | Kunci rahasia untuk membuka kunci Web Simulator & Wizard |
| `ADMIN_JID` | - | Nomor WhatsApp pemilik/admin berwewenang penuh |
| `TZ` / `AINA_TIMEZONE` | `Asia/Jakarta` | Zona waktu operasional (WIB: UTC+7) |
| `AINA_LOCALE` | `id-ID` | Standar locale bahasa |

---

## 🏛️ Arsitektur Kode (Clean Architecture)

```text
src/
├── core/                        # CORE BUSINESS LOGIC (Murni, Bebas Framework)
│   ├── domain/                  # Entitas Pesan, Pengirim, Gatekeeper, Persona
│   ├── ports/                   # Trait Interface (AgentEnginePort, WhatsAppPort, SessionStorePort)
│   └── usecases/                # ProcessIncomingMessageUseCase, ScheduledTickUseCase
├── adapters/
│   ├── driving/                 # Webhook Server (Axum HTTP), Web Simulator, Scheduler
│   └── driven/                  # Antigravity CLI Adapter, Whatsmeow Client, SQLite Store
├── config/                      # Pengaturan aplikasi & pemuat persona
└── main.rs                      # Composition Root & Dependency Injection
```

---

## 📜 Lisensi & Kontribusi

Dilisensikan di bawah lisensi [MIT](LICENSE). Kontribusi, perbaikan bug, dan *feature requests* sangat dipersilakan melalui *Pull Request* atau *Issue* di GitHub.
