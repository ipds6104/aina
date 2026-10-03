# 🚨 Aina Event Ingress & Observability Extensibility Guide

Panduan resmi arsitektur, spesifikasi teknis, dan prosedur operasional untuk mengintegrasikan sistem **observability**, **CI/CD pipelines**, **monitoring alerts**, dan **brankas rahasia terpusat (Infisical / Vault)** ke dalam Aina sebagai asisten otonom multi-repositori.

---

## 📑 Daftar Isi
1. [Arsitektur & Prinsip Desain (Zero Bloat Guarantee)](#1-arsitektur--prinsip-desain-zero-bloat-guarantee)
2. [Spesifikasi HTTP Ingress API (`/api/v1/events`)](#2-spesifikasi-http-ingress-api-apiv1events)
3. [Model Keamanan & Inbound API Key](#3-model-keamanan--inbound-api-key)
4. [Cookbook Integrasi Webhook (Sentry, Prometheus, GitHub Actions)](#4-cookbook-integrasi-webhook)
5. [Tata Kelola Rahasia Terpusat: Infisical Universal Auth & JIT Injection](#5-tata-kelola-rahasia-terpusat-infisical-universal-auth--jit-injection)
6. [Ekstensibilitas Dinamis Multi-Repo via Git (`USER_SKILLS_REPO`)](#6-ekstensibilitas-dinamis-multi-repo-via-git-user_skills_repo)
7. [Skill Otonom Aina: `skills/event-triage`](#7-skill-otonom-aina-skillsevent-triage)
8. [Siklus Hidup Eksekusi & Audit Trail](#8-siklus-hidup-eksekusi--audit-trail)

---

## 1. Arsitektur & Prinsip Desain (Zero Bloat Guarantee)

Aina dirancang untuk dapat mengawasi puluhan hingga ratusan repositori produksi sekaligus. Untuk menjaga performa tinggi dan portabilitas container, Aina memisahkan secara tegas antara **Inbound Context Ingress** dan **Southbound Operations / Secrets**:

```
┌────────────────────────────────────────────────────────────────────────────────────────┐
│ EXTERNAL MONITORING & CI/CD                                                            │
│ (Sentry, Prometheus Alertmanager, Datadog, GitHub Actions, ArgoCD, Grafana)            │
└───────────────────────────────────────────┬────────────────────────────────────────────┘
                                            │ HTTP POST /api/v1/events (API Key Protected)
                                            ▼
┌────────────────────────────────────────────────────────────────────────────────────────┐
│ AINA RUST CORE ENGINE (Hexagonal Architecture)                                         │
│ • Driving Adapter: Inbound Controller (`src/adapters/driving/web/controllers/events.rs`)│
│ • Response Cepat: Mengembalikan HTTP 202 Accepted (<5ms)                               │
│ • Non-Blocking Queue: Memasukkan pesan ke `dispatch_message_to_queue`                  │
│ • Zero External Bloat: Core daemon tidak memuat dependensi SDK vault pihak ketiga      │
└───────────────────────────────────────────┬────────────────────────────────────────────┘
                                            │ Dispatch ke Agent Engine
                                            ▼
┌────────────────────────────────────────────────────────────────────────────────────────┐
│ EXTENSIBLE EXECUTION LAYER (Sandboxed Skills & JIT Secrets)                           │
│ • Skill Triage: `skills/event-triage` membaca rincian error & menentukan severity      │
│ • JIT Secret Injection: `secret_tool run -- ...` menginjeksi token vault ke sub-proses │
│ • Multi-Repo Custom Skills: Di-sync via `USER_SKILLS_REPO` dari Git pribadi            │
│ • Output & Notifikasi: Temuan RCA dikirim ke WhatsApp / Web Dashboard / GitHub PR      │
└────────────────────────────────────────────────────────────────────────────────────────┘
```

### Mengapa SDK Vault Tidak Ditanam Langsung di Rust?
1. **Zero Binary Bloat**: Binary Rust `aina` tetap ringan, cepat di-compile, dan bebas dependensi C/FFI eksternal.
2. **Vendor-Agnostic & Zero Rebuild**: Jika infrastruktur berpindah dari Infisical ke HashiCorp Vault, AWS Secrets Manager, atau Doppler, operator cukup mengganti script CLI wrapper di level skill tanpa perlu recompile binary atau merilis container baru.
3. **Least Privilege & Attack Surface Rendah**: Daemon utama tidak memegang token rahasia secara permanen di memori. Kredensial hanya disuntikkan ke proses anak (*sub-process*) tepat saat investigasi berlangsung (*Just-In-Time*).

---

## 2. Spesifikasi HTTP Ingress API (`/api/v1/events`)

Aina menyediakan endpoint HTTP POST context-agnostic:
- **Rute Utama**: `POST /api/v1/events`
- **Rute Alias**: `POST /api/events`

### JSON Payload Schema

Endpoint menerima payload JSON terstruktur maupun payload bebas (*untyped JSON*):

| Field | Tipe | Wajib? | Deskripsi |
| :--- | :--- | :--- | :--- |
| `source` | `string` | Opsional | Sistem sumber event (misal: `"sentry"`, `"prometheus"`, `"github-actions"`, `"datadog"`). Default: `"observability"`. |
| `event` | `string` | Opsional | Kategori kejadian (misal: `"alert"`, `"error_spike"`, `"pipeline_failed"`, `"deployment"`). Default: `"alert"`. |
| `severity` | `string` | Opsional | Tingkat keparahan (`"critical"`, `"error"`, `"warning"`, `"info"`). Default: `"info"`. |
| `service` | `string` | Opsional | Nama layanan/mikroservis (misal: `"billing-service"`, `"auth-api"`). |
| `repository` | `string` | Opsional | Repositori kode terkait (misal: `"my-org/billing-service"`). |
| `title` | `string` | Opsional | Judul ringkas kejadian atau headline alert. |
| `details` | `string \| object` | Opsional | Rincian error, stacktrace, log body, atau markdown deskripsi. |
| `metadata` | `object` | Opsional | Label tambahan, region, host, kubernetes pod, commit SHA. |
| `target_chat` | `string` | Opsional | JID tujuan notifikasi (misal: grup WhatsApp `1203630xxx@g.us` atau nomor user). Default ke Companion JID / Bot JID. |
| `suggested_actions` | `array[string]` | Opsional | Daftar saran investigasi atau mitigasi awal. |

### Contoh Request

```bash
curl -X POST http://localhost:8080/api/v1/events \
  -H "Content-Type: application/json" \
  -H "X-API-Key: YOUR_EVENTS_API_KEY" \
  -d '{
    "source": "sentry",
    "event": "alert",
    "severity": "critical",
    "service": "billing-service",
    "repository": "my-org/billing-service",
    "title": "Payment gateway timeout spike > 15%",
    "details": "Connection pool exhausted on postgres-replica-02. Stacktrace: DBConnectionTimeout at repo.checkout:42",
    "metadata": { "env": "production", "region": "ap-southeast-1" },
    "suggested_actions": ["check_active_connections", "restart_replica"]
  }'
```

### Contoh Response (HTTP 202 Accepted)

Respon diberikan secara instan (<5ms) karena pemrosesan diserahkan ke antrean otonom:

```json
{
  "status": "accepted",
  "event_id": "evt_1727999842_412",
  "message": "Event berhasil diterima dan dimasukkan ke antrean proses Aina.",
  "source": "sentry",
  "severity": "critical",
  "service": "billing-service",
  "target_chat": "628999888777@s.whatsapp.net"
}
```

---

## 3. Model Keamanan & Inbound API Key

Untuk mencegah penyalahgunaan endpoint ingress dari pihak luar, Aina menerapkan verifikasi autentikasi bertingkat:

### Hierarki API Key
1. **`AINA_EVENTS_API_KEY` (atau `EVENTS_API_KEY`)**: Kunci khusus untuk sistem observability.
2. **Fallback Otomatis**: Jika kunci khusus di atas tidak didefinisikan di `.env` / environment container, sistem otomatis memverifikasi kecocokan terhadap `AINA_ADMIN_KEY` / `SETUP_CODE` atau `WHATSMEOW_API_KEY`.

### Metode Pengiriman Autentikasi yang Didukung
Klien atau webhook penyedia monitoring dapat mengirimkan token melalui salah satu metode berikut:
- **Header `X-API-Key`**: `X-API-Key: YOUR_SECRET_KEY`
- **Header `X-Events-Key`**: `X-Events-Key: YOUR_SECRET_KEY`
- **Header `Authorization`**: `Authorization: Bearer YOUR_SECRET_KEY`
- **URL Query Parameter**: `?api_key=YOUR_SECRET_KEY` atau `?token=YOUR_SECRET_KEY` *(sangat berguna untuk webhook eksternal yang tidak mendukung custom header)*.

Jika tidak valid, Aina mengembalikan HTTP `401 Unauthorized`:
```json
{
  "status": "error",
  "error": "Akses ditolak. API Key untuk endpoint events tidak valid atau belum dikirimkan."
}
```

---

## 4. Cookbook Integrasi Webhook

### Sentry Alert Rule Webhook
Di dashboard Sentry (**Alerts** -> **Create Alert Rule** -> **Actions** -> **Send a Webhook**):
- **URL**: `https://aina.domainkamu.com/api/v1/events?api_key=YOUR_EVENTS_API_KEY`
- **Payload**: Sentry secara otomatis mengirimkan payload JSON insiden. Parser Aina otomatis mengekstrak `project`, `message`, `level`, dan stacktrace.

### Prometheus Alertmanager (`alertmanager.yml`)
Tambahkan webhook receiver di Alertmanager:
```yaml
receivers:
  - name: 'aina-events'
    webhook_configs:
      - url: 'http://aina:8080/api/v1/events'
        send_resolved: true
        http_config:
          authorization:
            credentials: 'YOUR_EVENTS_API_KEY'
```

### GitHub Actions CI/CD Failure Alert
Tambahkan langkah notifikasi di akhir workflow:
```yaml
- name: Notify Aina on Failure
  if: failure()
  run: |
    curl -s -X POST "https://aina.domainkamu.com/api/v1/events" \
      -H "Content-Type: application/json" \
      -H "X-API-Key: ${{ secrets.AINA_EVENTS_API_KEY }}" \
      -d '{
        "source": "github-actions",
        "event": "pipeline_failed",
        "severity": "error",
        "service": "${{ github.event.repository.name }}",
        "repository": "${{ github.repository }}",
        "title": "CI Run Failed on '${{ github.ref_name }}'",
        "details": "Workflow '${{ github.workflow }}' run #${{ github.run_number }} failed on commit ${{ github.sha }}.",
        "metadata": { "run_url": "${{ github.server_url }}/${{ github.repository }}/actions/runs/${{ github.run_id }}" }
      }'
```

---

## 5. Tata Kelola Rahasia Terpusat (Vendor-Agnostic Secret Architecture)

Ketika Aina menerima notifikasi insiden di service `billing-service`, Aina sering kali perlu mengakses token API Sentry, GitHub personal access token, atau koneksi database log.

Aina menggunakan arsitektur **Vendor-Agnostic Secret Provider Pattern** via CLI tunggal `secret_tool`. Sistem secara otomatis mendeteksi backend brankas yang tersedia tanpa mengharuskan Anda menggunakan Infisical.

### Provider Backends yang Didukung

| Provider | Variabel Lingkungan Utama | Cara Kerja |
| :--- | :--- | :--- |
| **Infisical** | `INFISICAL_CLIENT_ID`, `INFISICAL_CLIENT_SECRET`, `INFISICAL_PROJECT_ID` | Universal Auth (Machine Identity), token di-cache di `/root/.infisical/cached_token`. |
| **HashiCorp Vault** | `VAULT_ADDR`, `VAULT_TOKEN` (atau AppRole) | Terhubung ke Vault KV v1/v2 melalui CLI `vault` atau direct REST API (`/v1/secret/data/...`). |
| **Doppler** | `DOPPLER_TOKEN` | Doppler CLI (`doppler run`) atau service token. |
| **Local Persistent Env** | Otomatis aktif jika tidak ada cloud vault di atas | Menyimpan rahasia di berkas persisten volume Docker `/app/data/config/.secrets.env` (chmod 600). Zero configuration! |

> Operator dapat memaksa provider tertentu dengan menyetel environment variable:
> `SECRET_PROVIDER=infisical` atau `SECRET_PROVIDER=vault` atau `SECRET_PROVIDER=doppler` atau `SECRET_PROVIDER=env`.

### Contoh Konfigurasi di Docker / Coolify

#### Opsi A: Infisical
```ini
SECRET_PROVIDER=infisical
INFISICAL_DOMAIN=https://secrets.dvlpid.my.id/api
INFISICAL_PROJECT_ID=f13379e0-9661-4f8e-81ef-0e81d1502da1
INFISICAL_ENV=dev
INFISICAL_CLIENT_ID=d972328c-3c0c-4617-8c63-1cb1f7391f0f
INFISICAL_CLIENT_SECRET=1a73548b812589b6edf888ab64d8b0e52d615611feb7425ec75f5ea46017ae11
```

#### Opsi B: HashiCorp Vault
```ini
SECRET_PROVIDER=vault
VAULT_ADDR=https://vault.internal.mycompany.com:8200
VAULT_TOKEN=s.xxxxxxxxxxxxxxxxxxxx
VAULT_MOUNT=secret
```

#### Opsi C: Local Persistent (Zero External Vault)
```ini
SECRET_PROVIDER=env
# Rahasia otomatis disimpan di volume persisten /app/data/config/.secrets.env
```

### Mekanisme Eksekusi: JIT (Just-In-Time) Process Injection
Aina tidak pernah menyimpan token mentah di chat atau memori daemon. Ketika Aina menjalankan script pemeriksaan log atau diagnosa repositori, Aina menggunakan CLI wrapper `secret_tool`:

```bash
# 1. Mengecek ketersediaan kunci tanpa menampilkan nilai mentah
secret_tool list

# 2. Menjalankan perintah dengan secret terinjeksi JIT ke memori proses
secret_tool run -- python3 scripts/diagnose_service.py

# 3. Mengambil secret spesifik jika benar-benar dibutuhkan
secret_tool get SENTRY_AUTH_TOKEN --plain

# 4. Memeriksa status & provider yang aktif
secret_tool status
```

### Aturan Ketat OpSec (Anti-Leakage)
1. Nilai secret mentah **dilarang keras** dicetak ke riwayat obrolan WhatsApp atau web dashboard.
2. Aina hanya melaporkan status verifikasi (misal: *"Token Sentry untuk billing-service valid"*).
3. Secret tidak pernah ditulis ke file `.env` statis di disk workspace.

---

## 6. Ekstensibilitas Dinamis Multi-Repo via Git (`USER_SKILLS_REPO`)

Untuk mempermudah setup Aina di lingkungan baru tanpa perlu memodifikasi image container:

1. **Buat Repositori Git Pribadi**: Buat repo (misal: `my-org/aina-ops-skills.git`) yang berisi folder-folder skill khusus perusahaan Anda.
2. **Setel Variabel Lingkungan**:
   ```ini
   USER_SKILLS_REPO=https://github.com/my-org/aina-ops-skills.git
   ```
3. **Otomasi Entrypoint**:
   Saat container dinyalakan, [`docker-entrypoint.sh`](../docker-entrypoint.sh) akan otomatis melakukan clone atau `git pull --rebase` ke `/app/data/custom-skills`.
4. **Multi-Discovery (`.agents/skills.json`)**:
   Mesin Antigravity Aina otomatis membaca dan memuat seluruh tool yang ada di folder tersebut secara dinamis.

---

## 7. Skill Otonom Aina: `skills/event-triage` & `skills/skill-builder`

Aina dibekali dua meta-skill otonom bawaan untuk mengelola observability dan ekstensibilitas:

### A. Triase Insiden & Analisis Akar Masalah (`skills/event-triage`)
- **`SKILL.md`**: Memberikan instruksi formal kepada Aina bagaimana membaca notifikasi dari `/api/v1/events`, mengklasifikasikan tingkat urgensi, melakukan analisis akar masalah (*Root Cause Analysis / RCA*), dan menawarkan rekomendasi perbaikan.
- **`scripts/event_triage_tool.py`**:
  - `event_triage_tool parse -j '<JSON>'`: Mengubah JSON mentah menjadi triage card terstruktur.
  - `event_triage_tool parse -m -j '<JSON>'`: Menghasilkan ringkasan Markdown siap kirim ke obrolan.
  - `event_triage_tool check-secrets <SERVICE>`: Memeriksa apakah token vault untuk service terkait telah tersedia di Infisical.

### B. Pembuatan & Persistensi Skill Otonom (`skills/skill-builder`)
- **`SKILL.md`**: Memandu Aina secara otomatis saat pengguna meminta dibuatkan integrasi baru atau tool khusus untuk suatu repositori produksi.
- **Persistent Volume Guarantee**: Memastikan skill baru selalu disimpan di folder persisten `data/custom-skills/<nama>/` (volume `aina_data:/app/data/custom-skills/`), sehingga **tidak hilang saat container redeploy**.
- **`scripts/skill_scaffolder.py`**:
  - `skill_scaffolder init <NAME> --desc "<DESC>"`: Men-scaffold template skill standar Antigravity (`SKILL.md` dan executable script `chmod +x`).
  - `skill_scaffolder validate <NAME>`: Memvalidasi kepatuhan YAML frontmatter dan memindai potensi kebocoran hardcoded secrets (OpSec audit).
  - `skill_scaffolder sync [--repo <URL>]`: Melakukan sinkronisasi commit dan push folder custom skills ke remote Git pribadi pengguna (`USER_SKILLS_REPO`).

---

## 8. Siklus Hidup Eksekusi & Audit Trail

Setiap event yang masuk dicatat secara transparan di sistem audit Aina:

1. **Ingress Logging**: Event dicatat dengan ID pelacakan `evt_<epoch>_<millis>`.
2. **Gatekeeper Evaluation**: Karena event masuk sebagai `ChatType::DirectMessage` atau grup dengan `is_bot_mentioned: true`, domain gatekeeper otomatis meloloskannya untuk ditindaklanjuti.
3. **Action Audit**: Detail prompt, durasi investigasi, tools yang dipanggil (`secret_tool`, `event_triage_tool`, `bash`), dan teks laporan akhir disimpan ke database SQLite (`action_audits`).
4. **Dashboard & API Monitoring**: Operator dapat memantau jalannya audit melalui Web Dashboard (`/`) atau endpoint audit:
   - `GET /api/audit/actions`
   - `GET /api/audit/actions/{id}`
   - `GET /api/audit/summary`
