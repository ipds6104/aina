# Phase 1: Zero-Trust Invariant & Profile Seeding Hardening

> [!NOTE]
> **Komponen**: `src/adapters/driven/sqlite_store/`, `src/core/usecases/process_message/context_preparer.rs`, `src/core/usecases/process_message/mod.rs`  
> **Status**: Ready for Implementation  
> **Tujuan**: Menghentikan eskalasi wewenang otomatis dari obrolan grup dan memperkuat skema profil pengguna di database.

---

## 1. Masalah yang Diselesaikan

Pada implementasi lama:
1. `process_message/mod.rs` (pada jalur `RecordOnly` dan `Respond`) mengeksekusi:
   ```rust
   let new_profile = crate::core::ports::UserProfile {
       sender_jid: msg.sender.jid.clone(),
       name: Some(clean_name.to_string()),
       role: Some("Rekan Tim".to_string()),
       authority_level: "staff".to_string(), // BUG KRITIS: Auto-promote siapapun di grup menjadi staff
       notes: Some("Terdaftar otomatis dari obrolan grup".to_string()),
   };
   ```
2. `context_preparer.rs` mengeksekusi:
   ```rust
   // Seamless promotion: if user was previously a guest, but now has confirmed group memberships,
   // automatically verify them as staff!
   if p.authority_level == "guest" && !user_groups.is_empty() {
       p.authority_level = "staff".to_string(); // BUG KRITIS: Membocorkan wewenang staf hanya karena anggota grup
   }
   ```
3. `AutonomousProfiler` dapat menimpa `role` dan `name` berdasarkan klaim sepihak di teks chat.

---

## 2. Rencana Perubahan Teknis

### 2.1 Migrasi Skema SQLite (`schema.rs`)
Tambahkan kolom baru ke tabel `user_profiles` dan `group_memberships` dengan *backward compatibility*:
```sql
ALTER TABLE user_profiles ADD COLUMN granted_by TEXT;
ALTER TABLE user_profiles ADD COLUMN granted_at INTEGER;
ALTER TABLE user_profiles ADD COLUMN role_hierarchy TEXT DEFAULT 'guest'; -- 'admin', 'atasan', 'staff', 'guest'
ALTER TABLE group_memberships ADD COLUMN is_group_trusted BOOLEAN NOT NULL DEFAULT 0;
ALTER TABLE group_memberships ADD COLUMN authorized_by TEXT;
ALTER TABLE group_memberships ADD COLUMN authorized_at INTEGER;
```

### 2.2 Hardening `UserProfile` Struct & Ports
Perbarui `UserProfile` struct di `src/core/ports/session_store.rs`:
```rust
pub struct UserProfile {
    pub sender_jid: String,
    pub name: Option<String>,
    pub role: Option<String>,
    pub authority_level: String, // "admin", "atasan", "staff", "guest"
    pub notes: Option<String>,
    pub granted_by: Option<String>,
    pub granted_at: Option<i64>,
    pub role_hierarchy: Option<String>,
}
```

### 2.3 Hardening Seeding Logika (`context_preparer.rs` & `mod.rs`)
1. Pengguna baru yang masuk dari grup atau DM **SELALU** di-seed sebagai `authority_level: "guest"`.
2. Hapus total logika promosi otomatis: `if p.authority_level == "guest" && !user_groups.is_empty()`.
3. Promosi ke `staff` HANYA diizinkan jika:
   - Profil pengguna memiliki `granted_by` yang valid (Companion), ATAU
   - Salah satu grup pengguna berstatus `is_group_trusted == true`.
4. `AutonomousProfiler` dilarang mengubah `authority_level`. Jika mendeteksi klaim peran ("saya atasan", "saya staff IT"), klaim tersebut hanya disimpan ke dalam `notes` sebagai `[Klaim Mandiri: ...]` tanpa meningkatkan privilege sistem.

---

## 3. Rencana Pengujian Bukti (Evidence Tests)
- `test_guest_in_group_stays_guest_without_companion_authorization`: Memverifikasi pesan di grup tidak mengubah guest menjadi staff.
- `test_autonomous_profiler_cannot_escalate_privilege`: Memverifikasi klaim teks "saya admin" tidak mengubah `authority_level`.
