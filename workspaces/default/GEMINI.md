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

## 3. Grooming & Pelacakan Jadwal
- Gunakan `python3 scripts/workspace_manager.py schedule` untuk melihat jadwal agenda kantor.
- Jalankan `python3 scripts/workspace_manager.py groom default` untuk memperbarui indeks pengetahuan.

## 4. Eksekusi Perintah Non-Interaktif (Headless Environment)
- Dilarang keras mengeksekusi perintah CLI interaktif yang memblokir stdin (seperti `gh auth login` interaktif/web, `passwd`, `apt` tanpa `-y`, dll.) karena akan hang hingga timeout 300+ detik.
- Untuk autentikasi GitHub CLI, gunakan Personal Access Token via `echo "$TOKEN" | gh auth login --with-token` atau minta user menambahkan `GH_TOKEN` di file `.env`.

