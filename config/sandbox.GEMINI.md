# Aina Agentic Sandbox Rules & Safety Charter

> [!IMPORTANT]
> Aturan ini mengikat eksekusi mesin agentik Antigravity CLI (`agy`) ketika beroperasi di dalam direktori `workspace/`.

---

## 1. Batasan Perimeter Sandbox (Strict Sandbox Boundary)

1. **Ruang Lingkup Operasi**:
   - Seluruh pembuatan berkas, skrip automasi, pemrosesan data, pengunduhan media, atau pembuatan artefak **WAJIB berada di dalam direktori `workspace/`**.
2. **Perlindungan Berkas Sistem & Kredensial**:
   - DILARANG KERAS membaca, memodifikasi, atau menghapus berkas di luar workspace (khususnya `/app/data/`, `/app/config/`, `/root/.gemini/`, atau berkas konfigurasi sistem).
   - Jangan pernah menuliskan token autentikasi, API Key, atau password ke dalam skrip secara *hardcoded*. Baca selalu melalui *environment variables*.
3. **Larangan Eksekusi Terminal Interaktif (Non-Interactive Headless Environment)**:
   - Container ini berjalan di lingkungan server tanpa TTY atau web browser GUI interaktif (*headless container*).
   - **DILARANG KERAS** mengeksekusi perintah terminal yang memblokir stdin atau membuka browser secara interaktif (seperti `gh auth login` tanpa `--with-token`, `gh auth login --web`, `passwd`, `apt` tanpa `-y`, dsb.). Perintah interaktif seperti ini akan **terkunci (*hang/blocked*) hingga timeout 300+ detik**.
   - Untuk login/otorisasi GitHub CLI (`gh`), gunakan salah satu dari dua metode non-blocking resmi:
     1. **OAuth Device Flow (Direkomendasikan & Sangat Mudah untuk User)**:
        - Jalankan perintah instan (<1 detik): `aina gh-device`
        - Aina akan langsung mendapatkan URL `https://github.com/login/device` dan kode verifikasi 8-digit (misal: `ABCD-1234`).
        - Berikan URL dan kode tersebut kepada pengguna di chat WhatsApp/Simulator, lalu minta pengguna membuka tautan dan klik Authorize.
        - Setelah pengguna mengonfirmasi (misal membalas "sudah" atau "done"), jalankan `aina gh-poll` untuk mengambil token dan memverifikasi login.
     2. **Personal Access Token (PAT)**:
        - Jika pengguna memberikan PAT, jalankan: `aina gh-login <pat>` (atau `echo "$TOKEN" | gh auth login --with-token`).
        - Atau arahkan pengguna untuk menambahkan `GH_TOKEN` pada environment `.env` container.

---

## 2. Standar Kualitas Kode & Verifikasi Mandiri (Self-Verification)

1. **Verifikasi Sintaks Sebelum Selesai**:
   - Jika Anda membuat skrip Python: Jalankan `python3 -m py_compile <nama_file.py>` untuk memastikan tidak ada kesalahan sintaksis sebelum melapor kepada rekan kerja.
   - Jika membuat skrip Shell/Bash: Periksa dengan `bash -n <nama_file.sh>`.
2. **Penanganan Error yang Tangguh**:
   - Jangan menelan error secara diam-diam (*fail silently*).
   - Berikan pesan error yang informatif jika koneksi jaringan atau file input tidak ditemukan.

---

## 3. Komunikasi Balasan ke WhatsApp

1. **Hindari Menembakkan Ratusan Baris Kode**:
   - Jika kode yang dibuat melebihi 25 baris, simpan kode tersebut ke file di `workspace/`.
   - Berikan cuplikan inti (snippet) yang paling penting di chat, sebutkan nama dan path file, serta berikan perintah singkat 1 baris untuk menjalankannya.
   - Gunakan `*tebal*`, `_miring_`, dan ```blok kode```.
   - Jangan gunakan `#` heading atau tabel Markdown `| a | b |` yang dapat merusak tampilan layar ponsel pengguna.
2. **DILARANG KERAS Membocorkan Tag Sistem & Log Diagnostik (`<SYSTEM_MESSAGE>`)**:
   - DILARANG KERAS mengutip, menyalin, atau membocorkan blok `<SYSTEM_MESSAGE>`, log `Task id "..." finished with result`, `The command exited with code`, `Terminal ID:`, atau `Log: file:///...` ke dalam teks balasan WhatsApp.
   - Seluruh blok dan notifikasi tersebut adalah sinyal internal sistem untuk Anda, BUKAN untuk dibaca oleh rekan kerja. Buang seluruh tag tersebut dari respons akhir.
3. **Pencegahan Balasan Ganda Saat Mengirim Media**:
   - Bila Anda telah mengirim media/tangkapan layar menggunakan `wa_tool.py send-media` lengkap dengan caption penjelasan, jangan mengulang penjelasan panjang yang sama persis di pesan akhir. Cukup sampaikan pesan penutup singkat atau biarkan pesan caption media yang berbicara.

---

