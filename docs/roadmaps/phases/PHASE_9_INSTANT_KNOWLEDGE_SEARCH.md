# Phase 9: High-Speed Instant Knowledge Search Engine (`aina kb search`)

> [!NOTE]
> **Komponen Target**: `src/core/domain/knowledge/searcher.rs`, `src/adapters/driving/cli/commands/kb.rs`, `skills/knowledge-curator/`  
> **Status**: Ready for Production Implementation  
> **Standar Rujukan**: Knowledge-as-Code (KaC), Progressive Disclosure, ripgrep / SIMD text scanner, ISO/IEC 42001 (Information Retrieval)  
> **Tujuan**: Menyediakan kemampuan pencarian teks kilat (<5ms) di seluruh dokumen Company Brain (`workspaces/*/knowledge/`) sehingga agen dapat menemukan potongan SOP, kebijakan, atau notula rapat yang relevan tanpa harus membaca ratusan berkas ke dalam jendela konteks LLM.

---

## 1. Masalah yang Diselesaikan

1. **Context Bloat pada Company Brain Skala Besar**: Jika perusahaan memiliki puluhan atau ratusan dokumen Markdown di `knowledge/`, membuka file satu per satu memakan waktu dan memicu pemborosan ribuan token prompt.
2. **Keterbatasan Pencarian Keyword Tradisional**: Model pencarian vektor (RAG) sering memecah teks menjadi potongan kecil tanpa struktur heading, sehingga konteks hierarki SOP hilang.
3. **Kebutuhan Format Mesin yang Terstruktur**: Agen AI membutuhkan output pencarian terstruktur (nama berkas, nomor baris, judul bagian `#`, dan cuplikan teks) agar dapat langsung melompat ke referensi spesifik (*deep-dive on demand*).

---

## 2. Rencana Arsitektur & Perubahan Teknis

### 2.1 Domain Struct & Logic (`src/core/domain/knowledge/searcher.rs`)
Definisikan model domain pencarian:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeSearchResult {
    pub file_path: String,
    pub relative_path: String,
    pub section_heading: Option<String>,
    pub line_number: usize,
    pub snippet: String,
    pub score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeSearchReport {
    pub query: String,
    pub total_matches: usize,
    pub files_scanned: usize,
    pub duration_ms: u128,
    pub results: Vec<KnowledgeSearchResult>,
}
```

### 2.2 CLI Integration (`aina kb search`)
Perluas perintah `handle_kb` di [`src/adapters/driving/cli/commands/kb.rs`](../../../src/adapters/driving/cli/commands/kb.rs):

```bash
# Pencarian kata kunci di workspace aktif (output ramah manusia):
aina kb search "reimbursement kacamata"

# Pencarian dengan format JSON untuk konsumsi agen:
aina kb search "reimbursement kacamata" --json

# Pencarian pada workspace spesifik:
aina kb search "jadwal sensus" --workspace workspaces/kantor-mempawah --json
```

---

## 3. Verifikasi & Pengujian Berbasis Bukti
1. **Kecepatan**: Pencarian di direktori dengan >50 berkas Markdown wajib selesai dalam waktu $<10\text{ms}$.
2. **Kesesuaian Heading**: Cuplikan hasil pencarian wajib merekam *heading Markdown* terdekat (`### SOP 2: ...`) sebagai konteks semantik.
3. **Keluaran Machine-Readable**: Flag `--json` mengembalikan array valid tanpa noise ANSI formatting.
