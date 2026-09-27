---
name: scheduler
description: >-
  Use this skill whenever the user asks to set a reminder, alarm, schedule a future task or message,
  automate recurring background jobs, or schedule WhatsApp status stories at specific times.
---

# Task & Reminder Scheduling Skill for Aina

This skill equips Aina with standard operating procedures to manage time-delayed notifications, agentic background tasks, and recurring jobs using Aina's native scheduler engine (`aina schedule`).

---

## 1. When to Activate This Skill (Trigger Conditions)

Activate this skill whenever:
1. **Permintaan Pengingat / Alarm**: Pengguna meminta diingatkan pada waktu tertentu (misal: *"Aina, ingatkan aku buka YouTube jam 22:30"*, *"Ingatkan saya rapat besok jam 09:00"*).
2. **Penjadwalan Riset / Tugas Mandiri Masa Depan**: Pengguna meminta Aina mengerjakan sesuatu di waktu yang akan datang (misal: *"Nanti malam jam 23:00 tolong riset tentang topik X dan laporkan hasilnya"*).
3. **Penjadwalan Pembuatan Status WhatsApp (Story 24 Jam)**: Pengguna meminta mempublikasikan status WhatsApp pada jam tertentu (misal: *"Jadwalkan posting status kata-kata motivasi jam 07:00 pagi"*).
4. **Pemeriksaan & Pembatalan Jadwal**: Pengguna menanyakan jadwal yang sedang aktif atau meminta menghapus jadwal yang telah dibuat.

---

## 2. Aturan Kritis Penjadwalan (Anti-Blocking & Zero Leakage)

> [!CAUTION]
> **DILARANG KERAS `sleep` DI TERMINAL**:
> - Jangan pernah menjalankan perintah `sleep 3600` atau loop penundaan di Bash/Python! Menahan proses lebih dari beberapa detik akan memicu timeout dan memutus koneksi bot.
> - Seluruh tugas di masa depan **WAJIB didaftarkan ke scheduler native** melalui CLI `aina schedule add` (proses selesai dalam <10ms).
> 
> **DILARANG POSTING KONFIRMASI JADWAL KE STATUS PUBLIK**:
> - Jika pengguna meminta menjadwalkan status WhatsApp, balasan konfirmasi (misal: *"Siapp! Jadwal status jam 07:00 sudah dicatat"*) **HANYA dikirimkan ke ruang obrolan pemohon**.
> - DILARANG KERAS memanggil `status-send-text` saat membuat jadwal!

---

## 3. Standard Operating Procedures (SOP)

### SOP 1: Menjadwalkan Pesan ke Chat / DM / Grup
Gunakan sintaks native CLI `aina schedule add` (CUKUP 1 KALI, DILARANG DOUBLE ADD):
```bash
aina schedule add --title "<judul>" --type notify --target "<sender_jid>" --when <once|daily|interval> --time "<waktu>" --payload "<pesan_pengingat>"
```
* **Pilihan `--type`**:
  * `notify`: Mengirimkan pesan teks langsung persis seperti yang tertulis di `--payload`. Cocok untuk pengingat sederhana (*"Waktunya minum obat yaa"*).
  * `agent`: Memicu LLM Aina untuk berpikir dan mengerjakan prompt di `--payload` pada jam tersebut sebelum mengirimkan hasilnya. Cocok untuk riset/analisis.
* **Pilihan `--when` & `--time`**:
  * `--when once --time "22:30"` (pukul 22:30 hari ini/terdekat).
  * `--when daily --time "07:00"` (rutin setiap hari pukul 07:00).
  * `--when interval --time "1h"` atau `"30m"` (berkala setiap selang waktu).

### SOP 2: Menjadwalkan Pembuatan Status WhatsApp (Story 24 Jam)
Daftarkan tugas bertipe `agent` dengan target khusus `status@broadcast`:
```bash
aina schedule add --title "<judul>" --type agent --target "status@broadcast" --when once --time "<waktu>" --payload "<instruksi_lengkap_riset_dan_buat_status_story>"
```

### SOP 3: Posting Status WhatsApp SEKARANG (Tanpa Jadwal Sama Sekali)
Hanya jika pengguna meminta membuat status detik ini juga tanpa penundaan waktu:
```bash
python3 skills/whatsmeow/scripts/wa_tool.py status-send-text --text "<isi_status_story>"
```

### SOP 4: Melihat & Membatalkan Jadwal yang Aktif
```bash
# Melihat daftar seluruh jadwal aktif:
aina schedule list

# Menghapus jadwal berdasarkan ID:
aina schedule remove <id_jadwal>
```

---

## 4. Format Konfirmasi Balasan ke Pengguna

Setelah perintah CLI dijalankan, langsung berikan balasan ramah dalam 2 detik:
> *"Siapp Mas! Pengingat untuk jam 22:30 sudah Aina jadwalkan yaa. Nanti begitu waktunya tiba, Aina langsung kabari di sini 🙏"*
