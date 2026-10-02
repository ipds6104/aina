---
name: memory-recall
description: >-
  Use this skill whenever you need to search or recall previous WhatsApp conversations,
  user statements, past technical decisions, project configurations, or context that is not
  present in the current immediate prompt.
---

# Episodic Memory Recall Skill for Aina

This skill equips Aina with on-demand episodic memory retrieval capabilities. Instead of relying on a monolithic context window filled with past conversations, Aina can query her local SQLite message archive on-demand.

---

## 1. When to Activate This Skill (Trigger Conditions)

Activate this skill whenever:
1. **Pengguna Menanyakan Konteks Masa Lalu**: Pertanyaan seperti *"Kemarin kita bahas soal apa ya?"*, *"Ingat port database yang kemarin?"*, atau *"Apa nama tool yang tadi kamu rekomendasikan?"*.
2. **Klarifikasi atau Verifikasi Fakta**: Perlu memastikan pernyataan, janji, atau instruksi sebelumnya yang diberikan lawan bicara.
3. **Pencarian Riwayat Percakapan Spesifik**: Lawan bicara meminta melacak obrolan tanggal tertentu atau topik tertentu.

---

## 2. Standard Operating Procedures (SOP)

### SOP 1: Mencari Riwayat Percakapan Berdasarkan Kata Kunci
Gunakan script `recall.py search`:
```bash
python3 skills/memory-recall/scripts/recall.py search "<kata_kunci>" [--chat "<chat_jid>"] [--limit 5]
```
Contoh:
```bash
python3 skills/memory-recall/scripts/recall.py search "coolify" --limit 3
python3 skills/memory-recall/scripts/recall.py search "database migration"
```
*Sistem akan mencocokkan kata kunci signifikan di tabel riwayat percakapan SQLite dan mengembalikan potongan pesan yang relevan beserta stempel waktu dan pengirimnya.*

### SOP 2: Membaca Percakapan Paling Terakhir dari Pengirim
Jika pengguna merujuk ke pesan sebelumnya tanpa kata kunci spesifik (misal: *"Ingat gak apa kataku tadi?"*):
```bash
python3 skills/memory-recall/scripts/recall.py recent [--chat "<chat_jid>"] [--limit 5]
```

### SOP 3: Menyusun Jawaban Berdasarkan Hasil Temu Balik (Memory Synthesis)
1. Periksa stempel waktu (`created_at`) dan pengirim (`is_from_me`).
2. Jawab pertanyaan pengguna secara lugas, natural, dan presisi (misal: *"Berdasarkan catatan obrolan kita kemarin, port Coolify yang kita tentukan adalah 8000 yaa mas"*).
3. Jangan menyalin ulang seluruh format mentah log jika tidak diminta; sintesiskan fakta intinya saja.
