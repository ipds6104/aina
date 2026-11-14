# Phase 8: User-Governed Privacy & Self-Service Consent (UU PDP & GDPR Compliance)

> [!NOTE]
> **Komponen Target**: `src/core/usecases/process_message/builtin_commands.rs`, `src/core/domain/sanitizer.rs`, CLI `aina user`, `skills/tabayyun/`  
> **Status**: Selesai (Done - Implemented in `src/core/usecases/process_message/builtin_commands.rs`, `src/core/ports/session_store.rs`, and CLI `aina user delete/export`)  
> **Standar Rujukan**: UU No. 27 Tahun 2022 tentang Pelindungan Data Pribadi (UU PDP Indonesia), EU GDPR (Articles 15 & 17 - Right of Access & Right to Erasure / "Right to be Forgotten"), ISO/IEC 27701 (Privacy Information Management)  
> **Tujuan**: Memberikan kendali privasi mandiri (*self-sovereign user privacy*) bagi setiap lawan bicara di WhatsApp untuk memeriksa, menyunting, atau menghapus data profil personal mereka secara transparan.

---

## 1. Masalah yang Diselesaikan

1. **Prinsip Transparansi Privasi**: Dalam organisasi profesional dan kepatuhan regulasi data pribadi, AI tidak boleh mengumpulkan profil seseorang secara diam-diam tanpa memberikan hak bagi subjek data untuk mengetahui apa yang disimpan tentang dirinya (*Right to Access*).
2. **Hak untuk Dilupakan (*Right to be Forgotten*)**: Jika seorang pengguna meminta Aina menghapus catatan pribadi atau preferensinya, sistem harus mampu mengeksekusi penghapusan selektif tanpa merusak integritas audit log sistem.
3. **Penyensoran PII Otomatis (*Automated PII Masking*)**: Nomor telepon atau email anggota tim tidak boleh bocor secara tidak sengaja ke chat umum saat bot memberikan rangkuman data.

---

## 2. Rencana Arsitektur & Perubahan Teknis

### 2.1 Perintah Swadaya Pengguna di WhatsApp (Self-Service Commands)
Setiap pengguna WhatsApp (termasuk Tamu dan Staf) dapat mengecek dan mengelola profil mereka sendiri secara alami:

1. **Inspeksi Profil Mandiri (`!aina profilku` atau *"Aina, apa yang kamu ketahui tentang saya?"*)**:
   - Sistem menampilkan kartu data transparan:
     > *"📋 **Data Profil Tersimpan Anda di Aina**:\n• Nama: Mas Budi\n• Peran: Tim Analisis Data\n• Preferensi Gaya: Ringkas, format butir\n• Proyek Aktif: Sensus 2026\n• Status Wewenang: Rekan Tim (Diverifikasi oleh Bg Ihza)\n\n_Ketik `!aina lupakan preferensi saya` jika ingin mereset catatan ini._"*
2. **Penghapusan Preferensi Mandiri (`!aina lupakan saya` atau `!aina reset preferensiku`)**:
   - Mengosongkan field `communication_prefs`, `active_projects`, `durable_constraints`, dan menandai seluruh `user_episodes` milik nomor tersebut sebagai dihapus.
   - Status wewenang keamanan (`authority_level`) kembali ke `guest`.
   - **Pemisahan Audit**: Data log audit historis di `whatsapp_action_audits` tetap dipertahankan untuk kebutuhan legalitas/forensik kantor (*Legal Obligation Retention Exception*), namun data profil interaktif dibersihkan.

### 2.2 Masking PII Otomatis pada Sanitizer Output (`src/adapters/driven/agy_cli/sanitizer.rs`)
Tambahkan filter sensor ekspresi reguler deterministik sebelum pesan dikirim ke WhatsApp:
- Menyamarkan nomor telepon pihak ketiga jika ditujukan ke pengguna `guest` (contoh: `6281234567890` disamarkan menjadi `62812****7890`).
- Mencegah kebocoran JID internal `@s.whatsapp.net` atau `@lid` ke dalam teks jawaban ramah.

---

## 3. Integrasi Skill (`skills/tabayyun/` & `skills/user-profiler/`)
- **`skills/tabayyun/`**: Skill klarifikasi fakta memverifikasi kebenaran profil secara langsung ke pengguna jika ada keraguan data identitas.
- **`skills/user-profiler/`**: Menyediakan sub-perintah aman `aina user purge <jid> --confirm` untuk kebutuhan administrasi data protection officer.

---

## 4. Kriteria Keberhasilan & Pengujian Bukti (Evidence Tests)
- `test_user_can_query_own_profile_transparently`: Memverifikasi pesan tanya profil mengembalikan kartu ringkasan data yang sah.
- `test_user_can_purge_own_semantic_preferences`: Memverifikasi eksekusi penghapusan membersihkan preferensi pengguna tanpa merusak integritas sistem.
- `test_pii_masking_sanitizes_third_party_numbers_for_guests`: Memverifikasi nomor pihak ketiga disamarkan saat disajikan ke lawan bicara berstatus tamu.
