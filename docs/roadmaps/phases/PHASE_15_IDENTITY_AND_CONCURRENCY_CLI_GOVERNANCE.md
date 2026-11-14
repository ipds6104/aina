# ⚡ Fase 15: Identity & Concurrency Native CLI Governance

> **Status**: APPROVED  
> **Komponen Target**: `src/adapters/driving/cli/commands/user.rs`, `src/adapters/driving/cli/commands/status.rs`, `src/adapters/driving/cli/commands/policy.rs`  
> **Prinsip**: *"Machine-Readable First, Zero Interactivity, Fail-Closed Security."*

---

## 1. Problem Statement & Kebutuhan Operasional

Agar User Companion (Admin) atau sistem otomasi dapat menginspeksi dan mengelola status verifikasi pengguna serta kapasitas beban konkurensi tanpa perlu membuka koneksi SQLite manual:
1. Diperlukan antarmuka CLI native untuk meninjau antrean klaim identitas yang berstatus `UNCONFIRMED`.
2. Diperlukan perintah deterministik untuk mengesahkan (`verify`) atau menolak (`reject-claim`) klaim nama dan peran.
3. Diperlukan telemetri pemantauan antrean konkurensi secara transparan (`aina status --concurrency`).

---

## 2. Spesifikasi Antarmuka CLI Native

### A. Subcommand Manajemen Identitas (`aina user`)

```bash
# 1. Menampilkan seluruh pengguna yang mengajukan klaim identitas dan berstatus UNCONFIRMED
aina user unconfirmed
aina user unconfirmed --json

# Output Contoh:
# 📋 Daftar Klaim Identitas Belum Terverifikasi (UNCONFIRMED):
# --------------------------------------------------------------------------------
# • JID           : 6289912345678@s.whatsapp.net
#   Klaim Nama    : Budi Hendra
#   Klaim Peran   : Pranata Komputer IPDS
#   Waktu Klaim   : 2026-11-14 18:10 WIB
#   Status Otoritas: GUEST
# --------------------------------------------------------------------------------

# 2. Memverifikasi dan mengesahkan profil pengguna
aina user verify 6289912345678@s.whatsapp.net \
  --name "Budi Hendra" \
  --role "Pranata Komputer IPDS" \
  --authority staff \
  --notes "Disahkan langsung oleh Companion via chat"

# Format Machine-Readable JSON:
aina user verify 6289912345678@s.whatsapp.net --authority staff --json

# 3. Menolak klaim palsu / klaim mencurigakan (Anti-Impersonation)
aina user reject-claim 6289912345678@s.whatsapp.net \
  --reason "Mencoba mengklaim nama admin (Ihza Karunia)"
```

### B. Subcommand Telemetri Konkurensi & Status Antrean (`aina status`)

```bash
# Melihat status kapasitas konkurensi, semaphore, dan antrean pesan:
aina status --concurrency
aina status --concurrency --json

# Output Contoh:
# ⚡ Status Konkurensi & Antrean Beban (High-Density Telemetry):
# --------------------------------------------------------------------------------
# • Kapasitas Semaphore Maksimal : 8 slot aktif
# • Slot In-Flight Sedang Berjalan: 2 slot (25% utilisasi)
# • Antrean Pesan Menunggu (Buffer): 0 pesan
# • Status Kolam Akun Google Antigravity:
#   - Akun #1 (d****@gmail.com): [ACTIVE] 1 in-flight
#   - Akun #2 (p****@gmail.com): [ACTIVE] 1 in-flight
#   - Akun #3 (i****@gmail.com): [IDLE]   0 in-flight
#   - Akun #4 (s****@gmail.com): [IDLE]   0 in-flight
# --------------------------------------------------------------------------------
```

---

## 3. Integrasi Kebijakan Agentic (`aina policy`)

Ketika AI Agent mengusulkan pengesahan identitas melalui percakapan alami WhatsApp:
- Agen hanya berhak mengeksekusi `aina policy verify-user <jid>` jika pesan pemicu berasal dari **`ADMIN_JID`** dan menyertakan token otorisasi tur yang valid (`AINA_TURN_TOKEN`).
- Eksekusi dari pihak `guest` atau tanpa token otomatis menghasilkan penolakan keras: `Exit Code 2 (Forbidden)`.