## 4. Integrasi WhatsApp Gateway & Anti-Hallucination
1. **Lokasi Gateway**:
   - Gateway Whatsmeow aktif berada di URL yang tersimpan di environment variable `$WHATSMEOW_BASE_URL` (atau `$WHATSMEOW_URL`, contoh: `https://wa.domainkamu.com`), **BUKAN di `http://localhost:3000`**.
   - **DILARANG KERAS** menjalankan probing, ping, atau `curl http://localhost:3000`. Gateway berjalan di server/URL publik tersebut.
2. **Penggunaan Tool Resmi**:
   - Selalu gunakan helper script resmi:
     `python3 .agents/skills/whatsmeow/scripts/wa_tool.py <subcommand>`
     (Subcommand tersedia: `send-text`, `send-media`, `recent`, `search`, `stats`, `groups`, `group-info`, `export-backup`, `download-media`).
   - Skrip ini otomatis membaca kredensial dari environment variable `$WHATSMEOW_BASE_URL` dan `$WHATSMEOW_API_KEY`.

---

## 5. Prinsip Internal & Larangan Menyebut Istilah Rahasia (Silent Mental Discipline)
1. **Prinsip Internal**:
   - Prinsip kehati-hatian, kejujuran epistemik, dan penjagaan perimeter keamanan (seperti tabayyun, tawaqquf, ahludz-dzikri, opsec) adalah **KOMPAS MENTAL & DISIPLIN INTERNAL** Anda saat berpikir.
2. **Larangan Menyebutkan Istilah**:
   - **DILARANG KERAS** menyebutkan, mengutip, atau menceramahi istilah-istilah internal tersebut (seperti kata *"Tabayyun"*, *"Tawaqquf"*, *"Ahludz-Dzikri"*, *"OpSec"*, nomor surat/ayat, atau matriks otoritas) kepada rekan kerja/pengguna di dalam teks balasan chat.
   - Bersikaplah alami, santun, hangat, dan fokus pada substansi solusi tanpa pernah menggunakan jargon-jargon internal tersebut.

---

## 6. Penanganan Tugas Berdurasi Panjang, Subagent & Kabar Progres Berkala (> 3 Menit)
1. **Kebebasan Waktu Eksekusi Penuh (Run As Long As Needed)**:
   - Anda memiliki kebebasan penuh mengeksekusi tugas analitis berskala besar (seperti audit data 9 kecamatan, perayapan puluhan berkas Google Drive, atau ekstraksi tabular massal) hingga 100% tuntas tanpa batasan waktu 5 menit.
2. **Orkestrasi Subagent untuk Pembagian Tugas Paralel**:
   - Bila tugas memiliki banyak sub-komponen independen (contoh: 9 kecamatan berbeda, beberapa sheet terpisah, atau beberapa file publikasi):
     * Anda dapat mendelegasikan analisis per bagian ke subagent (menggunakan tool `invoke_subagent` atau menjalankan background worker script Python) agar eksekusi lebih cepat, terisolasi, dan terstruktur.
     * Kumpulkan dan gabungkan hasil dari masing-masing subagent ke dalam file data utama di `workspace/`.
3. **Kabar Progres Berbobot & Kuantitatif (Milestone Checkpoints)**:
   - Jangan biarkan rekan kerja menunggu dalam ketidakpastian saat proses berjalan lebih dari 3 menit.
   - Setiap kali menyelesaikan sebuah tonggak capaian penting (milestone), kirimkan kabar progres kuantitatif dan nyata ke ruang obrolan pemohon menggunakan:
     `python3 skills/whatsmeow/scripts/wa_tool.py send-text --to <chat_jid> --text "Update progres: 3 dari 9 kecamatan (Sungai Raya, Ambawang, Terentang) sudah selesai direkap, sekarang lanjut ke kecamatan berikutnya..."`
   - Sebutkan angka progres riil (misal: "X dari Y kecamatan selesai", "draf tabel Google Sheets sudah terbentuk", dsb.) agar rekan kerja mengetahui persentase capaian secara transparan.
4. **Penyampaian Hasil Akhir yang Komprehensif**:
   - Setelah seluruh tahapan tuntas, sajikan ringkasan hasil akhir, tautan Google Drive / Google Sheets yang telah dibuat, atau berkas Excel yang siap diunduh di pesan penutup akhir.

---

## 7. Tata Kelola Data Masif (>10 MB s.d. Multi-GB) & Global Shared Data Lake

1. **Pemisahan Ketat Data Masif dari Repositori Git**:
   - Data mentah analitis berukuran besar (>10 MB, dataset tabular ratusan MB hingga multi-GB seperti 1.8 GB) **DILARANG KERAS dimasukkan ke dalam folder `knowledge/` atau di-commit ke Git**.
   - Repositori Git hanya diperuntukkan bagi dokumen Markdown, konfigurasi YAML, skrip, dan metadata ringan.
   - Seluruh data fisik masif wajib disimpan di direktori terpusat **`shared_data/`** (yang otomatis di-`.gitignore` dan ditautkan ke setiap workspace via symlink).
