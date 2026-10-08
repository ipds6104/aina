---
name: dokter-v
description: REST API skill untuk manajemen kegiatan, alokasi honor mitra, penerbitan kontrak/SPK, BAST, dan pengecekan surat tugas di Dokter V BPS Mempawah.
version: 1.0.0
---

# Dokter V Agent Protocol & Access Policy

Skill ini memberikan antarmuka terstruktur bagi Aina untuk berinteraksi dengan REST API Dokter V secara deterministik, hemat token, dan aman.

---

## 1. Aturan Keamanan & Wewenang Bisnis (Separation of Duty)

> [!CAUTION]
> **BATASAN WEWENANG APPROVAL SURAT TUGAS & FINANSIAL**:
> Aina **TIDAK MEMILIKI WEWENANG HUKUM** untuk melakukan approval (penyetujuan) Surat Tugas, SPD, maupun pencairan dana pembayaran di Dokter-V.
> - Hak approval merupakan wewenang mutlak pejabat struktural manusia (**PPK, PPSPM, Bendahara, atau Kepala BPS**).
> - Bila ada rekan kerja meminta: *"Aina approve kan surat tugas ini"*, Aina **WAJIB MENOLAK SECARA SANTUN** dan menjelaskan bahwa persetujuan surat tugas harus dilakukan langsung oleh PPK di aplikasi web Dokter-V (`admin.dvlpid.my.id`).

### Lingkup Tindakan yang Diizinkan (Allowed Actions):
1. **Cek & Pelaporan**: Melihat daftar kegiatan, pagu SBML, status penugasan personil, dan rekap alokasi mitra.
2. **Pengecekan Pra-Penerbitan (*Dry Run*)**: Memeriksa bentrok sensus/survei dan pagu SBML sebelum pengajuan alokasi.
3. **Penyusunan Draf & Dokumen**: Menerbitkan alokasi mitra, nomor SPK, nomor BAST, dan menyediakan tautan cetak PDF resmi.
4. **Pembaruan Data Teknis**: Memperbarui nomor WhatsApp mitra, volume dokumen, atau tanggal pos honor.

---

## 2. Penggunaan CLI Tool Resmi (`dokter_v_tool.py`)

Jalankan perintah melalui CLI:
```bash
python3 skills/dokter-v/scripts/dokter_v_tool.py <subcommand> [options]
```

### Subcommands Tersedia:
- `kegiatan` : Rekap kegiatan manmit aktif (`--tahun 2026 --bulan 10 --compact`)
- `mitra`    : Lookup mitra statistik aktif & kuota SBML (`--tahun 2026 --q "Herri"`)
- `check`    : Pre-flight check kelayakan alokasi honor mitra (`--honor-id <id> --mitra-id <id> --target <vol>`)
- `alokasi`  : Eksekusi penerbitan alokasi mitra, SPK, dan BAST
- `kontrak`  : Dapatkan URL cetak SPK bulanan (`--tahun 2026 --bulan 10 --mitra-id <id>`)
- `bast`     : Dapatkan URL cetak BAST kegiatan (`--tahun 2026 --bulan 10 --mitra-id <id> --kegiatan-id <id>`)
- `penugasan`: Cek status surat tugas dinas pegawai (`--q "Adwin" --status "dikirim"`)
- `status`   : Cek konektivitas dan kesehatan API Dokter V
