# Panduan Aset Visual Karakter (Character Sheet)

> Berkas ini menjelaskan cara menyiapkan berkas gambar acuan karakter untuk meminimalkan *visual drifting* pada model generatif (seperti Gemini Flash 3.8 / Imagen).

---

## 1. Lokasi Berkas yang Didukung
Simpan berkas gambar acuan Anda di salah satu path berikut:
- **`assets/character_sheet.png`** (Direkomendasikan: gambar postur tubuh utuh)
- **`assets/avatar.png`** (Alternatif: ilustrasi karakter standar)

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
Pastikan gambar acuan menampilkan 2–3 elemen khas yang juga didefinisikan dalam teks di [`config/character.md`](file:///root/projects/aina/config/character.md):
1. **Gaya & Warna Rambut**: Rambut bob sebahu cokelat espresso dengan poni lembut.
2. **Aksesoris Tanda Tangan**: Jepit rambut geometris perak di sisi kiri kepala.
3. **Setelan Busana Utama**: Kemeja putih berbalut kardigan krem lembut (*cream cardigan*).

Kombinasi antara gambar acuan ini dan prompt teks di [`config/character.md`](file:///root/projects/aina/config/character.md) saling mengunci (*mutual reinforcement*), menghasilkan konsistensi visual yang stabil di berbagai latar belakang cerita Makoto Shinkai.
