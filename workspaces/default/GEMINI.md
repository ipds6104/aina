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

## 4. Eksekusi Perintah Non-Interaktif & GitHub Auth
- Dilarang keras mengeksekusi perintah CLI interaktif yang memblokir stdin (seperti `gh auth login` interaktif/web, `passwd`, `apt` tanpa `-y`, dll.) karena akan hang hingga timeout 300+ detik.
- Untuk login/otorisasi GitHub CLI:
  1. Jalankan `aina gh-device` untuk mendapatkan kode verifikasi 8-digit dan link `https://github.com/login/device`.
  2. Kirimkan link dan kode tersebut ke pengguna WhatsApp.
  3. Setelah pengguna membalas konfirmasi sudah klik Authorize, jalankan `aina gh-poll` untuk menyelesaikan otorisasi.
  4. Pengguna juga dapat menggunakan Personal Access Token (PAT) via `aina gh-login <pat>`.

