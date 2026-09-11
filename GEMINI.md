# Aina (あいな) - Codebase Charter & Engineering Guidelines

> [!NOTE]
> File ini adalah panduan kustomisasi kanonik Antigravity (`GEMINI.md`) untuk repositori **`aina`**. Semua pengembang dan AI assistant wajib mematuhi batasan arsitektur dan konvensi di bawah ini.

---

## 1. Identitas Sistem & Arsitektur Utama

**Aina** adalah persona asisten AI *self-hosted* yang terintegrasi ke WhatsApp (melalui gateway Whatsmeow) dan ditenagai langsung oleh mesin agentik **Google Antigravity CLI (`agy`)**.

Aplikasi dibangun menggunakan bahasa **Rust** dengan arsitektur **Hexagonal (Ports & Adapters)** murni:

```text
src/
├── core/                        # DOMAIN & BUSINESS LOGIC (Murni tanpa dependensi luar)
│   ├── domain/                  # Entitas Pesan, Gatekeeper, Persona & Temporal Engine
│   ├── ports/                   # Traits / Interfaces abstrak (AgentEngine, WhatsApp, SessionStore)
│   └── usecases/                # Orkestrasi alur pesan dan cron scheduler
├── adapters/
│   ├── driving/                 # PRIMARY ADAPTERS (Axum HTTP Webhook & Web Simulator)
│   └── driven/                  # SECONDARY ADAPTERS (Antigravity CLI runner, Whatsmeow client, SQLite)
├── config/                      # Pengaturan aplikasi & pemuat persona
└── workspace/                   # Sandbox runtime tempat agen mengeksekusi kode
```

---

## 2. Batasan Arsitektur Mutlak (Hard Constraints)

1. **Isolasi Domain (`src/core/domain/`)**:
   - Dilarang keras mengimpor framework atau I/O library eksternal (seperti `axum`, `reqwest`, `tokio::process`, atau `rusqlite`) ke dalam modul `core/domain`.
   - Modul domain hanya boleh berisi logika bisnis murni, kalkulasi waktu sipil (*civil time*), dan aturan evaluasi pesan.
2. **Pola Pembalikan Ketergantungan (*Dependency Inversion*)**:
   - Seluruh interaksi ke sistem luar (WhatsApp, CLI `agy`, database) **wajib melalui Trait** di `src/core/ports/`.
   - Usecase tidak boleh bergantung langsung pada struct adapter konkrit, melainkan melalui `Arc<dyn PortTrait>`.
3. **Pemisahan Peran: Skills vs Rules vs Persona**:
   - **`GEMINI.md` (Rules)**: Menetapkan batasan arsitektur, kebijakan keamanan (OpSec), dan aturan baku yang selalu aktif.
   - **`skills/<name>/SKILL.md` (Skills)**: Menampung prosedur operasional multi-langkah dan pemanggilan alat eksternal (misal: REST API Whatsmeow) dengan prinsip *Progressive Disclosure* untuk menghemat token.
   - **`config/persona.md` (Persona)**: Menentukan gaya bahasa, karakter rekan kerja, dan keramahan komunikasi.

---

## 3. Standar Waktu & Lokalisasi (WIB / UTC+7)

1. **Zona Waktu Resmi**: Seluruh penanggalan sistem, logging, dan temporal awareness AI menggunakan **WIB (`Asia/Jakarta`, UTC+7)** dengan locale **`id-ID`**.
2. **Kalkulasi Waktu Murni**: Format penanggalan lokal dihitung secara deterministik menggunakan algoritma waktu sipil tanpa menambah dependensi crate eksternal.
3. **Penyelarasan Kontainer**: Dockerfile dan runtime environment wajib mempertahankan `ENV TZ=Asia/Jakarta`.

---

## 4. Etika WhatsApp & Kesadaran Platform (*Platform Awareness*)

1. **Gatekeeper di Grup WhatsApp**:
   - Pesan pribadi (DM) selalu dijawab secara personal.
   - Pesan grup hanya dijawab jika di-mention (`@Aina`), di-quote/reply, atau namanya dipanggil jelas. Pesan lainnya berstatus `RecordOnly` (menyimak pasif tanpa spam).
2. **Format Pesan Adaptif**:
   - **Di WhatsApp**: Gunakan formatting teks native WhatsApp: `*tebal*`, `_miring_`, `~coret~`, dan `monospace`.
     - ❌ **DILARANG** menggunakan `# Heading` Markdown (hanya muncul sebagai tanda pagar).
     - ❌ **DILARANG** menggunakan tabel Markdown `| a | b |` (hancur di layar HP, ganti dengan bullet points `•`).
     - ❌ **DILARANG** menggunakan hyperlink `[teks](url)` (tulis URL langsung).
     - Jika menghasilkan kode panjang (>25 baris), simpan ke file di `workspace/` dan sajikan ringkasannya di chat.
   - **Di Web Simulator**: Gunakan Full Rich GitHub Flavored Markdown (headings, callout alerts, tables, syntax-highlighted code blocks).

---

## 5. Standar Rekayasa Kode Rust

- **Error Handling**: Gunakan `anyhow::Result` untuk layer usecases/adapters dan `thiserror` jika membutuhkan domain error terstruktur. Jangan pernah menggunakan `.unwrap()` pada jalur kode produksi yang menangani I/O jaringan atau input pengguna.
- **Asinkron**: Gunakan runtime `tokio`. Hindari blocking call (`std::thread::sleep` atau I/O sinkron); gunakan `tokio::time::sleep` dan `tokio::fs`.
- **Verifikasi Sebelum Commit**:
  - Pastikan syntax check valid: `cargo check`.
  - Pastikan seluruh pengujian unit lulus: `cargo test`.
  - Pertahankan kebersihan repositori tanpa cache atau file build yang tidak terlacak.

---

## 6. Protokol Deployment Coolify & Persistensi

- **Port Standar**: Port HTTP server adalah `8090` (mendukung override via `PORT` atau `SERVER_PORT`).
- **Persistent Volumes Wajib**:
  1. `/app/data` ➔ Database SQLite (`aina.db`).
  2. `/root/.gemini` ➔ Token autentikasi OAuth Antigravity CLI.
  3. `/app/workspace` ➔ Sandbox direktori kerja agen.
- **Izin Eksekusi Skrip**: Skrip pembantu di dalam `skills/*/scripts/` wajib memiliki izin `chmod +x`, yang otomatis diamankan oleh `docker-entrypoint.sh`.
