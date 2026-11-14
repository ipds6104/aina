# 12. Glosarium Istilah (Glossary)

> [!INFO] **Architecture Provenance & Freshness**
> - **Verified Commit**: `f1ae07a` (branch: `main`)
> - **Verified Timestamp**: `2026-11-14T14:14:27+07:00`
> - **Tracked Paths**: `src/core/domain/`, `src/core/ports/`, `src/adapters/`
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
| **Aktivitas Bawaan (*Baseline Activity*)** | Tugas terjadwal bawaan pabrik yang didefinisikan secara deklaratif di `config/activities.default.json` (seperti briefing harian, status WhatsApp story, dan pengingat presensi). |
| **Aktivitas Kustom (*Custom Activity*)** | Tugas terjadwal yang dibuat atau dimodifikasi oleh pengguna dan disimpan di direktori persisten `/app/data/activities/<slug>.json`. |
| **Rekonsiliasi Aktivitas (*Activity Reconciler*)** | Proses penyelarasan idempoten saat booting yang menggabungkan baseline default pabrik dengan modifikasi persisten pengguna tanpa pernah menghapus jadwal yang telah diubah. |
| **Penyingkapan Bertingkat (*Progressive Disclosure*)** | Model arsitektur pemuatan keahlian 3-tahap (Discovery $\rightarrow$ Activation $\rightarrow$ Execution) sesuai standar `agentskills.io` untuk mencegah pemborosan token dan *context rot*. |
| **Keahlian Khusus (*Custom Skills*)** | Alat bantu atau prosedur kerja tambahan (misal pengolah data kantor) yang disimpan di folder terpisah (`data/custom-skills`) dan bisa diunduh dari Git. |
| **Skor Kebosanan (*Boredom Score*)** | Metrik kuantitatif (0.0 hingga 1.0) yang mengukur tingkat pengulangan tema adegan dan busana dalam status harian Aina untuk memicu variasi baru. |
| **Impact Maxxing** | Filosofi hidup Aina yang berupaya menghadirkan dampak positif, ketenangan, optimisme, dan rasa syukur bagi rekan kerja tanpa bersikap menggurui. |
| **Riset Web Terarah (*Gated Novelty Search*)** | Protokol pencarian internet terarah (maksimal 1–2 query) saat sesi refleksi untuk memverifikasi cuaca atau mencari inspirasi lokal tanpa halusinasi. |
| **Tabayyun** | Sikap aktif Aina untuk bertanya dan meminta klarifikasi terarah kepada pengguna jika instruksi yang diberikan masih ambigu atau tidak lengkap. |
| **Tawaqquf** | Sikap menahan diri untuk tidak membagikan data kantor atau dokumen rahasia kepada orang asing di grup sebelum mendapat persetujuan pemilik bot. |
| **SQLite FTS5** | Teknologi database lokal berkecepatan tinggi yang mampu mencari kata di puluhan ribu riwayat obrolan dalam waktu hitungan milidetik. |
| **TurnCapability** | Tiket otorisasi berumur pendek (TTL 10 menit) yang mengikat nomor pengirim pesan asli dan tingkat wewenangnya ke setiap pemanggilan tool oleh AI. |
| **Non-Repudiation (Anti-Sangkalan)** | Jaminan hukum dan matematis bahwa suatu aksi tool yang dilakukan AI tidak dapat disangkal oleh pihak yang memerintahkannya (NIST SP 800-53 AU-10). |
| **Rantai Hash SHA-256 (*Hash Chaining*)** | Metode penguncian log audit bergaya blockchain di mana setiap blok memuat hash dari blok sebelumnya, sehingga manipulasi baris data lama akan mematahkan seluruh rantai hash. |
| **Zero Implicit Trust** | Doktrin keamanan di mana tidak ada nomor WhatsApp atau keanggotaan grup yang dipercaya secara bawaan tanpa verifikasi eksplisit oleh Companion. |
| **Hak untuk Dilupakan (*Right to be Forgotten*)** | Hak subjek data untuk menghapus profil personal dan preferensi komunikasinya dari memori sistem AI sesuai UU PDP No. 27/2022 dan GDPR Article 17. |
| **Penyensoran PII (*PII Masking*)** | Teknik penyembunyian otomatis nomor telepon pihak ketiga (misal `62812****7890`) saat bot berinteraksi dengan lawan bicara berstatus tamu (*guest*). |
| **Profiling Semantik (*Semantic Profiling*)** | Model penyimpanan memori lawan bicara 4-dimensi (`communication_prefs`, `active_projects`, `durable_constraints`, `confidence_score`) untuk mempersonalisasi interaksi secara terukur. |
