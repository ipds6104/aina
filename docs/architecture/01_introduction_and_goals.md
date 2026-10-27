# 1. Pendahuluan dan Tujuan (Introduction and Goals)

> [!INFO] **Architecture Provenance & Freshness**
> - **Verified Commit**: `e9b88a2` (branch: `main`)
> - **Verified Timestamp**: `2026-10-26T11:19:12+07:00`
> - **Tracked Paths**: `src/`, `config/`, `skills/`
> - **Drift Status**: `verified`

Dokumen ini menjelaskan latar belakang, masalah nyata yang dihadapi, serta sasaran utama dari pembuatan **Aina**. Penjelasan disajikan dalam bahasa yang lugas dan mudah dipahami, tanpa istilah rumit yang membingungkan.

---

## 1.1 Gambaran Umum dan Masalah Nyata

### Apa itu Aina?
**Aina (あいな)** adalah asisten kecerdasan buatan (*AI Co-Worker*) yang berjalan di server mandiri (*self-hosted*), terhubung langsung ke **WhatsApp**, dan digerakkan oleh mesin agen pintar **Google Antigravity CLI (`agy`)**.

Aina **bukanlah chatbot kaku** seperti bot layanan pelanggan biasa yang hanya membalas dengan template atau teks tanya-jawab sederhana. Aina dirancang sebagai **rekan kerja teknis** yang dapat:
1. Membaca pesan obrolan dan memahami konteks percakapan di grup maupun jalur pribadi (DM).
2. Mengeksekusi perintah kerja nyata di komputer atau server (misalnya menjalankan skrip analisis, merapikan data, atau mengunduh berkas Google Drive).
3. Mengekstrak tabel rumit dari dokumen PDF dan gambar tanpa merusak susunan kolom dan baris.
4. Menjaga tata krama kerja di WhatsApp: tidak membocorkan obrolan pribadi ke grup, dan tidak membocorkan data rahasia kantor kepada orang asing.

### Masalah Nyata yang Diselesaikan
Dalam penggunaan asisten AI di lingkungan kantor atau tim kerja sehari-hari, ada beberapa kendala besar:

1. **Kendala Izin Birokrasi di Grup WhatsApp**:
   Memasukkan nomor bot baru ke dalam grup kantor sering kali ditolak admin atau membutuhkan surat izin resmi yang berbelit-belit karena kekhawatiran privasi.
   *Solusi Aina*: Aina menyediakan jalur ganda (*Dual-WhatsApp*). Selain nomor bot resmi, pengguna bisa menghubungkan nomor WhatsApp pribadinya sendiri sebagai "sensor pendengar". Aina ikut membaca grup yang sudah diikuti pengguna, mencatat konteks secara pasif, dan hanya menjawab jika dipanggil oleh pemiliknya (`!aina`). Tidak perlu menambah nomor baru ke grup.

2. **Kuota Akun AI Cepat Habis**:
   Menggunakan model AI pintar sepanjang hari sering kali terhenti di tengah jalan karena terkena batas pemakaian harian (*rate limit*).
   *Solusi Aina*: Aina mengumpulkan banyak akun Google dalam satu wadah (*pool*). Aina otomatis memutar penggunaan akun secara bergiliran. Jika satu akun kehabisan kuota, Aina otomatis berpindah ke akun berikutnya secara mulus tanpa membuat pengguna mengalami kegagalan.

3. **Bot Mengganggu di Grup Obrolan**:
   Bot biasa sering kali membalas setiap pesan sehingga menimbulkan kebisingan (*spam*). Sebaliknya, jika bot hanya menyala saat dipanggil, ia tidak tahu apa yang sedang dibahas sebelumnya.
   *Solusi Aina*: Aina memiliki aturan penjaga pintu (*gatekeeper*). Di grup, Aina menyimak obrolan dengan tenang tanpa membalas. Namun, obrolan tersebut dicatat dalam memori jangka pendek. Saat ada anggota yang memanggil `@Aina`, ia langsung paham riwayat pembicaraan terakhir tanpa perlu dijelaskan ulang dari awal.