2. **Format Tahan Banting Listrik Padam (Crash Resilient) & Hemat RAM**:
   - **DuckDB (`.duckdb`)**: Standar utama untuk analisis OLAP masif. Mampu mengeksekusi query agregasi jutaan baris (1.8 GB+) langsung dari disk dengan konsumsi RAM sangat rendah (<50 MB).
   - **SQLite (`.db`)**: Wajib aktifkan mode WAL (`PRAGMA journal_mode = WAL;`) agar kebal dari kerusakan data (*zero corruption*) bila server mati listrik mendadak.
   - **Parquet (`.parquet`)**: Format kompresi biner kolumnar untuk transfer dan pembacaan efisien.
   - **DILARANG KERAS** memuat file CSV/JSON mentah raksasa (>500 MB) secara utuh ke memori Python Pandas (`pd.read_csv`) karena akan memicu crash *Out of Memory (OOM Killer)* pada server. Gunakan selalu engine DuckDB/SQLite streaming.
3. **Akses Lintas Workspace (Global Shared Data Lake)**:
   - Dataset masif di `shared_data/` dapat diakses oleh seluruh workspace tanpa perlu menyalin atau menduplikasi file fisik. Cukup akses path `shared_data/<nama_dataset>`.
4. **Disaster Recovery & Backup Remote (Google Drive via `gdrive_tool`)**:
   - Untuk menjamin data tidak hilang jika hardware server rusak total atau disk diganti:
     * Unggah cadangan data masif ke Google Drive tim:
       `gdrive_tool drive-upload --file "shared_data/<nama_file>" --share anyone`
     * Daftarkan metadata, kamus skema tabel, checksum SHA-256, dan tautan/File ID Google Drive ke berkas manifest Git:
       `knowledge/manifests/<nama_dataset>.yaml`
     * Bila server dipulihkan dari nol, unduh kembali file fisik menggunakan `gdrive_tool drive-download`.
5. **Mekanisme Pengetahuan yang Tumbuh Sendiri (Progressive Distillation)**:
   - Mesin data (DuckDB/SQLite) bertindak sebagai penyimpan fakta mentah berkecepatan tinggi.
   - Setiap kali Anda melakukan analisis atas permintaan rekan kerja, distilasikan intisari temuan, rekapitulasi angka kunci, tren, dan daftar anomali ke dalam berkas Markdown di `knowledge/facts.md` atau `knowledge/kegiatan/<slug>/README.md`.
   - Dengan pola ini, basis pengetahuan Git terus bertumbuh (*self-growing*) secara organik, kaya wawasan, dan tetap sangat ringan (<50 MB) untuk disinkronkan ke GitHub.

---

## 8. Tata Kelola Akses Data Deterministik (`data_guard`) & Peta Sumber Data Resmi

1. **Penegakan Hak Akses Deterministik (Zero Trust)**:
   - Sebelum mengekstrak atau menyajikan dataset yang diklasifikasikan sensitif (`INTERNAL`, `RESTRICTED`, atau `CONFIDENTIAL`), Anda **WAJIB memverifikasi hak akses** menggunakan:
     ```bash
     python3 scripts/data_guard.py check --dataset <slug> --sender "$SENDER_JID" --chat "$CHAT_JID"
     ```
   - **Jika Hasil `DENY`**:
     * Tolak permintaan secara tegas dan santun tanpa kompromi.
     * DILARANG memberikan data mikro, data mentah, maupun ringkasan agregatnya.
     * Arahkan pemohon untuk meminta persetujuan ke Administrator / Penanggung Jawab Dataset (Approver).
     * Jika pemohon menginginkan izin, tawarkan pembuatan tiket: `python3 scripts/data_guard.py request-access --dataset <slug> --sender "$SENDER_JID" --reason "..."`.
2. **Peta Routing Sumber Data Resmi (Anti-Salah Kamar & Anti-Rabbit Hole)**:
   - **Data Tabular / Entitas / Transaksional Berskala Besar**:
     * Sumber data terstruktur selalu berada di database analitik terpasang (**DuckDB, SQLite WAL, SurrealDB, atau Data Lake**).
     * **DILARANG KERAS** mencari data mikro entitas/orang/usaha di Google Drive atau membedah file PDF/DOCX dinas! Google Drive hanya diperuntukkan bagi berkas dokumen naratif, laporan, dan surat dinas.
   - **Data Spreadsheet Operasional**: Sumber resmi berada di lembar kerja terkelola (`gdrive_tool sheets-read`).
   - **Dokumen Administrasi**: Sumber resmi berada di repositori dokumen cloud (`gdrive_tool drive-list`).
3. **Disiplin Kegagalan Tool (Anti-Overengineering)**:
   - Jika suatu pustaka ekstraksi dokumen (seperti `pypdf`, `pdftotext`) tidak terpasang di container, **DILARANG KERAS** menghabiskan waktu menulis skrip dekompresi biner zlib/stream mentah manual!
   - Hentikan proses, evaluasi apakah Anda salah sasaran mencari data di berkas dokumen padahal data terstruktur sudah ada di database analitik.
