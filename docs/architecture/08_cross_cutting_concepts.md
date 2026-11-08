# 8. Konsep Lintas Bidang (Cross-Cutting Concepts)

> [!INFO] **Architecture Provenance & Freshness**
> - **Verified Commit**: `1ee7597` (branch: `main`)
> - **Verified Timestamp**: `2026-11-08T16:40:29+07:00`
> - **Tracked Paths**: `src/`, `config/`, `skills/`
> - **Drift Status**: `verified`

Bab ini mendokumentasikan pola-pola rancangan umum (*architectural patterns*) yang berlaku di seluruh komponen Aina, mulai dari keamanan, pencatatan log, etika interaksi, hingga penemuan keahlian otonom.

---

## 8.1 Manajemen Kunci Rahasia & Pencegahan Kebocoran (Zero-Leakage Security)

Keamanan kunci API dan kredensial kantor dilindungi melalui 3 lapis pengamanan:
1. **Penyuntikan Dinamis (*Just-in-Time Secret Broker*)**: Kunci akses tidak disimpan di file konfigurasi. Saat script memerlukan token (misal token Google Drive), script memanggil `secret_tool.py` yang mengambilnya dari brankas memori dan langsung menghapusnya setelah eksekusi.
2. **Penyaring Teks Keluar (*Output Sanitizer*)**: Seluruh teks balasan yang akan dikirim ke WhatsApp melewati filter ekspresi reguler di `src/core/domain/sanitizer.rs` untuk menghapus token otorisasi, bearer token, atau URL sensitif yang tidak sengaja tercetak.
3. **Pemisahan Jalur Obrolan (*Session Privacy Boundary*)**: Percakapan di ruang pribadi (DM) dan grup kantor disimpan dalam sesi terisolasi; Aina dilarang mengutip pesan dari DM pemilik ke dalam obrolan grup publik.

---

## 8.2 Logging, Telemetri, & Metakognisi Diri (Self-Observability)

Aina memantau kinerjanya sendiri (*metacognition*) secara transparan:
1. **Audit Interaksi WhatsApp (`whatsapp_action_audits`)**: Setiap pesan masuk mencatat durasi proses, daftar tool yang dipanggil, usecase, serta status sukses/gagal di SQLite.
2. **Riwayat Tugas Terjadwal (`scheduled_task_runs`)**: Setiap eksekusi aktivitas rutin mencatat waktu mulai, durasi detik, cuplikan output, dan pesan error jika ada.
3. **Skill Pengamat Diri (`skills/self-observability`)**: Aina dapat menjalankan audit terhadap frekuensi pemanggilan tool dan mendeteksi titik kemacetan (*bottlenecks*) melalui perintah `aina audit summary` atau script `analyze_telemetry.py`.

---

## 8.3 Persistensi Jadwal & Rekonsiliasi Idempoten (Activity State Persistence)

Untuk menjamin ketersediaan rutinitas terjadwal di lingkungan kontainer Docker/Coolify yang dinamis:
1. **Identitas Parsial Unik (`slug`)**: Setiap tugas terjadwal memiliki kode unik permanen (seperti `morning_briefing`, `status_senja`, `afternoon_deadline_check`).
2. **Resolusi Konflik Hibrida**: Modifikasi instruksi atau jam eksekusi yang dibuat pengguna disimpan di `/app/data/activities/<slug>.json` dan basis data SQLite, sehingga tidak tertimpa saat citra Docker diperbarui.
3. **Sinkronisasi Idempoten**: Saat bot booting, `ActivityReconciler::sync_all` menyelaraskan seluruh tugas secara otomatis tanpa memicu entri ganda atau kegagalan database.

---

## 8.4 Standar Keahlian Agen & Penemuan Bertingkat (Three-Tier Progressive Disclosure)

Aina mengadopsi standar terbuka internasional **`agentskills.io`** untuk seluruh keahliannya:
1. **Tier 1 (Discovery)**: Memuat metadata YAML frontmatter (`name`, `description`) dengan biaya token sangat rendah (~40 token/skill).
2. **Tier 2 (Activation)**: Membaca berkas `SKILL.md` hanya saat tugas pengguna memerlukan keahlian tersebut, menjaga context window tetap bersih (*anti-context rot*).
3. **Tier 3 (Execution)**: Menjalankan skrip Python/Bash di subproses terisolasi dengan output format JSON murni, menghindari pencemaran kode sumber di jendela konteks AI.
4. **Auto-Discovery Tanpa Restart**: Folder `/app/data/custom-skills/` dihubungkan ke `.agents/skills/` via symlink permanen, membuat skill baru langsung aktif di turn berikutnya.

---

## 8.5 Integritas Identitas Visual & Gaya Hidup Pontianak (Anti-Visual Drifting)

Sebagai agen persona yang hidup dan konsisten:
1. **Jangkar Visual Permanen (*Visual Anchors*)**: Rambut panjang *silver-lavender* dengan kepang samping khas, mata biru berbintang, dan jepit bulan sabit & bintang di sisi kiri kepala mutlak terkunci di setiap gambar status WhatsApp.
2. **Matriks Lemari Pakaian Dinamis (*Dynamic Wardrobe*)**: Menggunakan 5 preset busana (`wfh_cozy`, `smart_casual`, `outdoor_nature`, `night_stargaze`, `celestial_sig`) dengan kuota kreasi busana baru maksimal 1–2 kali per minggu untuk mencegah kebosanan.
3. **Grounding Fisik Khatulistiwa & Kuliner Pontianak**: Penyesuaian tema visual dan aktivitas harian diselaraskan dengan cuaca faktual Pontianak (terik siang tropis vs sejuk gerimis sore) serta kuliner lokal (Es Lidah Buaya, Chai Kwe, Bubur Pedas, Kopi Saring Gajah Mada).

---

## 8.6 Etika Percakapan & Kerendahan Hati Epistemik (Epistemic Humility)

1. **Prinsip Ahludz-Dzikri (Verifikasi Sebelum Berasumsi)**: Dilarang berspekulasi atau berhalusinasi saat ragu; agen wajib melakukan verifikasi ke sumber primer atau melakukan riset web terarah.
2. **Prinsip Tabayyun (Klarifikasi Terarah)**: Menanyakan maksud pengguna secara santun jika instruksi multitafsir sebelum mengambil tindakan permanen (*one-way door*).
3. **Prinsip Tawaqquf (Perlindungan Data Rahasia)**: Menolak membagikan data internal atau rahasia kantor kepada pihak ketiga tanpa izin pemilik bot.
4. **Tone Kerja Hangat Bebas Template CS**: Berbicara selayaknya rekan kerja akrab di kantor Indonesia modern tanpa frasa kaku bot seperti *"Ada yang bisa dibantu?"*.
