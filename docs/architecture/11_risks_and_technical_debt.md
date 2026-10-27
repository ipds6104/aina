# 11. Risiko dan Hutang Teknis (Risks and Technical Debt)

> [!INFO] **Architecture Provenance & Freshness**
> - **Verified Commit**: `e9b88a2` (branch: `main`)
> - **Verified Timestamp**: `2026-10-26T11:19:12+07:00`
> - **Tracked Paths**: `src/`, `config/`, `skills/`
> - **Drift Status**: `verified`

Bab ini secara terbuka mendata potensi risiko yang mungkin timbul dari keputusan arsitektur (Bab 9), serta catatan hutang teknis (*technical debt*) yang diketahui agar dapat dimitigasi dan dirawat dengan baik.

---

## 11.1 Matriks Risiko Arsitektur (Architecture Risk Matrix)

Tabel berikut merangkum seluruh risiko yang muncul dari keputusan arsitektur (ADR-001 hingga ADR-005) beserta rencana mitigasinya:

| ID Risiko | Deskripsi Risiko Nyata | Probabilitas | Dampak | Strategi Mitigasi & Pengamanan |
|:---|:---|:---:|:---:|:---|
| RISK-01 | Kompleksitas pemetaan tipe data antara lapisan inti (*Core Ports*) dan lapisan luar (*Adapters*). | Sedang | Rendah | Menggunakan trait yang jelas di `src/core/ports/` dan penanganan error terstruktur (`anyhow::Result`) tanpa unwrap di jalur produksi. |
| RISK-02 | Akun WhatsApp sensor bayangan (*Companion*) tiba-tiba terputus (*logout*) jika pengguna mencabut perangkat tertaut di aplikasi HP. | Sedang | Sedang | Endpoint `/api/status` dan perintah `aina whatsapp status` otomatis memantau koneksi soket; sistem mengirim peringatan jika sesi terputus. |
| RISK-03 | Token autentikasi OAuth Google pada salah satu akun di kolam kedaluwarsa atau dicabut pemiliknya. | Rendah | Sedang | Sistem Auto-Failover otomatis mengabaikan akun yang rusak dan melompat ke akun berikutnya, serta menyediakan wizard login web cepat di `/setup`. |
| RISK-04 | Repositori Git eksternal untuk keahlian khusus kantor mengalami perubahan mendadak atau tidak dapat diunduh. | Rendah | Rendah | Folder `data/custom-skills` disimpan di volume permanen lokal; kegagalan pembaruan jaringan tidak merusak keahlian lokal yang sudah terpasang. |
| RISK-05 | Layanan brankas rahasia eksternal (misal server Infisical atau HashiCorp Vault) mengalami gangguan koneksi (*timeout*). | Rendah | Tinggi | Secret Broker menyediakan cadangan failover otomatis ke file lokal terenkripsi (`.secrets.env`, izin chmod 600) untuk menjaga kelangsungan kerja. |

---

## 11.2 Catatan Hutang Teknis (Technical Debt Ledger)

Hutang teknis adalah kompromi praktis yang diambil saat pengembangan agar aplikasi dapat segera digunakan, namun perlu dirapikan di masa depan:

| ID Hutang | Area Modul | Deskripsi Kompromi Saat Ini | Dampak Saat Ini | Rencana Perbaikan (Remediation) |
|:---|:---|:---|:---|:---|
| **DEBT-01** | `sqlite_store` | Fungsi `is_user_in_group` dan `get_group_members` pada port session store masih memicu compiler warning dead code karena baru disiapkan untuk audit masa depan. | Tidak ada dampak fungsional (hanya peringatan saat kompilasi). | Menghubungkan fungsi tersebut ke perintah CLI audit keanggotaan grup atau menghapus atribut jika tidak digunakan. |
| **DEBT-02** | `skill_scaffolder.py` | Skrip instalasi keahlian kustom dari Git masih menggunakan pemanggilan subprocess `git clone` sistem lokal. | Memerlukan utilitas biner `git` terpasang di host/kontainer. | Biner git sudah terpasang di Dockerfile bawaan; ke depan dapat dipertimbangkan pustaka libgit bawaan bila diperlukan eksekusi tanpa git CLI. |
| **DEBT-03** | `sanitizer.rs` | Pembersihan teks rahasia balasan chat mengandalkan pola pencocokan ekspresi reguler (*regex patterns*). | Jika ada pola token baru yang aneh, ada kemungkinan kecil pola belum terdeteksi. | Menambahkan tes otomatis berkala (*fuzz testing*) untuk mendeteksi variasi format kunci rahasia baru. |
