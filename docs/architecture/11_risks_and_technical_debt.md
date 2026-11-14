# 11. Risiko dan Hutang Teknis (Risks and Technical Debt)

> [!INFO] **Architecture Provenance & Freshness**
> - **Verified Commit**: `fb69f88` (branch: `main`)
> - **Verified Timestamp**: `2026-11-14T13:47:00+07:00`
> - **Tracked Paths**: `src/core/domain/audit/`, `src/core/usecases/process_message/`, `src/adapters/driven/sqlite_store/`
> - **Drift Status**: `verified`

Bab ini secara terbuka mendata potensi risiko yang mungkin timbul dari keputusan arsitektur (Bab 9), serta catatan hutang teknis (*technical debt*) yang diketahui agar dapat dimitigasi dan dirawat dengan baik.

---

## 11.1 Matriks Risiko Arsitektur (Architecture Risk Matrix)

Tabel berikut merangkum seluruh risiko yang muncul dari keputusan arsitektur (ADR-001 hingga ADR-008) beserta rencana mitigasinya:

| ID Risiko | Deskripsi Risiko Nyata | Probabilitas | Dampak | Strategi Mitigasi & Pengamanan |
|:---|:---|:---:|:---:|:---|
| RISK-01 | Kompleksitas pemetaan tipe data antara lapisan inti (*Core Ports*) dan lapisan luar (*Adapters*). | Sedang | Rendah | Menggunakan trait yang jelas di `src/core/ports/` dan penanganan error terstruktur (`anyhow::Result`) tanpa unwrap di jalur produksi. |
| RISK-02 | Akun WhatsApp sensor bayangan (*Companion*) tiba-tiba terputus (*logout*) jika pengguna mencabut perangkat tertaut di aplikasi HP. | Sedang | Sedang | Endpoint `/api/status` dan perintah `aina whatsapp status` otomatis memantau koneksi soket; sistem mengirim peringatan jika sesi terputus. |
| RISK-03 | Token autentikasi OAuth Google pada salah satu akun di kolam kedaluwarsa atau dicabut pemiliknya. | Rendah | Sedang | Sistem Auto-Failover otomatis mengabaikan akun yang rusak dan melompat ke akun berikutnya, serta menyediakan wizard login web cepat di `/setup`. |
| RISK-04 | Repositori Git eksternal untuk keahlian khusus kantor mengalami perubahan mendadak atau tidak dapat diunduh. | Rendah | Rendah | Folder `data/custom-skills` disimpan di volume permanen lokal; kegagalan pembaruan jaringan tidak merusak keahlian lokal yang sudah terpasang. |
| RISK-05 | Layanan brankas rahasia eksternal (misal server Infisical atau HashiCorp Vault) mengalami gangguan koneksi (*timeout*). | Rendah | Tinggi | Secret Broker menyediakan cadangan failover otomatis ke file lokal terenkripsi (`.secrets.env`, izin chmod 600) untuk menjaga kelangsungan kerja. |
| RISK-06 | Terjadi duplikasi tugas terjadwal saat proses rekonsiliasi startup berulang kali. | Rendah | Sedang | Skema SQLite dilindungi *partial unique index* `idx_scheduled_tasks_slug` pada kolom `slug`, sehingga operasi penambahan bersifat idempoten (*UPSERT*). |
| RISK-07 | Terjadinya *context rot* atau perhatian LLM terdistraksi akibat katalog keahlian yang semakin banyak. | Rendah | Sedang | Penerapan model *Three-Tier Progressive Disclosure* (`agentskills.io`): hanya nama dan deskripsi ringkas yang dimuat di Tier 1; detail lengkap hanya dibaca saat cocok. |
| RISK-08 | Layanan API cuaca publik (Open-Meteo) tidak dapat dihubungi atau offline saat sesi refleksi berjalan. | Rendah | Rendah | Modul `reflect.py` menyediakan fallback offline estimasi cuaca tropis khatulistiwa otomatis berdasarkan waktu WIB agar proses refleksi tidak pernah macet (*fail-safe*). |
| RISK-09 | Kebocoran nomor WhatsApp bot ke publik atau dimasukkan ke grup pihak asing tanpa izin. | Sedang | Sedang | Paradigma Zero Implicit Trust otomatis mengisolasi kontak tak dikenal ke status 'GUEST' tanpa hak baca data internal, rate-limiting, dan honeypot logging. |
| RISK-10 | Percobaan modifikasi langsung (*tampering*) pada tabel log audit oleh pihak internal/penyerang. | Rendah | Tinggi | Rantai hash SHA-256 (NIST AU-10 & ISO 42001): setiap blok mengikat hash sebelumnya. Verifikasi via `aina audit verify-chain` otomatis mendeteksi baris yang termanipulasi. |
| RISK-11 | Tuntutan hukum privasi UU PDP No. 27/2022 akibat penyimpanan preferensi personal lawan bicara. | Rendah | Sedang | Fitur mandiri `/profilku` (transparansi profil) dan `/lupakansaya` (pembersihan seketika dari SQLite), dengan pemisahan log audit legal forensik. |

---

## 11.2 Catatan Hutang Teknis (Technical Debt Ledger)

Hutang teknis adalah kompromi praktis yang diambil saat pengembangan agar aplikasi dapat segera digunakan, namun perlu dirapikan di masa depan:

| ID Hutang | Area Modul | Deskripsi Kompromi Saat Ini | Dampak Saat Ini | Rencana Perbaikan (Remediation) |
|:---|:---|:---|:---|:---|
| **DEBT-01** | `sqlite_store` & `activity` | Beberapa fungsi pembantu (`is_user_in_group`, `get_group_members`, `to_activity`, `simple`) memicu compiler warning *unused* karena disiapkan untuk fungsionalitas lanjutan. | Tidak ada dampak fungsional runtime (hanya peringatan kompilasi). | Menghubungkan fungsi tersebut ke perintah CLI audit keanggotaan grup atau menambahkan anotasi `#[allow(dead_code)]`. |
| **DEBT-02** | `skill_scaffolder.py` | Skrip instalasi keahlian kustom dari Git masih menggunakan pemanggilan subprocess `git clone` sistem lokal. | Memerlukan utilitas biner `git` terpasang di host/kontainer. | Biner git sudah terpasang di Dockerfile bawaan; ke depan dapat dipertimbangkan pustaka libgit bawaan bila diperlukan eksekusi tanpa git CLI. |
| **DEBT-03** | `sanitizer.rs` | Pembersihan teks rahasia balasan chat mengandalkan pola pencocokan ekspresi reguler (*regex patterns*). | Jika ada pola token baru yang aneh, ada kemungkinan kecil pola belum terdeteksi. | Menambahkan tes otomatis berkala (*fuzz testing*) untuk mendeteksi variasi format kunci rahasia baru. |
| **DEBT-04** | `activity_reflection` | Skala katalog skill saat ini (~16 skill) menggunakan *flat index* metadata (~700 token). | Jika library skill membengkak hingga $\ge 50$ skill di masa depan, Tier 1 prompt dapat membesar. | Menyiapkan layer *Vector Tool Retrieval* lokal menggunakan ekstensi vector SQLite saat jumlah skill mendekati batas 50 skill. |