4. **Kekhawatiran Kebocoran Data dan Password**:
   Kunci akses (API token) atau data internal kantor rawan bocor jika ditaruh secara sembarangan di dalam file program.
   *Solusi Aina*: Aina menggunakan kurir rahasia (*Secret Broker*). Kunci sandi hanya diserahkan saat sebuah alat dijalankan di balik layar, dan langsung dimusnahkan dari memori setelah selesai. Aina juga memiliki penyaring otomatis agar teks rahasia tidak pernah terkirim ke obrolan WhatsApp.

---

## 1.2 Sasaran Mutu (Quality Goals)

Sasaran mutu berikut dirumuskan secara terukur untuk menjamin bahwa Aina dapat diandalkan setiap hari sebagai rekan kerja nyata:

| No | Tag | Sasaran Mutu | Skenario Konkret & Ukuran Keberhasilan |
|:---|:---|:---|:---|
| 1 | #reliability | Ketersediaan Sistem & Rotasi Kuota Otomatis | Sistem mampu melayani percakapan 24/7 tanpa henti. Jika akun Google utama terkena batas kuota (kode 429), sistem otomatis beralih ke akun cadangan dalam waktu < 2 detik tanpa membuat pesan pengguna gagal. |
| 2 | #security | Perlindungan Privasi & Keamanan Kunci Sandi | 100% pesan pribadi (DM) di akun sensor tidak pernah dicatat atau dibocorkan ke grup. Seluruh kunci akses API disuntikkan secara dinamis saat eksekusi (*Just-in-Time*) dan disaring agar 0% token rahasia bocor ke teks balasan chat. |
| 3 | #maintainability | Kemudahan Perawatan & Pemisahan Keahlian | Kode inti aplikasi bersifat netral. Seluruh keahlian khusus kantor atau instansi dipisahkan ke folder tersendiri (`data/custom-skills`), sehingga penambahan fitur baru tidak merusak logika inti program. |
| 4 | #efficiency | Kecepatan Respon & Pencarian Memori Kilat | Pencarian riwayat percakapan masa lalu menggunakan database teks berkecepatan tinggi selesai dalam waktu < 5 milidetik, dengan filter rentang waktu presisi untuk mencegah salah ingat tanggal. |
| 5 | #usability | Kenyamanan Komunikasi & Interaksi Alami | Pesan balasan di WhatsApp diformat rapi sesuai tampilan layar ponsel (tanpa tanda pagar Markdown besar atau tabel lebar yang rusak), serta mampu meminta klarifikasi terarah jika instruksi multitafsir (*Tabayyun*). |

---

## 1.3 Pemangku Kepentingan (Stakeholders)

Tabel berikut memetakan pihak-pihak yang berinteraksi dengan Aina beserta harapan mereka terhadap sistem:

| Peran Pemangku Kepentingan | Harapan Utama terhadap Sistem | Bentuk Manfaat dari Dokumen Ini |
|:---|:---|:---|
| **Pengguna Pendamping (*User Companion*)** | Memiliki asisten pribadi yang cekatan, menjaga rahasia, dan bisa dihubungi kapan saja lewat WhatsApp. | Memahami cara kerja sensor akun pribadi, batas kewenangan Aina, dan cara mengendalikan sistem lewat perintah ringkas. |
| **Rekan Kerja di Grup (*Team Members*)** | Mendapatkan bantuan teknis cepat (olah data, cari dokumen, tanya prosedur) tanpa grup menjadi bising oleh balasan bot yang tidak relevan. | Mengetahui adab memanggil Aina di grup dan bagaimana Aina mengenali peran anggota tim secara otomatis. |
| **Pengembang & Pengelola Server (*Engineers / Ops*)** | Memasang dan merawat aplikasi dengan mudah di server (misalnya via Coolify atau Docker) dengan penggunaan memori yang sangat hemat. | Memahami arsitektur heksagonal, alur kerja komponen, jalur database SQLite, serta penanganan kunci rahasia. |
| **Pimpinan Tim / Penjamin Keamanan (*Auditors / SecOps*)** | Menjamin tidak ada data kantor yang bocor ke publik dan aktivitas asisten dapat diaudit secara transparan. | Meninjau batasan privasi antar sesi, jejak rekam tindakan (*audit log*), serta kepatuhan pemisahan data sensitif. |
