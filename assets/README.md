# Panduan Aset Visual Karakter (Character Sheet)

> Berkas ini menjelaskan cara menyiapkan berkas gambar acuan karakter untuk meminimalkan *visual drifting* pada model generatif (seperti Gemini Flash 3.8 / Imagen).

---

## 1. Lokasi Berkas yang Didukung & Mekanisme Fallback
Sistem memeriksa berkas acuan dengan urutan prioritas berikut:
1. **`assets/character_sheet.png`** (Kustom Pengguna: prioritas tertinggi, diabaikan oleh `.gitignore` sehingga tidak akan pernah tertimpa saat update / `git pull`).
2. **`assets/avatar.png`** (Kustom Pengguna: foto profil kustom alternatif).
3. **`assets/character_sheet.default.png`** (Bawaan Template Repo: berkas master acuan bawaan jika pengguna belum menaruh berkas kustom sendiri).

---

## 2. Riset & Best Practice Gambar Acuan (Anti-Visual Drifting)

Berdasarkan riset karakteristik model difusi dan VLM multimodal (Gemini / Imagen):

| Aspek | Yang Kurang Efektif (Rentan Drifting) | Yang Sangat Efektif (Stabil & Konsisten) |
| :--- | :--- | :--- |
| **Bentuk Pose** | **T-Pose Kaku**: Model difusi mengasosiasikan T-pose dengan *3D untextured wireframe mesh*, sehingga output sering kaku seperti patung plastik. | **Relaxed A-Pose / Natural Standing (Sudut 3/4)**: Berdiri santai menghadap sedikit serong (3/4 angle), tangan santai di samping tubuh, memperlihatkan siluet utuh dari kepala hingga sepatu. |
| **Cakupan Tubuh** | **Hanya Foto Profil / Headshot (Wajah Saja)**: Model "menebak" dan menghalusinasikan bentuk badan, pakaian, dan tinggi tubuh di setiap status. | **Full-Body Shot / Concept Art Sheet**: Menampilkan proporsi tubuh lengkap, panjang rambut dari depan/samping, dan setelan busana khas. |
| **Latar Belakang** | **Background Ramai / Penuh Warna**: Detail background sering ikut merembes (*bleed*) ke dalam gambar baru. | **Background Polos Bersih**: Warna putih polos, abu-abu muda netral, atau transparan agar model hanya mengisolasi fitur karakter. |
| **Pencahayaan** | **Shadow Kuat / Bayangan Gelap Ekstrem**: Mengaburkan detail pakaian dan warna rambut asli. | **Pencahayaan Studio Rata (*Flat Soft Lighting*)**: Warna pakaian, kulit, dan rambut terlihat jelas tanpa bayangan yang menutupi detail. |

---

## 3. Jangkar Visual Tanda Tangan (*Signature Anchors*)
Pastikan gambar acuan menampilkan elemen khas yang juga didefinisikan dalam teks di [`config/character.md`](file:///root/projects/aina/config/character.md):
1. **Gaya & Warna Rambut**: Rambut panjang bergelombang warna *silver-lavender* lembut dengan kepang samping khas (*signature side braid*).
2. **Mata & Wajah**: Mata biru malam berbintang (*starry deep blue eyes*), ekspresi ramah dan hangat.
3. **Aksesoris Tanda Tangan**: Jepit rambut bulan sabit & bintang bercahaya (*crescent moon and glowing star hair clip*) di sisi kiri kepala.
4. **Setelan Busana Utama**: Menyesuaikan konteks aktivitas (lihat Wardrobe Matrix di `config/character.md`) dengan sentuhan palet celestial/lembut yang senada.

Kombinasi antara gambar acuan ini dan prompt teks di [`config/character.md`](file:///root/projects/aina/config/character.md) saling mengunci (*mutual reinforcement*), menghasilkan konsistensi visual yang stabil di berbagai latar belakang cerita Makoto Shinkai.
