# Phase 10: Discretionary Asset & File Custodianship Management (`aina asset`)

> [!NOTE]
> **Komponen Target**: `src/adapters/driving/cli/commands/asset.rs`, `src/core/domain/knowledge/asset.rs`, `knowledge/manifests/`  
> **Status**: Ready for Production Implementation  
> **Standar Rujukan**: NIST SP 800-53 (AC-3 Access Enforcement), ISO/IEC 42001 (AIMS Data Governance), Discretionary Access Control (DAC)  
> **Tujuan**: Mengimplementasikan antarmuka CLI operasional dan verifikasi deterministik untuk tata kelola hak asuh berkas (`RESTRICTED`), pencatatan Kustodian Berkas (`custodian_jid`), pemberian izin akses kepada rekan kerja (`authorized_users`), dan pencabutan hak akses.

---

## 1. Masalah yang Diselesaikan

1. **Pengendalian Manual Berkas Rahasia**: Saat ini Kustodian berkas dicatat di file manifest YAML, namun belum memiliki antarmuka perintah CLI langsung untuk inspeksi cepat, pendelegasian wewenang, dan pencabutan izin.
2. **Ketiadaan Validasi CLI Otomatis**: Tanpa perintah `aina asset`, operator atau agen yang ingin memverifikasi apakah User B memiliki hak akses ke `laporan_keuangan_q3.xlsx` harus mem-parsing file YAML secara manual di bash.
3. **Kebutuhan Integrasi Audit**: Setiap pendelegasian wewenang berkas wajib dapat diverifikasi integritasnya melalui `aina audit trace` dan hash chaining SHA-256.

---

## 2. Rencana Arsitektur & Perubahan Teknis

### 2.1 Format Manifest Aset (`knowledge/manifests/<asset_slug>.yaml`)
```yaml
asset_name: "laporan_keuangan_q3.xlsx"
asset_slug: "laporan-keuangan-q3"
file_path: "shared_data/laporan_keuangan_q3.xlsx"
custodian_jid: "6281234567890@s.whatsapp.net"
classification: "RESTRICTED" # PUBLIC | INTERNAL | RESTRICTED
authorized_users:
  - "6281234567890@s.whatsapp.net"
  - "6289876543210@s.whatsapp.net"
created_at: "2026-11-14T17:45:00+07:00"
updated_at: "2026-11-14T17:45:00+07:00"
description: "Rekapitulasi keuangan triwulan III satker"
```

### 2.2 Antarmuka CLI Native (`aina asset`)
Tambahkan perintah `aina asset`:
```bash
# 1. Menampilkan seluruh aset terlindungi di workspace:
aina asset list [--workspace <ws>] [--json]

# 2. Memeriksa status dan daftar pemegang izin suatu berkas:
aina asset get <asset_slug> [--workspace <ws>] [--json]

# 3. Mendaftarkan berkas baru di bawah hak asuh pengirim:
aina asset register <file_path> --custodian <jid> [--classification <RESTRICTED|INTERNAL>] [--desc "<deskripsi>"]

# 4. Memberikan izin akses kepada rekan kerja (hanya sah dari Custodian/Admin):
aina asset grant <asset_slug> <target_jid> [--by <custodian_jid>]

# 5. Mencabut izin akses:
aina asset revoke <asset_slug> <target_jid> [--by <custodian_jid>]
```

---

## 3. Verifikasi & Pengujian Berbasis Bukti
1. **Aturan Hak Asuh**: Pihak selain Kustodian dan Admin yang mencoba menjalankan `aina asset grant` akan ditolak dengan kode kegagalan otorisasi (`ACCESS_DENIED`).
2. **Penyimpanan Idempoten**: Penambahan pengguna yang sudah terdaftar tidak menimbulkan duplikasi pada array `authorized_users`.
3. **Pencatatan Audit**: Perubahan wewenang aset otomatis terekam ke log audit sistem.
