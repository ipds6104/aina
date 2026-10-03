---
name: event-triage
description: >-
  Use this skill whenever processing incoming observability events, webhook alerts,
  CI/CD failure notifications, or production incidents received via `/api/v1/events` or `/api/events`.
  Guides autonomous root cause analysis, JIT secret retrieval via Infisical, log investigation,
  and safe remediation without leaking credentials.
---

# Observability Incident & Event Triage Skill

This skill equips Aina with formal operating procedures and best practices to triage, investigate, and remediate alerts, logs, and pipeline failures received via Aina's context-agnostic Event Ingress API (`/api/v1/events` / `/api/events`).

---

## 1. When to Activate This Skill (Trigger Conditions)

Activate this skill immediately whenever:
1. **Pesan Masuk Dimulai dengan Awalan Event**:
   Pesan masuk berisi format event: `🚨 [EVENT MASUK: ...]` atau `[OBSERVABILITY EVENT: ...]`.
2. **Pengguna Meminta Investigasi Insiden / Alert**:
   Pengguna di WhatsApp atau Web Simulator bertanya: *"Aina, tolong cek alert di repo billing-service"* atau *"Kenapa pipeline CI di repo auth gagal?"*.
3. **Webhook Monitoring / Observability Terpicu**:
   Alert masuk dari Sentry, Datadog, Prometheus Alertmanager, Grafana, ArgoCD, atau GitHub Actions.

---

## 2. Strict OpSec & Anti-Leakage Rules (Aturan Wajib)

> [!CAUTION]
> **ATURAN MUTLAK KEAMANAN KREDENSIAL PADA OBSERVABILITY**:
> 1. **DILARANG MENGULANG NILAI TOKEN/SECRET KE CHAT ATAU LOG!**
>    Saat melaporkan hasil investigasi ke grup atau pengguna, HANYA sebutkan statusnya (misal: *"Kredensial database berhasil diverifikasi"*). Jangan pernah menampilkan password, token, atau connection string mentah.
> 2. **GUNAKAN POLA JIT (JUST-IN-TIME) SECRET INJECTION**:
>    Selalu gunakan `secret_tool run -- ...` agar secret hanya disuntikkan ke proses anak saat dibutuhkan, tanpa pernah menulis secret ke file `.env` di disk.
> 3. **TAWAKKUF PADA TINDAKAN MERUSAK**:
>    Untuk tindakan *read-only* (melihat log, commit diff, stacktrace, status service), Aina boleh langsung menganalisis secara otonom. Namun untuk tindakan merusak (*drop table*, *hard reboot*, *force push*, *rollback deploy production*), Aina **WAJIB** meminta persetujuan eksplisit dari Admin terlebih dahulu.

---

## 3. Standard Operating Procedures (SOP Investigasi Insiden)

```text
┌──────────────┐     ┌──────────────┐     ┌──────────────┐     ┌──────────────┐     ┌──────────────┐
│  1. Parse &  │ ──> │ 2. Identify  │ ──> │ 3. JIT Vault │ ──> │ 4. Root Cause│ ──> │ 5. Report &  │
│  Triage Sev  │     │  Repo / Svc  │     │ Credentials  │     │   Analysis   │     │  Remediate   │
└──────────────┘     └──────────────┘     └──────────────┘     └──────────────┘     └──────────────┘
```

### Langkah 1: Parse & Triage Severity
1. Ekstrak data utama event:
   - `Source`: Sistem asal (Sentry, Prometheus, GitHub Actions, dll.).
   - `Severity`: `CRITICAL` (merah), `ERROR` (oranye), `WARNING` (kuning), `INFO` (biru).
   - `Service` & `Repository`: Nama mikroservis dan repositori GitHub yang terpengaruh.
   - `Details`: Pesan error, stacktrace, atau payload webhook.
2. Jalankan helper tool jika diperlukan:
   ```bash
   python3 skills/event-triage/scripts/event_triage_tool.py parse -m -j '<JSON_PAYLOAD>'
   ```

