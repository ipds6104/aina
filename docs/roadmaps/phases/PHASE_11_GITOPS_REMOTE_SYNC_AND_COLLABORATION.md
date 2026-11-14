# Phase 11: GitOps Remote Synchronization & Collaborative Living Brain (`aina kb sync`)

> [!NOTE]
> **Komponen Target**: `src/core/domain/knowledge/git_sync.rs`, `src/adapters/driving/cli/commands/kb.rs`, `src/core/usecases/scheduled_tick/`  
> **Status**: Ready for Implementation  
> **Standar Rujukan**: GitOps Living Documentation, Context-as-Code (CaC), "AI Proposes, Human Merges" Paradigm  
> **Tujuan**: Menghubungkan direktori `workspaces/<company>/knowledge` dengan repositori remote Git (GitHub/GitLab) secara dua arah, memungkinkan pembaruan otomatis dokumen dari tim kerja serta mekanisme pembuatan proposal draf PR saat agen menyusun notula atau SOP baru.

---

## 1. Masalah yang Diselesaikan

1. **Keterisolasian Dokumen Lokal**: Workspace saat ini hanya tersimpan di disk server. Jika staf kantor memperbarui SOP di repositori GitHub kantor, Aina tidak mengetahui perubahannya tanpa manual pull.
2. **Risiko Penimpaan Dokumen Kebijakan**: Jika AI langsung menimpa dokumen kebijakan penting di cabang `main`, ada risiko kesalahan interpretasi (*rogue write*).
3. **Kebutuhan Kolaborasi Asinkron**: Organisasi memerlukan pola kolaborasi yang aman di mana AI agen bertindak sebagai penyusun draf dan manusia bertindak sebagai pengesah.

---

## 2. Rencana Arsitektur & Perubahan Teknis

### 2.1 Pola "AI Proposes, Human Merges"
1. **Pembaruan Pasif (Pull Sync)**:
   - Aina secara berkala (atau via webhook GitHub) menarik pembaruan: `git -C workspaces/<ws> pull --rebase`.
   - Menjalankan `KnowledgeEngine::groom_knowledge_base` untuk menyegarkan `index.md`.
2. **Pengusulan Kebijakan Baru (Draft Branch Proposal)**:
   - Jika staf meminta Aina menyusun SOP baru atau notula panjang:
     - Aina membuat cabang baru: `git checkout -b aina/proposal-<slug>`.
     - Menyimpan berkas di `knowledge/kegiatan/.../README.md` atau `knowledge/procedures.md`.
     - Melakukan commit: `git commit -m "docs(kac): propose update for <slug> [skip ci]"`.
     - Mendorong cabang ke remote dan memberikan tautan pembuatan Pull Request kepada staf di chat WhatsApp.

### 2.2 CLI Command (`aina kb sync`)
```bash
# Menarik pembaruan terkini dari remote repositori:
aina kb sync --pull [--workspace <ws>]

# Mendorong draf perubahan proposal ke remote:
aina kb sync --push --branch aina/draf-sop-q4 [--workspace <ws>]
```

---

## 3. Verifikasi & Pengujian Berbasis Bukti
1. **Deteksi Konflik**: Jika terjadi konflik Git saat pull, sistem membatalkan rebase dan mempertahankan kondisi lokal yang bersih tanpa merusak data yang sedang berjalan.
2. **Kemandirian Operasional**: Jika workspace tidak menggunakan Git (folder biasa), perintah `aina kb sync` memberikan pesan informatif tanpa kegagalan fatal (*graceful degradation*).
