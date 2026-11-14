# Phase 6: Tamper-Evident Non-Repudiation Audit Trail & Provenance

> [!NOTE]
> **Komponen Target**: `src/adapters/driven/sqlite_store/schema.rs`, `src/adapters/driven/sqlite_store/audit.rs`, `src/core/usecases/process_message/audit_tracker.rs`, CLI `aina audit`  
> **Status**: Selesai (Done - Implemented in `src/adapters/driven/sqlite_store/audit.rs`, `src/adapters/driving/cli/commands/audit.rs`, and `src/core/usecases/process_message/audit_tracker.rs`)  
> **Standar Rujukan**: NIST SP 800-53 Rev. 5 (AU-3, AU-10 Non-Repudiation), ISO/IEC 42001:2023 (AIMS Traceability & Operational Evidence), EU AI Act (Article 12 Record-Keeping)  
> **Tujuan**: Memastikan setiap tindakan eksekusi, pemanggilan alat (tool call), modifikasi file, atau perubahan izin oleh Aina terikat secara kriptografis dan tak terbantahkan (*non-repudiable*) ke nomor WhatsApp pengirim yang memintanya.

---

## 1. Masalah yang Diselesaikan

Di lingkungan kantor dan organisasi enterprise:
1. **Akuntabilitas Tindakan AI**: Jika Aina mengunggah berkas ke Google Drive, menjalankan script di terminal server, atau mengubah jadwal pengingat, auditor kantor wajib mengetahui **siapa nomor WhatsApp yang memerintahkannya**.
2. **Risiko Modifikasi Log (Log Tampering)**: Tabel audit standar di database relasional rentan diedit atau dihapus secara sepihak oleh siapapun yang memiliki akses shell.
3. **Keterbatasan Pelacakan Alat Saat Ini**: Tabel `whatsapp_action_audits` saat ini hanya mencatat daftar nama alat global `["run_command", "view_file"]` per giliran chat, tanpa merekam parameter perintah spesifik per eksekusi alat dan bukti *turn token* yang mendasarinya.

---

## 2. Rencana Arsitektur & Perubahan Teknis

### 2.1 Skema Audit Rantai Kriptografis (*Hash-Chained Audit Trail*)
Tambahkan tabel eksekusi alat terperinci (`agent_tool_executions`) dengan pengikatan rantai hash (*tamper-evident SHA-256 hash chaining*):

```sql
CREATE TABLE IF NOT EXISTS agent_tool_executions (
    id                 INTEGER PRIMARY KEY AUTOINCREMENT,
    action_audit_id    INTEGER NOT NULL,              -- Relasi ke whatsapp_action_audits.id
    turn_token_prefix  TEXT NOT NULL,                 -- 16 karakter pertama dari TurnCapability token
    sender_jid         TEXT NOT NULL,                 -- Nomor WhatsApp pemohon sah
    sender_authority   TEXT NOT NULL,                 -- Otoritas saat eksekusi: 'admin' | 'staff' | 'atasan' | 'guest'
    tool_name          TEXT NOT NULL,                 -- Misal: 'run_command', 'wa_tool', 'gdrive_upload'
    command_payload    TEXT NOT NULL,                 -- Perintah/parameter mentah yang dieksekusi
    execution_status   TEXT NOT NULL,                 -- 'success' | 'failed' | 'blocked'
    exit_code          INTEGER,
    duration_ms        INTEGER NOT NULL,
    prev_hash          TEXT NOT NULL,                 -- SHA-256 dari baris audit sebelumnya
    record_hash        TEXT NOT NULL,                 -- SHA-256(prev_hash + sender_jid + tool_name + command_payload + timestamp)
    created_at_epoch   INTEGER NOT NULL,
    FOREIGN KEY(action_audit_id) REFERENCES whatsapp_action_audits(id)
);

CREATE INDEX IF NOT EXISTS idx_tool_exec_sender ON agent_tool_executions(sender_jid, created_at_epoch DESC);
CREATE INDEX IF NOT EXISTS idx_tool_exec_name ON agent_tool_executions(tool_name, created_at_epoch DESC);
CREATE INDEX IF NOT EXISTS idx_tool_exec_hash ON agent_tool_executions(record_hash);
```

### 2.2 Prinsip Non-Repudiasi (Attribution Binding)
Setiap eksekusi alat di sandbox agen wajib disematkan metadata provenansi:
1. **Identitas Pemohon**: `sender_jid` asli yang memicu giliran percakapan.
2. **Kredensial Otorisasi**: `TurnCapability` yang sah, memastikan agen tidak dapat memalsukan asal-usul pesan.
3. **Integritas Rantai Hash**:
   $$\text{Hash}_n = \text{SHA256}(\text{Hash}_{n-1} \parallel \text{sender\_jid} \parallel \text{tool\_name} \parallel \text{payload} \parallel \text{timestamp})$$
   Jika ada baris yang disunting secara ilegal di database SQLite, rantai hash akan putus (*integrity broken*).

### 2.3 Antarmuka Audit CLI Baru (`aina audit`)
Perluas perintah `aina audit` ([audit.rs](file:///root/projects/aina/src/adapters/driving/cli/commands/audit.rs)):
- `aina audit trace --sender <whatsapp_jid> [--limit 20]`:
  Menampilkan kronologi seluruh alat dan perintah yang dijalankan atas permintaan nomor tertentu.
- `aina audit who-ran <tool_name>`:
  Mencari siapa saja pengguna yang pernah memicu alat tertentu (misal: `wa_tool`, `git`, `rm`, `curl`).
- `aina audit verify-chain`:
  Melakukan verifikasi deterministik terhadap seluruh rantai hash audit untuk memastikan tidak ada catatan yang dimanipulasi.

---

## 3. Integrasi Skill & Observabilitas

1. **`skills/self-observability/`**:
   - Skill observabilitas otonom Aina dapat memanfaatkan riwayat audit untuk mendeteksi anomali (misal: nomor asing yang berulang kali memicu perintah gagal / probing).
2. **Laporan Audit Otomatis untuk Companion**:
   - Jika ada aksi yang gagal atau ditolak kebijakan pada wewenang staf/atasan, ringkasan audit otomatis disiapkan untuk notifikasi Companion via japri DM.

---

## 4. Kriteria Keberhasilan & Pengujian Bukti (Evidence Tests)

- `test_audit_trail_creates_valid_hash_chain`: Memverifikasi setiap penambahan record menghasilkan hash chaining yang valid.
- `test_audit_chain_detects_tampering`: Memverifikasi perubahan isi baris audit lama menyebabkan fungsi `verify-chain` mengembalikan error manipulasi data.
- `test_tool_execution_strictly_attributed_to_sender`: Memverifikasi pesan dari nomor A tidak dapat tercatat atas nama nomor B.
