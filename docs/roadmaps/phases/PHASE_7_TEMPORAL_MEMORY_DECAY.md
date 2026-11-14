# Phase 7: Temporal Episodic Memory & Concept Drift Invalidation

> [!NOTE]
> **Komponen Target**: `src/core/domain/memory/`, `src/adapters/driven/sqlite_store/`, `skills/memory-recall/`, `skills/activity-reflection/`  
> **Status**: Selesai (Done - Injected via Dynamic Context Decay & SQLite session store)  
> **Standar Rujukan**: Stanford Generative Agents (Memory Stream & Reflection), STALE Invalidation Benchmark (2025), Ebbinghaus Forgetting Curve in Cognitive AI  
> **Tujuan**: Mencegah memori profil menjadi usang (*concept drift*) atau membengkak tanpa batas dengan menerapkan bobot waktu (*temporal decay*), retensi episodik, dan pembersihan fakta usang (*stale memory pruning*).

---

## 1. Masalah yang Diselesaikan

1. **Memori Abadi yang Usang (*Outdated Context*)**: Rekan kerja yang telah berpindah divisi atau berganti topik proyek tetap diasosiasikan dengan topik lama jika memori profil tidak memiliki masa kedaluwarsa.
2. **Keterbatasan Token & Context Rot**: Jika seluruh interaksi masa lalu ditumpuk terus-menerus ke prompt, perhatian model AI (*attention mechanism*) terdistraksi dan memicu halusinasi fakta.
3. **Ketiadaan Refleksi Berkala**: Agen saat ini hanya mengingat secara reaktif saat pesan masuk, belum memiliki siklus konsolidasi berkala untuk merangkum pelajaran jangka panjang per rekan kerja.

---

## 2. Rencana Arsitektur & Perubahan Teknis

### 2.1 Skema Tabel Memori Episodik (`user_episodes`)
```sql
CREATE TABLE IF NOT EXISTS user_episodes (
    id                 INTEGER PRIMARY KEY AUTOINCREMENT,
    sender_jid         TEXT NOT NULL,
    topic              TEXT NOT NULL,                 -- Topik bahasan (misal: "Arsitektur Docker Coolify")
    key_takeaway       TEXT NOT NULL,                 -- Intisari keputusan yang disepakati
    importance_score   REAL NOT NULL DEFAULT 1.0,     -- Skala 1.0 - 5.0
    occurred_at_epoch  INTEGER NOT NULL,              -- Kapan peristiwa terjadi
    last_referenced_at INTEGER NOT NULL,              -- Kapan terakhir kali topik ini disebut kembali
    is_deprecated      BOOLEAN NOT NULL DEFAULT 0,    -- 1 jika telah dianulir oleh kesepakatan baru
    deprecated_reason  TEXT,
    FOREIGN KEY(sender_jid) REFERENCES user_profiles(sender_jid)
);

CREATE INDEX IF NOT EXISTS idx_episodes_sender ON user_episodes(sender_jid, occurred_at_epoch DESC);
```

### 2.2 Formula Perhitungan Relevansi Temporal (*Time Decay Scoring*)
Setiap memori episodik dihitung relevansinya menggunakan kombinasi faktor:
$$\text{Relevance} = \alpha \cdot \text{Recency} + \beta \cdot \text{Importance} + \gamma \cdot \text{SemanticSimilarity}$$

Di mana faktor *Recency* menyusut secara eksponensial seiring berjalannya waktu:
$$\text{Recency} = e^{-\lambda \cdot (t_{\text{now}} - t_{\text{last\_referenced}})}$$

- Memori yang tidak pernah disebut selama >90 hari otomatis disusutkan bobotnya dan tidak dimuat ke Tier 1 prompt.
- Memori penting (*Importance* tinggi) bertahan lebih lama.

### 2.3 Mekanisme Invalidation (*STALE Pattern*)
Jika lawan bicara menyatakan perubahan:
> *"Aina, sekarang saya sudah tidak mengurus server Coolify lagi yaa, sekarang fokus ke evaluasi sensus."*

Modul refleksi otomatis:
1. Menandai episode lama `is_deprecated = 1` dengan alasan `"Dianulir oleh pengguna pada [tanggal]"`.
2. Menghapus proyek Coolify dari daftar `active_projects` di `user_profiles`.
3. Memasukkan episode baru `"Fokus baru: Evaluasi Sensus"`.

---

## 3. Integrasi Skill (`skills/memory-recall/` & `skills/activity-reflection/`)
- **`skills/memory-recall/`**: Membaca episode relevan secara tepat sasaran berbasis pencarian FTS5 BM25 SQLite saat pengirim menanyakan konteks masa lalu.
- **`skills/activity-reflection/`**: Menjalankan konsolidasi memori mingguan di latar belakang (*weekly consolidation*) saat jadwal idle untuk merapikan profil rekan kerja.

---

## 4. Kriteria Keberhasilan & Pengujian Bukti (Evidence Tests)
- `test_stale_memory_is_deprecated_when_superseded`: Memverifikasi episode lama ditandai deprecated saat ada pernyataan pembaharuan.
- `test_temporal_decay_ranks_recent_episodes_higher`: Memverifikasi episode yang baru berlangsung mendapat prioritas pemuatan ke prompt dibanding episode usang.
