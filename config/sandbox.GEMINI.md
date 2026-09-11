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
2. **Format Teks WhatsApp**:
   - Gunakan `*tebal*`, `_miring_`, dan ```blok kode```.
   - Jangan gunakan `#` heading atau tabel Markdown `| a | b |` yang dapat merusak tampilan layar ponsel pengguna.
