# 10. Persyaratan Mutu (Quality Requirements)

> [!INFO] **Architecture Provenance & Freshness**
> - **Verified Commit**: `c477616` (branch: `main`)
> - **Verified Timestamp**: `2026-11-17T19:05:39+07:00`
> - **Tracked Paths**: `src/core/domain/audit/`, `src/core/usecases/process_message/`, `src/adapters/driven/sqlite_store/`
> - **Drift Status**: `verified`

Bab ini memuat skenario-skenario mutu terukur untuk menguji dan membuktikan bahwa sasaran kualitas yang ditetapkan pada Bab 1 benar-benar terpenuhi secara nyata.

---

## 10.1 Pohon Mutu (Quality Tree)

Hubungan antara sasaran mutu utama dan skenario pengujian dipetakan dalam pohon mutu berikut:

- Kualitas Sistem Aina
  ├── Ketersediaan & Keandalan
  │   ├── **QS-01**: Rotasi kuota otomatis saat batas harian 429 tercapai (#reliability)
  │   ├── **QS-12**: Ketahanan Konkurensi 100 Obrolan Serentak & Zero-429 (#reliability, #efficiency)
  │   └── **QS-15**: Pertahanan Kuota & Anti-Ban WhatsApp pada Banjir DM Tamu *(Aspirational — Fase 19)* (#reliability, #security)
  ├── Privasi & Keamanan
  │   ├── **QS-02**: Nol kebocoran pesan pribadi dan penyuntikan kunci JIT (#security)
  │   ├── **QS-08**: Integritas kriptografis rantai audit tool SHA-256 (#security)
  │   ├── **QS-09**: Pertahanan Zero Implicit Trust saat nomor WA bocor (#security)
  │   ├── **QS-10**: Hak penghapusan mandiri subjek data sesuai UU PDP (#security, #usability)
  │   ├── **QS-11**: Pertahanan Anti-Impersonasi & Integritas Identitas 3-Tingkat (#security)
  │   ├── **QS-13**: Otorisasi Web Fail-Closed, Header-Only Admin API, & Webhook SSRF/Path Lock *(Aspirational — Fase 17)* (#security)
  │   ├── **QS-14**: Canonical Path Containment & Zero Hardcoded Secrets Env-First *(Aspirational — Fase 18)* (#security, #maintainability)
  │   └── **QS-16**: Isolasi Nonce XML, Deterministic Target Binding, & Least-Privilege Subprocess Env *(Aspirational — Fase 20)* (#security)
  ├── Kemudahan Perawatan & Modul
  │   ├── **QS-03**: Pemasangan skill Git kustom tanpa menyentuh kode inti (#maintainability)
  │   └── **QS-06**: Ketahanan modifikasi jadwal saat redeploy kontainer (#maintainability)
  ├── Efisiensi & Kecepatan Respon
  │   └── **QS-04**: Pencarian memori riwayat chat selesai dalam < 5 milidetik (#efficiency)
  ├── Kenyamanan & Tata Krama Pengguna
  │   └── **QS-05**: Format teks ramah ponsel dan klarifikasi terarah (#usability)
  └── Adaptabilitas & Grounding Nyata
      └── **QS-07**: Evaluasi kejenuhan tempo harian dan grounding cuaca faktual (#adaptability)

---

## 10.2 Skenario Mutu Terukur (Quality Scenarios)

### QS-01: Rotasi Kuota Otomatis saat Terkena Limit 429
- **Kategori Mutu**: #reliability
- **Kondisi Awal**: Sistem berjalan dengan 3 akun Google yang terdaftar di Account Pool. Akun pertama telah mencapai batas kuota harian.
- **Pemicu**: Pengguna mengirim pesan pertanyaan baru di grup WhatsApp.
- **Respon Sistem**: `AntigravityCliAdapter` mendeteksi error HTTP 429 dari akun pertama, mencatat status limit, dan otomatis memindahkan sesi ke akun kedua dalam waktu < 2 detik.
- **Ukuran Keberhasilan**: Pengguna menerima jawaban lengkap di WhatsApp tanpa melihat pesan gagal (*zero failure rate*).

---

### QS-02: Perlindungan Privasi Pesan Pribadi Sensor Bayangan
- **Kategori Mutu**: #security
- **Kondisi Awal**: Sesi sensor bayangan (*User Companion*) aktif terhubung ke akun WhatsApp pribadi pengguna.
- **Pemicu**: Teman atau keluarga mengirim pesan pribadi (DM) ke akun WhatsApp pengguna.
- **Respon Sistem**: Logika `Gatekeeper` langsung mengabaikan pesan tersebut (100% diabaikan). Pesan tidak disimpan ke database dan tidak pernah diteruskan ke model AI.
- **Ukuran Keberhasilan**: 0% obrolan pribadi non-bot masuk ke database SQLite atau bocor ke ruang publik.

---

### QS-03: Pemasangan Skill Eksternal Tanpa Mengubah Kode Inti
- **Kategori Mutu**: #maintainability
- **Kondisi Awal**: Pengguna ingin menambahkan alat pengolah data khusus kantor yang disimpan di repositori Git privat.
- **Pemicu**: Pengguna menjalankan perintah `aina skill install https://github.com/kantor/skill-data.git`.
- **Respon Sistem**: Sistem mengunduh repositori ke folder `data/custom-skills/`, memvalidasi file `SKILL.md`, dan langsung mendaftarkan alat tersebut ke dalam katalog agen.
- **Ukuran Keberhasilan**: Fitur baru langsung dapat digunakan dalam obrolan tanpa perlu kompilasi ulang kode Rust inti ataupun me-restart kontainer.

---

### QS-04: Kecepatan Pencarian Riwayat Obrolan Masa Lalu
- **Kategori Mutu**: #efficiency
- **Kondisi Awal**: Database SQLite telah menampung lebih dari 50.000 pesan obrolan grup selama 6 bulan terakhir.
- **Pemicu**: Pengguna meminta Aina mengingat notulen rapat minggu lalu: `aina archive search "rapat evaluasi" --since 7d`.
- **Respon Sistem**: Mesin pencari SQLite FTS5 melakukan pencarian indeks teks penuh dan penyaringan tanggal secara lokal.
- **Ukuran Keberhasilan**: Hasil pencarian ditemukan dan disajikan dalam waktu < 5 milidetik dengan penggunaan CPU < 5%.

---

### QS-05: Format Teks Sesuai Layar Ponsel & Verifikasi Tabayyun
- **Kategori Mutu**: #usability
- **Kondisi Awal**: Pengguna meminta ringkasan perbandingan data yang biasanya disajikan dalam bentuk tabel besar.
- **Pemicu**: Pengguna mengirim instruksi ringkas di WhatsApp.
- **Respon Sistem**: Aina menyusun balasan menggunakan poin-poin tebal bertitik (`•`), huruf tebal native WhatsApp (`*tebal*`), dan menghindari tabel Markdown lebar. Bila instruksi kurang jelas, Aina terlebih dahulu menanyakan maksud pengguna dengan sopan (*Tabayyun*).
- **Ukuran Keberhasilan**: Teks balasan nyaman dibaca di layar ponsel tanpa terpotong horizontal, dan tidak ada kesalahpahaman instruksi.

---

### QS-06: Ketahanan Modifikasi Jadwal Aktivitas Saat Redeploy Kontainer
- **Kategori Mutu**: #maintainability
- **Kondisi Awal**: Pengguna memodifikasi jam tayang briefing pagi atau teks status senja via `aina activity edit status_senja --time "17:45" --file`.
- **Pemicu**: Admin melakukan pembaruan kode Git yang memicu penghancuran dan pembuatan ulang kontainer Docker di Coolify.
- **Respon Sistem**: Saat kontainer baru booting, `ActivityReconciler` memuat berkas override dari `/app/data/activities/status_senja.json` dan memprioritaskannya di atas baseline default.
- **Ukuran Keberhasilan**: 100% pengaturan khusus pengguna tetap aktif dan tidak kembali ke setelan pabrik setelah redeployment selesai.

---

### QS-07: Evaluasi Kejenuhan Tempo Harian & Grounding Cuaca Faktual
- **Kategori Mutu**: #adaptability
- **Kondisi Awal**: Aina telah mempublikasikan tema status dan mengenakan busana yang sama selama 3 hari berturut-turut.
- **Pemicu**: Sesi evaluasi harian berjalan via `python3 skills/activity-reflection/scripts/reflect.py evaluate`.
- **Respon Sistem**: Mesin mendeteksi skor kebosanan $\ge 0.5$, mengambil data cuaca riil Pontianak via API Open-Meteo, merumuskan rekomendasi kuliner/tempat baru, serta memodifikasi instruksi status berikutnya.
- **Ukuran Keberhasilan**: Status WhatsApp berikutnya menampilkan variasi busana dan kuliner yang serasi dengan cuaca nyata tanpa ada halusinasi konteks lokal.

---

### QS-08: Integritas Kriptografis Rantai Audit Tool SHA-256 (NIST AU-10)
- **Kategori Mutu**: #security
- **Kondisi Awal**: Sistem mencatat puluhan pemanggilan tool di basis data `agent_tool_executions`.
- **Pemicu**: Administrator menjalankan audit verifikasi: `aina audit verify-chain` (atau serangan injeksi langsung memanipulasi baris SQLite).
- **Respon Sistem**: Fungsi `verify_audit_hash_chain` menghitung ulang seluruh rantai hash kriptografis SHA-256 dari genesis block hingga baris mutakhir. Jika ada 1 karakter payload atau sender yang diubah, sistem langsung mendeteksi kerusakan integritas dan menunjukkan nomor ID baris yang termanipulasi.
- **Ukuran Keberhasilan**: 100% deteksi manipulasi log, 0% celah modifikasi data audit tanpa terdeteksi (Terverifikasi di unit test `test_tamper_evident_tool_execution_hash_chain`).

---

### QS-09: Pertahanan Zero Implicit Trust saat Nomor WA Bocor ke Pihak Asing
- **Kategori Mutu**: #security
- **Kondisi Awal**: Nomor bot WhatsApp dihubungi oleh nomor asing yang belum terdaftar, atau dimasukkan ke grup publik tanpa izin Companion.
- **Pemicu**: Pihak asing meminta dokumen statistik internal satker atau mencoba melakukan jailbreak prompt.
- **Respon Sistem**: Logika `ContextPreparer` secara otomatis mengalokasikan wewenang `GUEST`, menerapkan pagar kognitif *Strict OpSec*, menolak memberikan data rahasia tanpa izin Companion, dan mencatat upaya tersebut di audit log.
- **Ukuran Keberhasilan**: 0% data internal kantor bocor ke pihak asing, penolakan disampaikan secara santun dan profesional.

---

### QS-10: Hak Penghapusan Mandiri Subjek Data Sesuai UU PDP (Right to be Forgotten)
- **Kategori Mutu**: #security, #usability
- **Kondisi Awal**: Pengguna WhatsApp telah berinteraksi dengan bot dan memiliki profil preferensi serta catatan proyek di basis data Aina.
- **Pemicu**: Pengguna mengirimkan perintah `/lupakansaya` (atau `!aina lupakan saya`).
- **Respon Sistem**: Sistem mengeksekusi `delete_user_profile(sender_jid)`, membersihkan seluruh catatan preferensi komunikasi dan nama panggilan dari tabel `user_profiles`, sembari tetap mempertahankan catatan audit legal di tabel forensik terpisah.
- **Ukuran Keberhasilan**: Data profil personal langsung terhapus 100% (Terverifikasi di unit test `user_can_view_and_clear_own_profile`), kepatuhan penuh UU PDP No. 27/2022.

---

### QS-11: Pertahanan Anti-Impersonasi & Integritas Identitas 3-Tingkat
- **Kategori Mutu**: #security
- **Skenario Pengujian**:
  Nomor asing mengirim pesan mengaku sebagai admin atau kepala satker.
- **Ukuran Keberhasilan**:
  100% klaim nama terlindungi dicatat di `claimed_name` dan status tetap `UNCONFIRMED`. Evaluasi `is_atasan` wajib bernilai `false` dan akses data internal tetap terkunci.

---

### QS-12: Ketahanan Konkurensi 100 Obrolan Serentak & Zero-429
- **Kategori Mutu**: #reliability, #efficiency
- **Skenario Pengujian**:
  100 pesan webhook diterima secara serentak dalam rentang waktu 5 detik.
- **Ukuran Keberhasilan**:
  Maksimal 8 agen aktif simultan, beban per akun Google <= 2 in-flight, 0% insiden server OOM, dan 0% error HTTP 429. Seluruh 100 pesan terlayani secara tertib.

---

### QS-13: Otorisasi Web Fail-Closed, Header-Only Admin API, & Webhook SSRF/Path Lock *(Aspirational — Fase 17)*
- **Kategori Mutu**: #security
- **Kondisi Awal**: Deployment baru berjalan tanpa `ADMIN_KEY` (atau `ADMIN_KEY=""`) serta eksposur endpoint web & `/webhook`.
- **Pemicu**: Pihak tidak terautentikasi mengakses `/api/setup`, `/api/simulate`, `/api/audit/*`, `/api/schedule/*`, mengirim `?key=...`, melakukan brute force `/api/auth/verify`, atau mengirim `download_url` eksternal & `id: "../../evil"` ke `/webhook`.
- **Respon Sistem**: Seluruh endpoint admin/diagnostik menolak dengan HTTP 401 (fail-closed + `constant_time_eq`), query `?key=` ditolak di admin API, percobaan ke-6 `/api/auth/verify` diblokir HTTP 429, dan `/webhook` memblokir SSRF serta menyanitasi `msg.id`.
- **Ukuran Keberhasilan**: 0% akses admin/diagnostik tanpa header/sesi OIDC sah, 0% request keluar ke host SSRF, dan 0% penulisan media di luar `workspace/media/`.

---

### QS-14: Canonical Path Containment & Zero Hardcoded Secrets Env-First *(Aspirational — Fase 18)*
- **Kategori Mutu**: #security, #maintainability
- **Kondisi Awal**: Repositori upstream dijalankan di lingkungan lokal/Docker tanpa layanan Vault eksternal wajib.
- **Pemicu**: Payload `POST /api/auth/oauth/exchange` mengirim `session_id: "../../evil"`, atau pemindaian statis memeriksa `scripts/` dan `skills/infisical/scripts/secret_tool.py`.
- **Respon Sistem**: Validasi ganda Rust & Python (`^[a-zA-Z0-9_-]{16,64}$` + canonical check) langsung mengembalikan HTTP 400 tanpa menulis/menghapus file; seluruh kredensial OAuth & Vault dibaca murni dari env var standar tanpa hardcoded plaintext maupun XOR.
- **Ukuran Keberhasilan**: 0% path escape pada `oauth_sessions`, 0 hardcoded credentials/domain privat di source Git, dan 0 dependensi Vault wajib.

---

### QS-15: Pertahanan Kuota & Anti-Ban WhatsApp pada Banjir DM Tamu *(Aspirational — Fase 19)*
- **Kategori Mutu**: #reliability, #security
- **Kondisi Awal**: Nomor asing berstatus `Authority::Guest` mengirim banjir 50 pesan DM (termasuk media) dalam 10 detik.
- **Pemicu**: Webhook menerima burst masif dari JID tamu yang sama sementara Admin juga aktif berinteraksi.
- **Respon Sistem**: Edge burst guard menahan flood ekstrem sebelum unduh media/antrean; pure domain `TokenBucket` mengizinkan maks 3 pesan, mengirim maks 1x balasan cooldown sopan per window, lalu melakukan silent auto-drop + tombstone sementara, dicatat di `whatsapp_action_audits`; Admin tetap dilayani penuh.
- **Ukuran Keberhasilan**: 0% kebocoran kuota `AccountPool` (429) akibat spam tamu, maks 1 balasan cooldown keluar per window (anti-ban WA), dan 0 impor `tokio`/I/O di `src/core/domain/rate_limiter.rs`.

---

### QS-16: Isolasi Nonce XML, Deterministic Target Binding, & Least-Privilege Subprocess Env *(Aspirational — Fase 20)*
- **Kategori Mutu**: #security
- **Kondisi Awal**: Tamu di grup mengirim payload indirect prompt injection (termasuk usaha XML breakout `</untrusted_ambient_history>`) untuk memicu `aina policy grant-user <attacker_jid>`, lalu Admin memanggil `@Aina` dengan pertanyaan netral.
- **Pemicu**: `ContextPreparer` merakit prompt dan menerbitkan `TurnCapability`; agen dieksekusi di `AntigravityCliAdapter`.
- **Respon Sistem**: Seluruh konteks untrusted (`ambient`, `recall`, `quote`, `sender.name`) terbungkus nonce XML acak + ter-escape; `capability::evaluate` menolak `GrantUser`/`RevokeUser` di grup untuk target yang tidak ada di `explicit_targets` pesan aktif Admin; subprocess `agy` pada turn `Guest` berjalan tanpa env `WHATSMEOW_API_KEY`/`WHATSMEOW_COMPANION_KEY`.
- **Ukuran Keberhasilan**: 100% penolakan deterministik eskalasi akses dari konteks ambien grup dan 0% paparan kredensial gateway pada proses anak turn `Guest`.

---

## 10.3 Matriks Ringkasan Skenario Mutu

| ID Skenario | Sasaran Mutu | Target Ukuran | Kondisi Terverifikasi Saat Ini |
|:---|:---|:---|:---|
| **QS-01** | #reliability | Waktu failover < 2 detik, 0 kegagalan pesan | Lulus (Terverifikasi di unit test `account_pool.rs`) |
| **QS-02** | #security | 100% pesan pribadi sensor diabaikan | Lulus (Terverifikasi di `process_message/context_preparer.rs`) |
| **QS-03** | #maintainability | Waktu pasang skill < 10 detik tanpa restart | Lulus (Terverifikasi di `skill_scaffolder.py`) |
| **QS-04** | #efficiency | Waktu pencarian FTS5 < 5 milidetik | Lulus (Terverifikasi di `sqlite_store/session.rs`) |
| **QS-05** | #usability | 0 tabel rusak di WhatsApp, klarifikasi terarah | Lulus (Terverifikasi di `domain/persona.rs`) |
| **QS-06** | #maintainability | 100% jadwal kustom bertahan pasca redeploy | Lulus (Terverifikasi di `activity.rs` & volume persistensi) |
| **QS-07** | #adaptability | Deteksi kebosanan $\ge 0.5$, 0% halusinasi cuaca | Lulus (Terverifikasi di `reflect.py` & Open-Meteo API) |
| **QS-08** | #security | 100% deteksi manipulasi log rantai hash SHA-256 | Lulus (Terverifikasi di `sqlite_store/mod.rs` & NIST AU-10) |
| **QS-09** | #security | 0% kebocoran data internal ke nomor asing | Lulus (Terverifikasi di `context_preparer.rs` Zero Trust) |
| **QS-10** | #security, #usability | 100% penghapusan profil mandiri sesuai UU PDP | Lulus (Terverifikasi di `builtin_commands.rs` test) |
| **QS-11** | #security | 100% klaim impersonasi terkunci `UNCONFIRMED` | Lulus (Terverifikasi di `profiler.rs` & `user.rs` test) |
| **QS-12** | #reliability, #efficiency | Maks 8 agen simultan, <= 2 in-flight/akun, 0% 429 | Lulus (Terverifikasi di `account_pool.rs` & `adapter.rs`) |
| **QS-13** | #security | 0% fail-open/SSRF/query-key admin, lockout 429 aktif | Aspirational (Target Fase 17 — terlacak di RISK-15 & DEBT-07) |
| **QS-14** | #security, #maintainability | 0% path traversal OAuth, 0 hardcoded secret di Git | Aspirational (Target Fase 18 — terlacak di RISK-16 & DEBT-05) |
| **QS-15** | #reliability, #security | 0% bleed kuota pool, maks 1x notice lalu silent drop | Aspirational (Target Fase 19 — terlacak di RISK-17 & DEBT-06) |
| **QS-16** | #security | 100% blokir mutasi target ambien & 0% key di turn Guest | Aspirational (Target Fase 20 — terlacak di RISK-18 & DEBT-08) |