### Langkah 2: Identifikasi Repositori & Kode Terkait
1. Jika repositori telah dicantumkan di payload (misal: `my-org/billing-service`):
   - Periksa apakah repository sudah ada di workspace lokal atau dapat diakses via GitHub CLI (`gh`).
   - Periksa commit terakhir atau PR yang baru saja di-*merge* sebelum insiden terjadi:
     ```bash
     gh pr list --repo my-org/billing-service --state merged --limit 3
     # atau
     git log -n 5 --oneline
     ```

### Langkah 3: Ambil Kredensial via Infisical Secara Aman (Just-In-Time)
Jika investigasi memerlukan token API pihak ketiga (misal: API token Sentry, NewRelic, GitHub Token, atau read-only DB connection):
1. Periksa ketersediaan key di vault:
   ```bash
   python3 skills/event-triage/scripts/event_triage_tool.py check-secrets billing-service
   ```
2. Jalankan script investigasi dengan secret terinjeksi JIT:
   ```bash
   python3 skills/infisical/scripts/secret_tool.py run -- python3 scripts/check_logs.py
   ```

### Langkah 4: Root Cause Analysis (RCA)
Lakukan analisis tajam terhadap error:
- Apakah error disebabkan oleh *unhandled exception*, *null pointer*, *database pool timeout*, atau *bad deploy*?
- Hubungkan stacktrace dengan baris kode spesifik pada commit terakhir.

### Langkah 5: Laporkan Hasil & Rekomendasi Perbaikan
Kirimkan laporan ringkas dan elegan kepada tim/pengguna:
- **Status Insiden**: `[SEV-1 CRITICAL]` / `[SEV-2 ERROR]`
- **Layanan Terdampak**: `billing-service` (`org/billing-service`)
- **Akar Masalah (RCA)**: Penjelasan to-the-point mengenai penyebab error.
- **Rekomendasi / Solusi**: Tindakan mitigasi yang disarankan atau draf pull request perbaikan.
- **Tawaran Bantuan**: *"Apakah Abang/Kakak ingin Aina buatkan Pull Request perbaikan untuk masalah ini?"*

### Prosedur Tambahan: Membantu Pengguna Menyiapkan Webhook Observability (Setup via Chat)
Jika pengguna di chat bertanya *"Aina, bagaimana cara menyambungkan alert Sentry / GitHub Actions / Prometheus ke kamu?"*:
1. Jalankan perintah helper untuk mendapatkan payload & URL siap pakai:
   ```bash
   python3 skills/event-triage/scripts/event_triage_tool.py webhook-info --service "<NAMA_SERVICE>" -m
   ```
2. Kirimkan ringkasan Markdown kepada pengguna berisi Webhook URL resmi, contoh perintah `curl` pengujian, dan petunjuk ringkas pemasangan di dashboard monitoring terkait.

---

## 4. Helper Tool Command Reference

| Perintah | Deskripsi |
| :--- | :--- |
| `python3 skills/event-triage/scripts/event_triage_tool.py parse -j '<JSON>'` | Melakukan parsing dan triase otomatis payload event ke format JSON terstruktur. |
| `python3 skills/event-triage/scripts/event_triage_tool.py parse -m -j '<JSON>'` | Menghasilkan ringkasan laporan insiden dalam format Markdown siap baca. |
| `python3 skills/event-triage/scripts/event_triage_tool.py check-secrets <SVC>` | Memeriksa apakah token/kredensial untuk service terkait tersedia di brankas Infisical/Vault. |
| `python3 skills/event-triage/scripts/event_triage_tool.py webhook-info [--service <SVC>] [-m]` | Menghasilkan URL webhook dan resep integrasi siap pakai untuk Sentry, Prometheus, dan CI/CD. |

---

## 5. Hubungan dengan Skill Lain

- **`skills/infisical`**: Digunakan untuk mengambil atau mengeksekusi script dengan kredensial terinjeksi JIT tanpa pernah mengekspos token.
- **`skills/tabayyun`**: Digunakan untuk menahan diri dan meminta konfirmasi jika tindakan remedi berisiko tinggi.
- **`skills/knowledge-curator`**: Digunakan untuk mencatat insiden penting ke basis pengetahuan (`knowledge/operations/incidents.md`) agar menjadi pelajaran bagi sistem di masa depan.
