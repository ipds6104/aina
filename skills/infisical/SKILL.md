---
name: infisical
description: >-
  Use this skill whenever you need to securely store, retrieve, or check API keys, tokens, or credentials
  in centralized vaults (Infisical, HashiCorp Vault, Doppler, or persistent local environment),
  or execute tools and scripts requiring secret credentials via JIT injection.
  Enforces zero-knowledge secret management and prevents credential leaks.
---

# Universal Secret & Vault Management Skill (Vendor-Agnostic)

This skill equips Aina with production-grade operating procedures to interact with centralized secret vaults (**Infisical**, **HashiCorp Vault**, **Doppler**, or **Local Persistent Store**) using the universal CLI tool [`scripts/secret_tool.py`](./scripts/secret_tool.py).

---

## 1. When to Activate This Skill (Trigger Conditions)

Activate this skill immediately whenever:
1. **Pengguna Memberikan Token/Kredensial Baru di Chat**:
   Pengguna mengirimkan API key, personal access token, client secret, atau password di WhatsApp (misal: *"Aina, ini API token Tally: `tly-xxxx`"* atau *"Ini token Sentry: `sntryu_xxxx`"*).
2. **Membuat atau Menjalankan Custom Skill Baru**:
   Aina sedang membuat skrip atau tool baru yang memerlukan kredensial pihak ketiga (misal: Sentry, Datadog, Tally, Notion, GitHub, Supabase).
3. **Memeriksa Ketersediaan Kredensial**:
   Aina perlu memastikan apakah API key tertentu sudah tersimpan di vault sebelum menjalankan operasi eksternal.
4. **Error Autentikasi (401 / Unauthorized)**:
   Tool eksternal gagal karena kredensial kadaluarsa atau belum disetel di lingkungan.

---

## 2. Strict OpSec & Anti-Leakage Rules (Paling Penting!)

> [!CAUTION]
> **ATURAN MUTLAK KEAMANAN KREDENSIAL**:
> 1. **DILARANG MENGULANG NILAI TOKEN MENTAH KE DALAM BALASAN CHAT!**
>    Saat mengonfirmasi kepada pengguna, HANYA sebutkan nama kuncinya (misal: `TALLY_API_KEY`). Jangan pernah meng-copy paste string token mentah ke teks balasan.
> 2. **DILARANG KERAS MENG-HARDCODE SECRET KE DALAM FILE KODE / SKILL**:
>    Jangan pernah menulis `API_KEY = "tly-xxxx"` di dalam file `.py`, `.sh`, atau `SKILL.md`. Kode harus selalu membaca dari environment variable: `os.getenv("TALLY_API_KEY")`.
> 3. **GUNAKAN INJEKSI JUST-IN-TIME (JIT)**:
>    Jalankan perintah dengan `secret_tool run -- ...` agar secret hanya disuntikkan ke memori proses tanpa pernah disimpan ke file disk statis.

---

## 3. Arsitektur Multi-Provider (Vendor-Agnostic)

`secret_tool` secara otomatis mendeteksi provider brankas yang tersedia di lingkungan sistem:

| Provider | Indikator Deteksi Otomatis | Mode Autentikasi |
| :--- | :--- | :--- |
| **Infisical** | `INFISICAL_CLIENT_ID` / `INFISICAL_TOKEN` | Universal Auth (Machine Identity), token di-cache di `/root/.infisical/cached_token`. |
| **HashiCorp Vault** | `VAULT_ADDR` & `VAULT_TOKEN` | Vault KV v1/v2 via CLI `vault` atau direct REST API (`/v1/secret/data/...`). |
| **Doppler** | `DOPPLER_TOKEN` | Doppler CLI (`doppler run`) atau service token. |
| **Local Env (Fallback)** | Tidak ada provider cloud di atas | Disimpan di volume persisten `/app/data/config/.secrets.env` (chmod 600). Zero-config! |

> Operator dapat memaksa provider tertentu dengan menyetel environment variable:
> `SECRET_PROVIDER=infisical` atau `SECRET_PROVIDER=vault` atau `SECRET_PROVIDER=env`.

---

## 4. Standard Operating Procedures (SOP)

### SOP 1: Menyimpan Secret Baru yang Diberikan Pengguna
1. Format nama kunci menjadi UPPERCASE standar: `<SERVICE>_API_KEY` atau `<SERVICE>_TOKEN` (contoh: `SENTRY_AUTH_TOKEN`, `TALLY_API_KEY`).
2. Jalankan perintah terminal:
   ```bash
   python3 skills/infisical/scripts/secret_tool.py set <KEY> "<VALUE>"
   # atau jika alias terpasang:
   secret_tool set <KEY> "<VALUE>"
   ```
3. Konfirmasi ke pengguna dengan elegan dan santun:
   > *"Baik Kak, token sudah Aina amankan langsung ke dalam brankas rahasia dengan kunci `<KEY>`. Token siap digunakan dan tidak tersimpan di teks riwayat obrolan 🙏"*

### SOP 2: Mengeksekusi Skrip dengan Secret Terinjeksi (Just-in-Time)
Saat menjalankan skrip Python atau tool eksternal yang memerlukan secret:
Gunakan perintah `run` agar secret disuntikkan secara aman ke dalam *environment* proses tanpa membuat file sementara di disk:
```bash
secret_tool run -- python3 scripts/my_tool.py
```

### SOP 3: Mengambil Nilai Secret Tertentu (Jika Dibutuhkan Langsung)
Jika skrip membutuhkan nilai tunggal secara langsung:
```bash
secret_tool get <KEY> --plain
```

### SOP 4: Memeriksa Daftar Secret yang Tersedia
Untuk memeriksa apakah kunci tertentu sudah ada:
```bash
secret_tool list
```

### SOP 5: Mengonfigurasi / Beralih Provider Brankas (Setup Vault via Chat)
Jika pengguna meminta Aina menghubungkan atau beralih ke backend brankas tertentu:
1. **Untuk HashiCorp Vault**:
   ```bash
   secret_tool configure --provider vault --addr "<VAULT_ADDR>" --token "<VAULT_TOKEN>"
   ```
2. **Untuk Infisical**:
   ```bash
   secret_tool configure --provider infisical --client-id "<CLIENT_ID>" --client-secret "<CLIENT_SECRET>" --project-id "<PROJECT_ID>"
   ```
3. **Untuk Doppler**:
   ```bash
   secret_tool configure --provider doppler --token "<DOPPLER_TOKEN>"
   ```
4. **Untuk Local Persistent Mode (Stand-alone)**:
   ```bash
   secret_tool configure --provider env
   ```
5. Aina memverifikasi status koneksi lalu mengonfirmasi ke pengguna:
   ```bash
   secret_tool status
   ```

### SOP 6: Memeriksa Status & Provider Aktif
```bash
secret_tool status
```

---

## 5. Helper Tool Command Reference

| Subcommand | Sintaks Perintah | Fungsi |
| :--- | :--- | :--- |
| `set` | `secret_tool set <KEY> "<VALUE>"` | Menyimpan secret baru ke vault aktif (output tersanitasi). |
| `get` | `secret_tool get <KEY> [--plain]` | Mengambil secret (default bertopeng, `--plain` untuk nilai mentah). |
| `list` | `secret_tool list` | Menampilkan seluruh nama kunci yang tersimpan di vault (tanpa menampilkan nilainya). |
| `run` | `secret_tool run -- <COMMAND>` | Menjalankan perintah dengan seluruh secret terinjeksi ke env proses. |
| `status` | `secret_tool status` | Memeriksa konektivitas dan provider brankas yang sedang aktif. |
