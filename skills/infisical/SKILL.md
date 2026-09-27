---
name: infisical
description: >-
  Use this skill whenever you need to securely store, retrieve, or check API keys, tokens, or credentials
  in the centralized Infisical vault, or execute tools and scripts requiring secret credentials.
  Enforces zero-knowledge secret management and prevents credential leaks.
---

# Infisical Centralized Secret & Vault Management Skill

This skill equips Aina with formal operating procedures to interact with the **Infisical Secret Vault** (`https://secrets.dvlpid.my.id/api`) using the companion CLI tool [`scripts/secret_tool.py`](./scripts/secret_tool.py).

---

## 1. When to Activate This Skill (Trigger Conditions)

Activate this skill immediately whenever:
1. **Pengguna Memberikan Token/Kredensial Baru di Chat**:
   Pengguna mengirimkan API key, personal access token, client secret, atau password di WhatsApp (misal: *"Aina, ini API token Tally: `tly-xxxx`"* atau *"Ini bot token Telegram: `12345:xxxx`"*).
2. **Membuat atau Menjalankan Custom Skill Baru**:
   Aina sedang membuat skrip atau tool baru yang memerlukan kredensial pihak ketiga (misal: Tally, Notion, GitHub, Supabase, Stripe).
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
> 3. **DILARANG MENULIS SECRET KE FILE `.env` LOKAL WORKSPACE**:
>    Kredensial wajib dikirim langsung ke Infisical vault via `secret_tool.py set`.

---

## 3. Standard Operating Procedures (SOP)

### SOP 1: Menyimpan Secret Baru yang Diberikan Pengguna
1. Format nama kunci menjadi UPPERCASE standar: `<SERVICE>_API_KEY` atau `<SERVICE>_TOKEN` (contoh: `TALLY_API_KEY`, `NOTION_API_KEY`).
2. Jalankan perintah terminal:
   ```bash
   python3 skills/infisical/scripts/secret_tool.py set <KEY> "<VALUE>"
   ```
   *(atau bila di sistem sudah terpasang alias: `secret_tool set <KEY> "<VALUE>"`)*.
3. Konfirmasi ke pengguna dengan elegan dan santun:
   > *"Baik Kak, token Tally sudah Aina amankan langsung ke dalam brankas Infisical dengan kunci `TALLY_API_KEY`. Token siap digunakan dan tidak tersimpan di teks riwayat obrolan 🙏"*

### SOP 2: Mengeksekusi Skrip dengan Secret Terinjeksi (Just-in-Time)
Saat menjalankan skrip Python atau tool eksternal yang memerlukan secret:
Gunakan perintah `run` agar Infisical menyuntikkan seluruh environment variable secara aman ke dalam proses tanpa membuat file sementara di disk:
```bash
python3 skills/infisical/scripts/secret_tool.py run -- python3 scripts/my_tool.py
```

### SOP 3: Mengambil Nilai Secret Tertentu (Jika Dibutuhkan Langsung)
Jika skrip membutuhkan nilai tunggal secara langsung:
```bash
python3 skills/infisical/scripts/secret_tool.py get <KEY> --plain
```

### SOP 4: Memeriksa Daftar Secret yang Tersedia
Untuk memeriksa apakah kunci tertentu sudah ada:
```bash
python3 skills/infisical/scripts/secret_tool.py list
```

---

## 4. Helper Tool Command Reference

| Subcommand | Sintaks Perintah | Fungsi |
| :--- | :--- | :--- |
| `set` | `secret_tool set <KEY> "<VALUE>"` | Menyimpan secret baru ke vault Infisical (output tersanitasi). |
| `get` | `secret_tool get <KEY> [--plain]` | Mengambil secret (default bertopeng, `--plain` untuk nilai mentah). |
| `list` | `secret_tool list` | Menampilkan seluruh nama kunci yang tersimpan di vault (tanpa menampilkan nilainya). |
| `run` | `secret_tool run -- <COMMAND>` | Menjalankan perintah dengan seluruh secret terinjeksi ke env proses. |
| `status` | `secret_tool status` | Memeriksa konektivitas dan token autentikasi Infisical. |

---

## 5. Failure Modes & Mitigasi

* **Token Expired / Auth Failed**: Jalankan `secret_tool status`. Jika token belum ada, `secret_tool.py` akan otomatis mencoba autentikasi Universal Auth menggunakan kredensial mesin di environment (`INFISICAL_CLIENT_ID` dan `INFISICAL_CLIENT_SECRET`).
* **Koneksi Jaringan Timeout**: Periksa apakah host bisa mengakses domain `https://secrets.dvlpid.my.id/api`. Jika offline, gunakan fallback variabel lingkungan yang sudah ada.
