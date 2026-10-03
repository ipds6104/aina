# Workspace Default (General Office & Daily Assistant)

> [!NOTE]
> Workspace ini didedikasikan untuk tugas harian, otomasi umum, pembuatan skrip serbaguna, dan bantuan teknis non-spesifik proyek.

---

## 1. Lingkup Pekerjaan Default
- Rekayasa perangkat lunak umum (Python, Bash, automasi ringan).
- Pengujian API, cek jaringan, dan troubleshooting sistem kantor.
- Pembuatan catatan rapat, draf dokumen, atau analisis teks singkat.

## 2. Struktur Penyimpanan
- Simpan berkas pengetahuan umum di `knowledge/facts.md` dan `knowledge/procedures.md`.
- Simpan kegiatan atau proyek berjangka di `knowledge/kegiatan/<nama>/<periode>/`.
- Simpan skrip automasi di subdirektori `scripts/`.
- Simpan berkas hasil olahan data atau unduhan sementara di subdirektori `data/`.

## 3. Grooming, Pelacakan Jadwal, & Utilitas Native CLI (`aina`)
- **Agenda & Deadlines**: Jalankan `aina kb schedule` (atau `python3 scripts/workspace_manager.py schedule`).
- **Grooming Pengetahuan**: Jalankan `aina kb groom` untuk memperbarui indeks katalog `knowledge/index.md`.
- **Kerapian Pengetahuan**: Jalankan `aina kb lint --auto-heal` untuk memvalidasi dan merapikan format berkas.
- **Pencarian Riwayat Chat**: Jalankan `aina archive search "<query>" --since 7d` (atau `--from YYYY-MM-DD --to YYYY-MM-DD`).
- **Profil & Otoritas Rekan Kerja**: Jalankan `aina user get <sender_jid>` dan `aina user set <sender_jid> ...` untuk mengelola hak wewenang.
- **Kontrol Model AI**: Jalankan `aina model get`, `aina model list`, atau `aina model set <model>`.
- **Manajemen Persona & Rollback**: Jalankan `aina persona status`, `aina persona backup [target] -m "..."`, `aina persona history [target]`, `aina persona rollback [target]`, atau `aina persona reset [target]` (mendukung: persona, character, activities, organization).

## 4. Eksekusi Perintah Non-Interaktif & GitHub Auth
- Dilarang keras mengeksekusi perintah CLI interaktif yang memblokir stdin (seperti `gh auth login` interaktif/web, `passwd`, `apt` tanpa `-y`, dll.) karena akan hang hingga timeout 300+ detik.
- Untuk login/otorisasi GitHub CLI:
  1. Jalankan `aina gh-device` untuk mendapatkan kode verifikasi 8-digit dan link `https://github.com/login/device`.
  2. Kirimkan link dan kode tersebut ke pengguna WhatsApp.
  3. Setelah pengguna membalas konfirmasi sudah klik Authorize, jalankan `aina gh-poll` untuk menyelesaikan otorisasi.
  4. Pengguna juga dapat menggunakan Personal Access Token (PAT) via `aina gh-login <pat>`.

## 5. Ekstraksi Dokumen & PDF (Vision VLM Engine)
- **Tool Resmi**: `python3 skills/vision-document-extractor/scripts/doc_extract.py` (atau `/usr/local/bin/agy-doc-extract`)
- **Skill Terkait**: `skills/vision-document-extractor`
- **Output Default**: `output/doc_extract/`
- **Aturan Eksekusi & Kesadaran Konkurensi**:
  - Selalu gunakan skrip ekstraktor di atas untuk berkas `.pdf`, invoice, struk belanja, bagan, atau gambar dokumen teks rapat.
  - Dilarang mencoba membaca biner PDF mentah menggunakan `view_file` atau `cat` secara langsung.
  - **Konkurensi Paralel Cepat**: Gunakan flag `-c 8` atau `-c 16` untuk dokumen multi-halaman agar semua halaman diproses serentak via 9Router (selesai dalam hitungan 2–3 detik).
  - **Auto-Restart / Retry**: Skrip otomatis melakukan retry 3x dengan exponential backoff jika terjadi gangguan jaringan sesaat.
  - **Filter Halaman**: Bila pengguna hanya menanyakan halaman tertentu, gunakan `-p <halaman>` (misal `-p 3-5`).
  - **Konversi CSV**: Bila pengguna minta tabel diubah ke file CSV/Excel, tambahkan `--csv`, lalu kirimkan file `output/doc_extract/*.csv` ke pengguna via `python3 skills/whatsmeow/scripts/wa_tool.py send-media`.
  - Setelah ekstraksi selesai, baca file `output/doc_extract/extracted_content.md` untuk menjawab pengguna.

## 6. Tata Kelola Secret (Infisical Vault) & Custom Skills
- **Skill Terkait**: `skills/infisical`
- **Tool Resmi**: `python3 skills/infisical/scripts/secret_tool.py` (atau `secret_tool <cmd>`)
- **Penyimpanan Token Baru**: Bila pengguna membagikan API key/token di chat, simpan via `secret_tool set <KEY> "<VALUE>"`. DILARANG mengulang string token mentah di balasan WhatsApp.
- **Injeksi Secret Just-in-Time**: Jalankan skrip yang membutuhkan secret via `secret_tool run -- python3 ...`.
- **Pembuatan Custom Skill**: Simpan skill kustom baru di `data/custom-skills/<nama>/` lengkap dengan `SKILL.md` dan `scripts/`. Tawarkan backup ke repo GitHub privat (`USER_SKILLS_REPO`) via `gh` CLI setelah berhasil diuji.
