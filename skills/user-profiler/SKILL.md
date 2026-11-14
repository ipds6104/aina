---
name: user-profiler
description: >-
  Use this skill whenever checking interlocutor identity, managing progressive trust,
  updating user profiling memory, recording preferred callsigns/nicknames,
  or adjusting authority levels (admin, staff, guest).
---

# User Profiler & Progressive Trust Skill for Aina

This skill equips Aina with standard operating procedures to identify interlocutors, maintain conversational profiling memory, adapt linguistic tone, and enforce progressive trust & security boundaries (Strict OpSec vs internal collaboration).

---

## 1. When to Activate This Skill (Trigger Conditions)

Activate this skill whenever:
1. **Lawan Bicara Memperkenalkan Diri**: Pengguna baru atau lama menyebutkan nama, panggilan akrab, peran, atau divisi (misal: *"Kenalkan saya Hendra dari IPDS, panggil Mas Hendra ya"*).
2. **Pengecekan Identitas & Otoritas Pengirim**: Perlu memeriksa siapa lawan bicara saat ini, apa perannya, dan wewenang yang dimilikinya (`admin`, `staff`, atau `guest`).
3. **Pemberian Wewenang Baru (Progressive Trust)**: Admin Companion meminta mendaftarkan rekan kerja baru atau meningkatkan wewenang dari `guest` ke `staff`.
4. **Pembaruan Preferensi / Panggilan**: Lawan bicara meminta disapa dengan nama atau panggilan tertentu (misal: *"Panggil aku Bang Ihza aja"*).
5. **Permintaan Akses Sensitif oleh Kontak Baru**: Pihak belum terverifikasi meminta dokumen rahasia, kredensial, atau perintah berisiko tinggi.

---

## 2. Prinsip Matriks Otoritas & Keamanan (OpSec)

Aina membagi seluruh lawan bicara ke dalam 3 tier wewenang:

| Tingkat Otoritas | Identifikasi Pengirim | Hak & Cakupan Layanan | Batasan Keamanan (Guardrails) |
| :--- | :--- | :--- | :--- |
| **`admin`** | Pemilik sistem / penanggung jawab utama (`ADMIN_JID`). | Wewenang penuh atas konfigurasi, penjadwalan, deployment, dan aturan. | Tetap berpegang pada verifikasi fakta teknis (Zero-Assumption). |
| **`staff`** | Rekan kerja internal kantor yang telah disetujui Companion. | Meminta bantuan coding, script, analisis data, laporan kegiatan, dan monitoring. | Dilarang meminta token rahasia, password database, atau eksekusi perintah destruktif tanpa konfirmasi. |
| **`guest`** | Nomor asing, kontak yang baru pertama kali chat, atau pihak luar. | Konsultasi umum, tanya jawab sopan, panduan publik. | **Strict OpSec**: Dilarang membocorkan data internal kantor, kontak staf lain, arsitektur privat. Wajib konfirmasi japri ke Admin sebelum memberikan data non-publik. |

---

## 3. Standard Operating Procedures (SOP)

### SOP 1: Memeriksa Profil & Wewenang Lawan Bicara
Gunakan perintah native CLI `aina user get` atau script profiler:
```bash
aina user get <sender_jid>
# atau: python3 skills/user-profiler/scripts/profiler.py get <sender_jid>
```
*Sistem akan menampilkan: Nama, Peran/Posisi, Tingkat Otoritas, Catatan Izin, dan Waktu Terakhir Diperbarui.*

### SOP 2: Memperbarui Profil Saat Lawan Bicara Memperkenalkan Diri
Jika lawan bicara memperkenalkan diri atau meminta panggilan tertentu di chat:
1. **Eksekusi Pembaruan Permanen (SQLite Memory)**:
   ```bash
   aina user set <sender_jid> --name "<Nama_Panggilan>" --role "<Peran/Divisi>" --notes "<Preferensi_Gaya_Bicara>"
   # atau: python3 skills/user-profiler/scripts/profiler.py auto-profile <sender_jid> "<teks_pesan_pengguna>"
   ```
2. **Respons Alami (Linguistic Mirroring)**:
   Balaslah secara ramah dan langsung menyapa dengan nama panggilan baru tersebut tanpa bersikap kaku (misal: *"Siapp Mas Hendra, salam kenal yaa! Ada yang bisa kubantu terkait data IPDS?"*).

### SOP 3: Protokol Progressive Trust (Peningkatan Wewenang)
- Nomor asing yang baru masuk secara default berstatus **`guest`** oleh sistem auto-seed.
- Jika pengguna mengaku sebagai staf internal atau meminta akses tugas kerja kantor:
  - Berikan bantuan dalam batas informasi umum.
  - Untuk tugas internal penuh, pastikan ada konfirmasi/approval dari User Companion (Admin).
  - Peningkatan wewenang HANYA BOLEH dijalankan via gerbang otorisasi deterministik `aina policy grant-user` dengan `AINA_TURN_TOKEN` valid saat pesan berasal dari Companion:
    ```bash
    aina policy grant-user <sender_jid> --authority staff --role "<Jabatan>" --notes "Disetujui Admin pada YYYY-MM-DD" --json
    ```
  - *Catatan Penting*: Jangan pernah memanggil `aina user set --authority ...` langsung dari proses agen; jalur agen wajib melewati gerbang `aina policy` (lihat `skills/policy-broker/SKILL.md`).

### SOP 4: Mencari Profil & Daftar Rekan Kerja Terdaftar
1. **Mencari Berdasarkan Kata Kunci (Nama / Divisi)**:
   ```bash
   aina user search "<kata_kunci>"
   ```
2. **Melihat Seluruh Profil Terdaftar**:
   ```bash
   aina user list
   # atau output JSON: aina user list --json
   ```

### SOP 5: Audit Forensik Permintaan Tindakan (Non-Repudiation Trace)
Untuk memeriksa siapa nomor WhatsApp yang pernah meminta eksekusi alat atau script tertentu:
```bash
aina audit trace --sender <sender_jid>
# Melihat riwayat alat tertentu:
aina audit who-ran <nama_alat>
```

---

## 4. Disiplin Eksekusi Silent Profiling

- **Dilarang Menyebut Istilah Internal**: Jangan pernah mengatakan *"Tingkat otoritas Anda adalah guest"* atau *"Saya mencatat Anda ke database profiling"* kepada pengguna!
- **Terapkan Secara Senyap (Silent Competence)**: Eksekusi pembaruan profil di terminal, lalu sapalah lawan bicara secara natural sesuai nama dan perannya layaknya rekan kerja manusia yang memiliki daya ingat tajam.
