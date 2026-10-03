---
name: self-observability
description: >-
  Use this skill whenever the user asks about Aina's tool usage frequency, previous actions,
  telemetry data, performance bottlenecks, suggestions for faster shortcuts/tools, or deciding
  whether an enhancement should be built-in to the Rust core or implemented as a custom skill.
---

# Autonomous Self-Observability & Optimization Skill for Aina

Skill ini membekali Aina dengan kemampuan metakognisi empiris untuk mengaudit jejak pemanggilan alat (tools), mendeteksi bottleneck efisiensi, dan memberikan rekomendasi optimasi arsitektural (apakah perbaikan harus **Built-in** ke core Rust atau **Custom Skill** fleksibel).

---

## 1. Sumber Data Telemetri Aina (Empirical Telemetry Sources)

Aina memiliki 3 lapisan sumber data yang dapat diakses langsung:

1. **SQLite Database Action Audits (`whatsapp_action_audits`)**:
   - Lokasi: `$DATABASE_PATH` (default: `/root/projects/aina/data/aina.db`).
   - Menyimpan metrik level interaksi chat: `sender_jid`, `chat_jid`, `tools_invoked` (JSON array), `duration_seconds`, `status` (success/failed), `usecase`, dan pesan pengguna.
2. **Antigravity Brain Transcripts (`transcript.jsonl`)**:
   - Lokasi: `/root/.gemini/antigravity-cli/brain/*/logs/transcript.jsonl`.
   - Menyimpan jejak eksekusi sub-langkah presisi: nama tool riil (`run_command`, `view_file`, `replace_file_content`), parameter detail, output, dan stempel waktu.
3. **Metacognitive Registry & CLI Observability**:
   - Perintah `aina audit summary [--json]`, `aina audit diag`, dan `aina metacog capabilities`.

---

## 2. Standard Operating Procedures (SOP)

### SOP 1: Mengambil Ringkasan Telemetri & Frekuensi Tool
Jalankan script telemetri bawaan:
```bash
python3 skills/self-observability/scripts/analyze_telemetry.py summary
# Atau untuk format JSON:
python3 skills/self-observability/scripts/analyze_telemetry.py summary --json
```

### SOP 2: Menganalisis Detail Tool Usage & Pola Perintah Berulang
Untuk melihat peringkat pemanggilan tool dan command prefix yang sering dieksekusi:
```bash
python3 skills/self-observability/scripts/analyze_telemetry.py tools
```

### SOP 3: Mendeteksi Bottleneck & Pola Pengulangan (Hotspots)
Untuk melihat file yang paling sering dibaca berulang atau perintah lambat:
```bash
python3 skills/self-observability/scripts/analyze_telemetry.py bottlenecks
```

### SOP 4: Menghasilkan Rekomendasi Optimasi Arsitektur
Jalankan:
```bash
python3 skills/self-observability/scripts/analyze_telemetry.py recommendations
```

---

## 3. Matriks Keputusan: Built-in (Rust Core) vs Custom Skill

Saat merumuskan perbaikan, Aina **WAJIB** mengevaluasi berdasarkan kriteria berikut:

| Parameter Evaluasi | Pilih **Built-in (Rust Core Daemon)** | Pilih **Custom Skill (`skills/` / Git)** |
| :--- | :--- | :--- |
| **Karakteristik Latensi** | Critical Path / Latensi mikrodetik (<10ms, non-blocking I/O) | Toleransi latensi standar (50ms - 2s via subprocess/API) |
| **Domain Beban Kerja** | Socket protokol (Whatsmeow), routing HTTP API, indexing FTS5, crypto/auth, in-memory cache | Integrasi SaaS/API pihak ketiga (GitHub, Sentry, Notion, Supabase, Infisical) |
| **Siklus Pembaruan** | Stabil, jarang berubah, memerlukan type-safety compile-time Rust | Dinamis, sering iterasi, domain logic spesifik user |
| **Blast Radius (Kegagalan)**| Risiko tinggi: Crash pada Rust daemon menghentikan bot | Terisolasi: Script subprocess gagal tidak merusak daemon |
| **Mekanisme Deployment** | Harus compile binary native & deploy container baru | Cukup hot-reload folder skill / sync repo GitHub (`USER_SKILLS_REPO`) |

---

## 4. Remediasi dan Eksekusi Perbaikan Otomatis

1. **Jika Solusi adalah Custom Skill**:
   - Aina langsung menawarkan atau mengeksekusi pembuatan skill via `skill-builder`:
     ```bash
     python3 skills/skill-builder/scripts/skill_scaffolder.py init <nama_skill>
     ```
   - Buat script otomasi ringkas (misal 1 perintah CLI gabungan daripada memanggil 5 tool berturut-turut).
   - Validasi manifest: `python3 skills/skill-builder/scripts/skill_scaffolder.py validate <nama_skill>`.
   - Push ke `USER_SKILLS_REPO` jika dikonfigurasi.

2. **Jika Solusi adalah Built-in Core**:
   - Rancang arsitektur bersih: Domain Port (`src/core/ports/`), Domain Logic (`src/core/domain/`), dan Adapter (`src/adapters/`).
   - Tambahkan unit test deterministik (`cargo test`).
   - Buat commit Git dan laporkan diff kepada pengguna.
