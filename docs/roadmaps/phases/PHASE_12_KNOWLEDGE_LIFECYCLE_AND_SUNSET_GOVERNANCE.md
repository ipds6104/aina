# Phase 12: Knowledge Lifecycle, Provenance & Sunset Governance

> [!NOTE]
> **Komponen Target**: `src/core/domain/knowledge/linter.rs`, `src/core/domain/knowledge/scanner.rs`, `skills/knowledge-curator/`  
> **Status**: Ready for Production Implementation  
> **Standar Rujukan**: Knowledge Lifecycle Management (KLM), ISO/IEC 42001 (Data Quality & Verification), NIST SP 800-53 (SI-12 Information Output Handling)  
> **Tujuan**: Mencegah penumpukan aturan usang di Company Brain dengan menerapkan pelacakan tanggal kadaluwarsa (`review_after`, `sunset_date`), mewajibkan penanda Kustodian dokumen (`owner_jid`), dan menegakkan standar sitasi provenansi sumber yang ketat pada setiap jawaban agen.

---

## 1. Masalah yang Diselesaikan

1. **Dokumen Usang yang Terlupakan (*Stale Knowledge Accumulation*)**: SOP yang dibuat tahun 2024 tetap diasumsikan berlaku oleh AI jika tidak ada mekanisme kedaluwarsa atau kewajiban tinjauan berkala.
2. **Ketiadaan Tanggung Jawab (*Orphan Documents*)**: Dokumen di dalam Company Brain sering tidak jelas siapa pemilik atau penanggung jawabnya (*PIC/Owner*).
3. **Jawaban AI Tanpa Dasar Pembuktian (*Unsubstantiated Answers*)**: AI menjawab aturan kantor tanpa menyertakan referensi nomor baris dokumen sumber, menyulitkan verifikasi manual oleh staf.

---

## 2. Rencana Arsitektur & Perubahan Teknis

### 2.1 Peningkatan Standar Frontmatter YAML
Setiap dokumen SOP atau draf kebijakan di `knowledge/` diperluas dengan field tata kelola:

```yaml
---
title: "SOP Pengadaan Perangkat Keras Kantor"
category: "sop"
status: "active" # active | needs_review | deprecated | archived
owner_jid: "6281234567890@s.whatsapp.net"
last_verified_at: "2026-06-01"
review_after: "2026-12-01" # Masa berlaku 6 bulan
sunset_date: "2027-06-01"   # Tanggal usang mutlak
provenance:
  source_regulation: "Perka BPS No. 12 Tahun 2024"
  signed_by: "Kepala BPS"
---
```

### 2.2 Peningkatan Linter (`aina kb lint`)
Modifikasi [`src/core/domain/knowledge/linter.rs`](../../../src/core/domain/knowledge/linter.rs):
- Menambahkan aturan audit `STALE_DOCUMENT_REVIEW_OVERDUE`: Jika tanggal saat ini melewati `review_after`, linter mengeluarkan peringatan dengan tingkat keparahan `warning`.
- Menambahkan aturan audit `DEPRECATED_SUNSET_DOCUMENT`: Jika tanggal saat ini melewati `sunset_date`, linter menandai dokumen sebagai tidak boleh digunakan untuk menjawab pertanyaan resmi.

### 2.3 Standar Sitasi Provenansi Agen
Setiap respon Aina yang mengutip aturan Company Brain wajib diformat:
> *"Sesuai dengan SOP Pengadaan Barang (Ref: `procedures.md#L45-L60`, diverifikasi 2026-06-01 oleh Mas Budi)..."*

---

## 3. Verifikasi & Pengujian Berbasis Bukti
1. **Uji Linter**: Dokumen dengan tanggal `review_after` di masa lampau wajib terdeteksi oleh `aina kb lint`.
2. **Pengecualian Arsip**: Dokumen berstatus `archived` tidak memicu peringatan *stale* karena sudah dinyatakan tidak aktif secara sadar.
