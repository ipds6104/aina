# 12. Glosarium Istilah (Glossary)

> [!INFO] **Architecture Provenance & Freshness**
> - **Verified Commit**: `e9b88a2` (branch: `main`)
> - **Verified Timestamp**: `2026-10-26T11:19:12+07:00`
> - **Tracked Paths**: `src/`, `config/`, `skills/`
> - **Drift Status**: `verified`

Bab ini menyajikan daftar istilah penting yang digunakan di seluruh sistem dan dokumentasi Aina, dijelaskan dalam bahasa sehari-hari yang mudah dipahami.

---

| Istilah / Kata Kunci | Penjelasan Sederhana & Perannya dalam Sistem |
|:---|:---|
| **Aina (あいな)** | Asisten AI mandiri (*AI Co-Worker*) yang terhubung ke WhatsApp dan mampu mengeksekusi tugas nyata di komputer/server. |
| **Antigravity CLI (`agy`)** | Mesin agen pintar resmi dari Google yang bertindak sebagai "otak" pemikir dan pengambil keputusan bagi Aina. |
| **Whatsmeow** | Program gerbang mandiri yang menghubungkan aplikasi Aina ke jaringan resmi WhatsApp tanpa menggunakan browser berat seperti Puppeteer. |
| **Pengguna Pendamping (*User Companion*)** | Pemilik sah Aina yang menjadi sponsor utama, pemegang hak akses tertinggi (`admin`), dan pihak yang dimintai izin jika ada hal sensitif. |
| **Sensor Bayangan (*Shadow Sensor*)** | Pemanfaatan akun WhatsApp pribadi pengguna untuk membaca obrolan grup kantor secara pasif tanpa perlu mengundang nomor bot baru. |
| **Penjaga Pintu (*Gatekeeper*)** | Aturan cerdas di dalam Aina yang menyaring pesan masuk: membalas pesan pribadi, namun hanya menyimak diam-diam di grup kecuali namanya dipanggil. |
| **Konteks Berjalan (*Rolling Context*)** | Riwayat 10–20 obrolan terakhir di grup yang otomatis dibaca oleh Aina saat namanya di-tag, agar ia langsung paham apa yang sedang dibicarakan. |
| **Kolam Akun (*Account Pool*)** | Kumpulan beberapa akun Google yang digunakan bergantian agar kuota pemakaian AI harian tidak cepat habis. |
| **Rotasi Berputar (*Round-Robin*)** | Cara membagi tugas dengan menggilir akun Google satu per satu secara berurutan pada setiap sesi obrolan baru. |
| **Peralihan Otomatis (*Auto-Failover*)** | Kemampuan Aina untuk langsung melompat ke akun cadangan jika akun yang sedang dipakai mendadak terkena batas kuota (error 429). |
| **Batas Giliran Sesi (*Turn Cap*)** | Batas maksimal tanya-jawab dalam satu sesi obrolan (misal 30 kali) untuk mencegah memori membengkak dan respon menjadi lemot. |
| **Rangkuman Padat (*Lean Resumed Prompt*)** | Ringkasan sari pati obrolan lama yang dibuat secara otomatis saat membuka sesi baru, sehingga AI tetap ingat kesepakatan sebelumnya dengan hemat token. |
| **Kurir Rahasia (*Secret Broker*)** | Sistem perantara yang bertugas mengambil kunci API atau sandi dari brankas aman tepat saat dibutuhkan, tanpa pernah menyimpannya di file teks. |
| **Penyuntikan Tepat Waktu (*Just-in-Time / JIT Injection*)** | Metode menyerahkan password ke proses komputasi hanya selama proses itu berjalan, lalu langsung dimusnahkan saat selesai. |
| **Keahlian Khusus (*Custom Skills*)** | Alat bantu atau prosedur kerja tambahan (misal pengolah data kantor) yang disimpan di folder terpisah (`data/custom-skills`) dan bisa diunduh dari Git. |
| **Tabayyun** | Sikap aktif Aina untuk bertanya dan meminta klarifikasi terarah kepada pengguna jika instruksi yang diberikan masih ambigu atau tidak lengkap. |
| **Tawaqquf** | Sikap menahan diri untuk tidak membagikan data kantor atau dokumen rahasia kepada orang asing di grup sebelum mendapat persetujuan pemilik bot. |
| **SQLite FTS5** | Teknologi database lokal berkecepatan tinggi yang mampu mencari kata di puluhan ribu riwayat obrolan dalam waktu hitungan milidetik. |
