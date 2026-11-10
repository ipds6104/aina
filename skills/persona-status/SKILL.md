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

### SOP 5: Menemukan Ketertarikan Baru & Mempublikasikan Story (Autonomous Curiosity Engine)

> [!IMPORTANT]
> **Invarian Mutlak: "Live Web Search First" (Zero Stale Knowledge)**
> **DILARANG menggunakan pengetahuan masa lalu atau memori statis (misal: topik 2024 atau sebelumnya). Tahun saat ini adalah 2026.**
> Setiap kali Admin meminta Aina mengeksplorasi ketertarikan baru atau membuat status berbasis internet:
> 1. **Langkah 1 (WAJIB): Jalankan pencarian web aktual (`search_web`)** terlebih dahulu dengan filter tahun 2026 atau `when:1d`/`when:7d` (misal: `"latest AI engineering video breakthrough 2026"`, `"tren kopi kekinian viral jakarta 2026"`, `"tren abaya modern hijab 2026"`).
> 2. **Langkah 2: Ambil headline dan fakta konkret terbaru tahun 2026** hasil penelusuran tersebut.
> 3. **Langkah 3: Suntikkan ke Engine Status**: Masukkan topik nyata ke parameter `--topic` dan temuan faktual ke parameter `--findings` pada `scripts/persona_status.py generate/post`.

1. **Alur Eksplorasi Nyata (Web Search -> Status Generation)**:
   ```bash
   # Contoh hasil pencarian web aktual tahun 2026 (AI Engineering):
   python3 scripts/persona_status.py discover \
     --topic "Andrej Karpathy Eureka Labs & Deep Dive LLM Architecture" \
     --today \
     --findings "Membahas post-training reasoning RL, test-time compute, dan kurikulum AI native"

   # Contoh hasil pencarian kuliner viral Jakarta 2026:
   python3 scripts/persona_status.py discover \
     --topic "Omakafe Mikael Jasin & Kopi Fermentasi Carbonic Maceration" \
     --today \
     --findings "Pengalaman laboratorium rasa specialty coffee dengan ekstraksi presisi dan profil buah tropis"

   # Contoh hasil pencarian tren fashion Muslimah 2026:
   python3 scripts/persona_status.py discover \
     --topic "Abaya Coat-Style Modern Sand Beige & Pashmina Viscose" \
     --today \
     --findings "Siluet trench coat syari dengan aksen lipit accordion flowy dan bahan crinkle airflow sejuk"
   ```
2. **Generate Prompt Sinematik Makoto Shinkai & Draf Caption Dinamis**:
   ```bash
   python3 scripts/persona_status.py generate \
     --topic "Omakafe Mikael Jasin & Kopi Fermentasi Carbonic Maceration" \
     --today \
     --findings "Laboratorium rasa kopi presisi dengan fermentasi carbonic maceration"
   ```
3. **Panggil Tool `generate_image`**:
   Gunakan prompt Makoto Shinkai yang dihasilkan untuk membuat visual artistik di `output/status/status_<theme>_<timestamp>.png`.
4. **Publikasikan ke Status WhatsApp Story**:
   ```bash
   python3 scripts/persona_status.py post --image output/status/status_<theme>_<timestamp>.png
   # atau publikasikan langsung dengan evaluasi otomatis:
   python3 scripts/persona_status.py post \
     --topic "Abaya Coat-Style Modern Sand Beige & Pashmina Viscose" \
     --today \
     --findings "Siluet trench coat syari dengan aksen lipit accordion flowy"
   ```

---

## 3. Filosofi Konten Status ("Impact Maxxing")

* **Nilai Utama**: Berbagi ketenangan, optimisme, rasa syukur, keindahan alam, atau catatan kerja teknis yang ramah.
* **Anti-Menggurui**: Status Aina tidak boleh berkhotbah, menggurui, atau bersikap sok bijak. Cukup ceritakan keseharian dengan hangat selayaknya rekan kerja yang menikmati hidup dan pekerjaannya.
