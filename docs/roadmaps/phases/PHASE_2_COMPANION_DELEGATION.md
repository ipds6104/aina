# Phase 2: Explicit Companion Delegation Engine

> [!NOTE]
> **Komponen**: `src/core/usecases/process_message/builtin_commands.rs`, `src/core/ports/session_store.rs`, `src/adapters/driving/cli/commands/user.rs`  
> **Status**: Ready for Implementation  
> **Tujuan**: Menerapkan mekanisme bagi User Companion untuk memberikan, mencabut, dan mengelola izin rekan kerja baik di grup maupun via DM secara instan dan aman.

---

## 1. Masalah yang Diselesaikan

Di sistem lama, Companion tidak memiliki perintah cepat dalam WhatsApp untuk mendelegasikan wewenang kepada rekan kerja di grup. Satu-satunya cara adalah melalui terminal server `aina user set ...`, yang menyulitkan operasi mobile sehari-hari di lapangan.

---

## 2. Rencana Perubahan Teknis

### 2.1 Perintah Cepat Companion di WhatsApp (`builtin_commands.rs`)
Tambahkan handler perintah administratif yang HANYA bereaksi jika pengirim adalah Companion (`is_admin_jid` / `msg.is_from_me`):

1. **Otorisasi Pengguna Tertentu di Grup / DM**:
   - Pola: `!aina authorize @user [role]` atau `!aina izinkan @user [role]`
   - Contoh: `!aina izinkan @62812345678 sebagai staff`
   - Hasil: Mengupdate `user_profiles` dengan `authority_level: "staff"`, `granted_by: companion_jid`, `granted_at: now()`.
   - Balasan: *"✅ Rekan kerja @62812345678 berhasil diizinkan sebagai staf tim."*

2. **Otorisasi Seluruh Anggota Grup (Group Trust)**:
   - Pola: `!aina izinkan grup ini` atau `!aina authorize group`
   - Hasil: Menandai `group_memberships.is_group_trusted = 1` untuk `chat_jid` grup tersebut. Seluruh anggota di grup ini sekarang diakui sebagai rekan kerja dalam koordinasi kegiatan grup.
   - Balasan: *"🛡️ Grup ini sekarang telah diverifikasi sebagai grup tim kerja sah oleh Bg Ihza. Anggota di grup ini memiliki wewenang staf untuk tugas kerja bersama."*

3. **Pencabutan Wewenang (Revoke)**:
   - Pola: `!aina cabut izin @user` atau `!aina revoke @user`
   - Hasil: Mengubah `authority_level` kembali ke `guest`.

4. **Pemeriksaan Status Otoritas Pengguna**:
   - Pola: `!aina siapa @user` atau `!aina cek izin @user`
   - Hasil: Menampilkan kartu profil, peran, tingkat otoritas, dan siapa yang mengizinkannya.

### 2.2 Warisan Otoritas dari Grup ke DM (Cross-Channel Inheritance)
- Saat Companion memberikan izin di grup via `!aina izinkan @user`, record `user_profiles` diperbarui di SQLite database global.
- Karena `user_profiles` berbasis `sender_jid` (nomor WhatsApp unik), ketika user tersebut kemudian mengirim pesan pribadi (DM) ke Aina, Aina langsung membaca profil yang sudah berstatus `staff`.
- Agen langsung melayani user sebagai rekan kerja sah tanpa perlu meminta izin lagi.

### 2.3 Pembaruan CLI `aina user` & `aina group`
- Tambahkan flag `--granted-by` pada `aina user set`.
- Tambahkan subcommand `aina group list` dan `aina group trust <group_jid> [--untrust]` untuk administrasi dari terminal.

---

## 3. Rencana Pengujian Bukti (Evidence Tests)
- `test_companion_can_authorize_user_in_group`: Memverifikasi pesan perintah `!aina izinkan @user` oleh admin berhasil mengubah status target.
- `test_non_companion_cannot_execute_authorize`: Memverifikasi anggota biasa yang mengetik `!aina izinkan ...` ditolak.
- `test_cross_channel_inheritance_group_to_dm`: Memverifikasi user yang diizinkan di grup mempertahankan status staff saat DM.
