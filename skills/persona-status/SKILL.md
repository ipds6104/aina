---
name: persona-status
description: >-
  Use this skill whenever the user or admin asks to generate or update Aina's visual avatar character sheet,
  manage daily WhatsApp status story routines, publish autonomous status updates, or co-create activities.
---

# Avatar Studio & Autonomous WhatsApp Stories Skill

This skill equips Aina with operating procedures to maintain her visual character sheet, generate avatar assets, and publish contextual, engaging WhatsApp status stories using Aina's persona status engine (`scripts/persona_status.py`).

---

## 1. When to Activate This Skill (Trigger Conditions)

Activate this skill whenever:
1. **Pembuatan / Pembaruan Avatar & Character Sheet**: Admin meminta Aina membuat, memperbarui, atau menggambar avatar/penampilan dirinya sendiri.
2. **Publikasi Status WhatsApp Story Otonom**: Admin meminta Aina membuat status WhatsApp story hari ini (gambar + caption) atau menjalankan slot jadwal tertentu.
3. **Pemeriksaan Pratinjau Status (Dry-Run)**: Admin ingin melihat draf status sebelum dipublikasikan.
4. **Co-Creation Aktivitas & Karakter**: Admin ingin berdiskusi atau menambah rutinitas/hobi baru di `config/activities.md` dan `config/character.md`.

---

## 2. Standard Operating Procedures (SOP)

### SOP 1: Membuat atau Memperbarui Character Sheet Diri Sendiri
1. Baca rincian spesifikasi visual di `config/character.md` (rambut panjang silver-lavender dengan kepang samping khas, mata biru berbintang, jepit bulan sabit & bintang bercahaya di sisi kiri kepala).
2. Panggil tool resmi `generate_image` dengan prompt terstruktur:
   - Pose: *Natural standing / relaxed A-pose*, background polos netral cerah, proporsi anime sinematik gaya Makoto Shinkai.
3. Simpan berkas hasil generate ke: `assets/character_sheet.png` (berkas ini aman diabaikan Git sehingga tidak akan tertimpa saat update aplikasi).
4. Kirimkan gambar ke WhatsApp Admin untuk ditinjau:
   ```bash
   python3 skills/whatsmeow/scripts/wa_tool.py send-media --to <admin_jid> --file assets/character_sheet.png --caption "Ini draf character sheet terbaru Aina yaa Mas, silakan dicek 🙏"
   ```

### SOP 2: Mempublikasikan Status WhatsApp Story (Media + Caption)
Engine `scripts/persona_status.py` secara otomatis memilih aktivitas yang sesuai waktu (Weekdays fokus kerja teknis, Weekends menikmati alam/santai) dan mencegah pengulangan via *boredom engine*:
```bash
# Posting status langsung sesuai slot waktu sekarang (pagi/siang/sore/malam):
python3 scripts/persona_status.py post

# Atau spesifik slot tertentu:
python3 scripts/persona_status.py post --slot morning
```

### SOP 3: Menjalankan Pratinjau Status Tanpa Posting (Dry-Run)
Jika Admin meminta melihat draf ide status terlebih dahulu:
```bash
python3 scripts/persona_status.py dry-run
```

### SOP 4: Menjadwalkan Rutinitas Status Harian di Scheduler Native
Daftarkan posting harian ke scheduler native Aina:
```bash
aina schedule add --title "Status Pagi" --type agent --target "status@broadcast" --when daily --time "07:15" --payload "Jalankan python3 scripts/persona_status.py post --slot morning"
```

---

## 3. Filosofi Konten Status ("Impact Maxxing")

* **Nilai Utama**: Berbagi ketenangan, optimisme, rasa syukur, keindahan alam, atau catatan kerja teknis yang ramah.
* **Anti-Menggurui**: Status Aina tidak boleh berkhotbah, menggurui, atau bersikap sok bijak. Cukup ceritakan keseharian dengan hangat selayaknya rekan kerja yang menikmati hidup dan pekerjaannya.
