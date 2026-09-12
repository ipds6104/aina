# Aina Multi-Workspace Sandbox Rules & Safety Charter

> [!IMPORTANT]
> Aturan ini merupakan payung tata kelola keamanan dan etika bagi seluruh workspace yang berada di bawah direktori `workspaces/`.

---

## 1. Batasan Perimeter Sandbox (Strict Workspace Boundary)

1. **Ruang Lingkup Operasi**:
   - Seluruh pembuatan berkas, skrip automasi, penarikan data, pemrosesan media, atau pembuatan artefak **WAJIB berada di dalam direktori workspace aktif saat ini**.
   - DILARANG KERAS memodifikasi atau menghapus berkas di luar workspace (khususnya `/app/data/`, `/app/config/`, `/app/src/`, `/root/.gemini/`, atau berkas konfigurasi sistem).
2. **Perlindungan Kredensial & Berkas Sensitif**:
   - Jangan pernah menuliskan token autentikasi, API Key, atau password ke dalam skrip secara *hardcoded*. Baca selalu melalui *environment variables*.
   - Jangan mengakses atau menyeberang ke direktori workspace proyek lain tanpa instruksi eksplisit.

---

## 2. Standar Kualitas Kode & Verifikasi Mandiri (Self-Verification)

1. **Verifikasi Sintaks Sebelum Selesai**:
   - Skrip Python: Jalankan `python3 -m py_compile <nama_file.py>` untuk memastikan tidak ada kesalahan sintaksis sebelum melapor kepada rekan kerja.
   - Skrip Shell/Bash: Periksa dengan `bash -n <nama_file.sh>`.
2. **Penanganan Error yang Tangguh**:
   - Tangani error secara eksplisit (gunakan try-except / status code check). Jangan menelan error secara diam-diam (*silent failure*).
   - Berikan pesan error yang informatif jika koneksi jaringan atau file input tidak ditemukan.

---

## 3. Komunikasi Balasan ke WhatsApp

1. **Format Teks WhatsApp yang Nyaman Dibaca di Layar HP**:
   - Gunakan `*tebal*` untuk penekanan/istilah kunci, `_miring_` untuk keterangan tambahan, dan ```blok kode``` untuk cuplikan sintaks/log.
   - **HINDARI tabel Markdown kompleks** (`| col1 | col2 |`) atau heading bertingkat (`# Heading`) karena akan berantakan di layar ponsel. Gunakan format bullet points atau daftar berangka yang rapi.
2. **Efisiensi Kode (Anti-Spam)**:
   - Jika kode melebihi 20 baris, simpan kode tersebut ke file di dalam direktori `scripts/` pada workspace aktif.
   - Berikan cuplikan inti yang paling penting di chat beserta perintah singkat 1 baris untuk menjalankannya.

---

## 4. Utilitas Kontrol Model AI (Native CLI & Skrip)

Gunakan perintah native `aina model`:
- `aina model get`: Cek model AI yang aktif saat ini.
- `aina model list`: Lihat daftar seluruh model yang didukung dan status aktifnya.
- `aina model set <model_name>`: Alihkan model aktif di runtime (prioritas keluarga Gemini, Opus hanya bila diminta eksplisit).
*(Atau via skrip fallback: `python3 scripts/model_control.py [get|list|set <model>]`)*

---

## 5. Tata Kelola Knowledge Base & Pelacakan Kegiatan (Temporal Activity)

1. **Struktur Pengetahuan Terisolasi**:
   - `knowledge/facts.md`: Catat fakta, kesepakatan, dan parameter penting umum yang bersifat permanen.
   - `knowledge/procedures.md`: Catat SOP dan panduan alur kerja universal.
   - `knowledge/kegiatan/<slug>/<periode>/README.md`: Untuk kegiatan/survei/proyek berbasis waktu, simpan terpisah dengan metadata YAML frontmatter (`nama`, `kategori`, `rutinitas`, `frekuensi`, `status`, `deadlines`).
2. **Pelacakan Agenda & Deadlines**:
   - Gunakan `aina kb schedule` (atau `python3 scripts/workspace_manager.py schedule [--week | --month | --overdue]`) untuk menjawab agenda kerja secara faktual dan tepat waktu tanpa spekulasi.
3. **Merapikan Indeks (Grooming)**:
   - Jalankan `aina kb groom` (atau `python3 scripts/workspace_manager.py groom <workspace>`) secara berkala agar ringkasan terkompilasi di `knowledge/index.md` selalu mutakhir untuk progressive retrieval.
4. **Pendeteksi Ketidakrapian Deterministik (Linter)**:
   - Jalankan `aina kb lint [--auto-heal]` (atau `python3 scripts/kb_linter.py <workspace> [--auto-heal]`) untuk memverifikasi kerapian struktur folder, metadata frontmatter, dan integritas tanggal tanpa mengonsumsi kuota token LLM.
5. **Audit Jejak Aksi Mandiri**:
   - Jalankan `aina audit [--limit 10 | --query <kata_kunci>]` (atau `python3 scripts/audit_agent.py [--since <durasi> | --query <kata_kunci>]`) untuk memeriksa riwayat perintah bash dan pengeditan berkas yang pernah dilakukan.
6. **Penyimpanan & Pencarian Arsip Chat Berbasis Waktu**:
   - Impor riwayat chat WhatsApp: `aina archive import <path_ke_zip> [--slug <nama>]`
   - Cari riwayat chat dengan filter temporal: `aina archive search "<query>" --since 7d` (atau `--from YYYY-MM-DD --to YYYY-MM-DD`) untuk mencegah halusinasi anachronistic.
7. **Manajemen Profil & Otoritas Rekan Kerja (Profiling Memory)**:
   - Periksa wewenang pengirim: `aina user get <sender_jid>`
   - Daftarkan/simpan izin rekan kerja: `aina user set <sender_jid> --name "<nama>" --role "<peran>" --authority <admin|staff|guest> --notes "<izin>"`
   - Cari rekan kerja: `aina user search "<kata_kunci>"`
   - Tampilkan seluruh daftar: `aina user list`
