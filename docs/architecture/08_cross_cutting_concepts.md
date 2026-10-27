# 8. Konsep Lintas Komponen (Cross-Cutting Concepts)

> [!INFO] **Architecture Provenance & Freshness**
> - **Verified Commit**: `e9b88a2` (branch: `main`)
> - **Verified Timestamp**: `2026-10-26T11:19:12+07:00`
> - **Tracked Paths**: `src/`, `config/`, `skills/`
> - **Drift Status**: `verified`

Bab ini membahas prinsip-prinsip menyeluruh yang berlaku di semua bagian sistem, meliputi aspek keamanan, tata krama komunikasi, kesadaran waktu, dan ketahanan sistem.

---

## 8.1 Keamanan dan Kerahasiaan Data (Security & Privacy)

### 1. Kurir Rahasia Tepat Waktu (*Runtime Secret Broker*)
- Kunci API, password database, atau token akses eksternal tidak pernah disimpan dalam file teks biasa di repositori.
- Saat sebuah alat bantu membutuhkan kunci, alat tersebut memanggil kurir rahasia (`secret_tool run -- ...`). Kunci disuntikkan langsung ke variabel lingkungan proses saat itu saja (*Just-in-Time*), dan hilang saat proses selesai.
- Mendukung berbagai brankas rahasia: Infisical, HashiCorp Vault, Doppler, maupun berkas `.secrets.env` lokal (chmod 600) untuk pengujian offline.

### 2. Penyaringan Balasan Chat Otomatis (*Chat Sanitizer*)
- Sebelum pesan dikirim ke WhatsApp, teks balasan diperiksa oleh penyaring (`src/adapters/driven/agy_cli/sanitizer.rs`).
- Jika model AI secara tidak sengaja mencetak token internal, status task latar belakang, atau potongan kunci rahasia, penyaring akan langsung menghapus teks tersebut demi keamanan.

### 3. Batas Privasi Antar-Sesi (*Cross-Session Privacy Boundaries*)
- Obrolan pribadi pengguna dengan Aina adalah rahasia mutlak.
- Aina dilarang keras mengutip, menceritakan, atau membawa konteks obrolan pribadi ke dalam grup kantor atau ke grup lain.

### 4. Tingkat Kepercayaan Bertahap (*Progressive Trust*)
- Aina mencatat peran setiap rekan kerja di database SQLite:
  - `admin`: Pemilik bot / Companion (kuasa penuh).
  - `staff`: Rekan kerja terverifikasi (bisa meminta olah data rutin).
  - `guest`: Kontak baru atau orang luar yang belum diverifikasi.
- Jika ada `guest` yang meminta data sensitif atau instruksi berbahaya di grup, Aina menerapkan prinsip **Tawaqquf** (menahan diri dan tidak langsung memberikan data) dan meminta konfirmasi pribadi ke pemilik bot via jalur pribadi (*DM*).

---

## 8.2 Kesadaran Waktu dan Penanggalan Lokal (Temporal Awareness)

1. **Zona Waktu Resmi WIB (UTC+7)**:
   - Seluruh aktivitas, pembuatan pengingat, dan riwayat obrolan dihitung berdasarkan waktu **WIB (`Asia/Jakarta`)** dengan nama hari dan bulan bahasa Indonesia.
   - Menghindari kebingungan jadwal yang sering terjadi pada model AI yang mengira dirinya hidup di zona waktu UTC atau waktu Amerika Serikat.
2. **Pencarian Waktu Kilat (< 5ms)**:
   - Mesin pencari SQLite FTS5 dilengkapi filter waktu (misal: `--since 7d`, `--from YYYY-MM-DD`). Ini mencegah AI salah mengira kejadian 6 bulan lalu sebagai kejadian kemarin pagi (*anachronism*).

---

## 8.3 Format Komunikasi Khusus WhatsApp (WhatsApp Formatting Hygiene)

Model AI biasanya terbiasa menghasilkan tulisan Markdown untuk layar komputer (seperti tanda pagar `# Heading` atau tabel lebar bertanda `|`). Format ini hancur dan sangat jelek jika dibaca di layar kecil ponsel.

Aina menerapkan aturan format ramah WhatsApp secara ketat:
- **Tebal**: Menggunakan `*teks*` (bukan `**teks**`).
- **Miring**: Menggunakan `_teks_`.
- **Coret**: Menggunakan `~teks~`.
- **Daftar**: Menggunakan simbol titik atau bullet `•` (bukan tabel Markdown lebar).
- **Link**: Menuliskan URL secara langsung, bukan format `[nama](url)`.
- **Kode Panjang**: Jika kode program lebih dari 25 baris, kode disimpan ke file di sandbox dan hanya ringkasannya yang dikirimkan ke chat.

---

## 8.4 Ketahanan dan Penanganan Gangguan (Resilience & Error Handling)

1. **Pemulihan Aliran Putus (*Self-Healing Stream Fallback*)**: Jika koneksi streaming antara AI dan server terputus tiba-tiba, Aina tidak langsung menyerah. Sistem mendeteksi teks yang belum selesai dan meminta model menyambung bagian terakhirnya secara otomatis.
2. **Pembatasan Giliran Obrolan (*Session Turn Cap*)**: Sesi obrolan yang sudah mencapai 30 giliran otomatis dirangkum secara padat (*lean resumed prompt*) dan dialihkan ke sesi baru agar server tetap ringan dan respon tetap cepat.
3. **Pencatatan Berstruktur (*Structured Logging*)**: Seluruh peristiwa dicatat dengan `tracing` terstruktur sehingga pengembang dapat memantau kesehatan sistem dengan perintah `aina status` atau `aina audit presence`.
