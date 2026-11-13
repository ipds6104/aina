# Phase 4: Dynamic Policy Engine & Production RBAC

> [!NOTE]
> **Komponen**: `src/core/domain/access_control/`, `src/core/domain/metacognition/manifest.rs`, CLI `aina policy`  
> **Status**: Design & Production Extensibility  
> **Tujuan**: Menyediakan engine kebijakan akses dinamis (granular capability scopes) yang dapat diperketat dan disesuaikan saat Aina beroperasi di lingkungan produksi tanpa perlu recompile kode.

---

## 1. Masalah yang Diselesaikan

Dalam implementasi skala besar, organisasi memiliki variasi kebutuhan keamanan:
- Ada grup yang boleh menggunakan Google Drive tapi tidak boleh memicu Antigravity CLI bash execution.
- Ada tamu yang hanya diizinkan konsultasi tanya-jawab dokumen publik, tapi dilarang keras memicu scheduler atau export riwayat chat.
- Administrator membutuhkan kemampuan mengubah batas izin (*policy scopes*) secara on-the-fly di server produksi.

---

## 2. Rencana Arsitektur Teknis

### 2.1 Definisi Capability Scopes
Setiap tool dan tindakan agentik di Aina dipetakan ke dalam *scope* terstandarisasi:
- `scope:casual_chat`: Percakapan santai, tanya-jawab umum, FAQ publik. (Default untuk `GUEST`).
- `scope:office_task`: Analisis dokumen PDF/Excel, pembuatan draft surat, coding assistance. (Default untuk `STAFF`).
- `scope:drive_read`: Membaca Google Spreadsheet atau dokumen Drive internal.
- `scope:drive_write`: Mengunggah file atau membuat Spreadsheet baru.
- `scope:scheduler_manage`: Mendaftarkan tugas rutin atau alarm pengingat.
- `scope:companion_sensor`: Membaca obrolan akun companion atau sensor grup lain via `--companion`. (Hanya untuk `ADMIN`).
- `scope:system_control`: Mengganti model, mereset sesi, menambah akun Google Antigravity. (Hanya untuk `ADMIN`).

### 2.2 Tabel Kebijakan Akses Dinamis (`access_policies`)
```sql
CREATE TABLE IF NOT EXISTS access_policies (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    policy_name TEXT NOT NULL UNIQUE,
    target_type TEXT NOT NULL, -- 'role', 'group', 'user'
    target_id TEXT NOT NULL,   -- e.g. 'staff', 'guest', or specific jid
    allowed_scopes TEXT NOT NULL, -- JSON array: ["casual_chat", "office_task"]
    denied_scopes TEXT NOT NULL DEFAULT '[]',
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);
```

### 2.3 Antarmuka CLI & WhatsApp Command
Companion dapat memeriksa dan mengupdate policy di produksi:
- `aina policy list`
- `aina policy set role:guest --allow '["casual_chat"]' --deny '["drive_read","drive_write","companion_sensor"]'`
- WhatsApp command: `!aina policy set ...` (khusus Companion).

---

## 3. Rencana Pengujian Bukti (Evidence Tests)
- `test_policy_engine_denies_unauthorized_scope`: Memverifikasi request tool yang memerlukan scope tertentu ditolak jika profil tidak memiliki izin.
- `test_dynamic_policy_override_in_runtime`: Memverifikasi penambahan rule dinamis segera aktif tanpa restart server.
