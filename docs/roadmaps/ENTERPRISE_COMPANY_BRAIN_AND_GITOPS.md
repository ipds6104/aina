# Roadmap: Enterprise Company Brain, Knowledge-as-Code (KaC) & GitOps Governance

> [!IMPORTANT]
> **Status Dokumen**: Active Canonical Architecture Roadmap  
> **Komponen Target**: `aina` Core Knowledge Engine (`src/core/domain/knowledge/`), Workspace Management (`workspaces/`), Asset Custodianship (`knowledge/manifests/`), CLI `aina kb` & `aina asset`, Skills (`knowledge-curator`, `policy-broker`)  
> **Referensi Induk**: Ditautkan langsung dari [`GEMINI.md`](../../GEMINI.md#6-company-brain-knowledge-as-code--discretionary-asset-custodianship)  
> **Standar Kepatuhan**: Knowledge-as-Code (KaC), GitOps Living Documentation, NIST SP 800-53 (AC-3, MP-2), ISO/IEC 42001:2023, UU No. 27 Tahun 2022 (UU PDP)  
> **Versi**: 1.0 (November 2026)

---

## 1. Ringkasan Eksekutif & Latar Belakang

Dalam organisasi enterprise dan instansi modern (seperti BPS), aset intelektual perusahaan terakumulasi dalam bentuk **Company Brain**—kumpulan dokumen SOP, notula rapat, draf kebijakan, arsitektur sistem, dan dataset analitis.

Pendekatan lama yang mengandalkan **wiki statis yang terpisah (Confluence/Notion)** atau **database vektor hitam (*black-box Vector RAG*)** memiliki kelemahan kritis:
1. **Pembusukan Dokumentasi (*Documentation Rot*)**: Dokumen tidak pernah diperbarui seiring berjalannya kode dan keputusan nyata di lapangan.
2. **Context Bloat & Token Inefficiency**: Membaca dokumen tebal ratusan halaman ke jendela konteks LLM memicu fenomena *Lost-in-the-Middle* dan memboroskan biaya token.
3. **Ketiadaan Tata Kelola Kepemilikan (*No Ownership & Leakage*)**: Berkas rahasia yang dititipkan staf sering bocor atau dapat diakses bebas oleh siapa saja tanpa mekanisme persetujuan pemilik data (*Data Custodian*).

Roadmap ini menetapkan arsitektur **Knowledge-as-Code (KaC)** di mana dokumen perusahaan dikelola selayaknya kode sumber (berversi Git, berformat teks murni Markdown + YAML), dirawat secara deterministik oleh AI Agent sebagai *Automated Knowledge Gardener*, dan dilindungi oleh gerbang *Discretionary Access Control (DAC)*.

---

## 2. Empat Pilar Arsitektur Company Brain & GitOps

```mermaid
flowchart TD
    subgraph Pilar1["Pilar 1: High-Speed Instant Retrieval"]
        A["Ratusan Berkas Markdown di workspaces/*/knowledge/"] --> B["Fast In-Memory / FTS5 Regex Scanner"]
        B --> C["aina kb search: <5ms snippet & heading retrieval"]
        C --> D["Progressive Disclosure: Zero Context Waste"]
    end

    subgraph Pilar2["Pilar 2: Discretionary Asset Custodianship"]
        E["Berkas Masuk: PDF / Docx / Parquet / Script"] --> F["Asset Custodian Binding: custodian_jid"]
        F --> G["Gerbang Zero-Disclosure & Approval Flow"]
        G --> H["CLI Operasional: aina asset list / grant / revoke"]
    end

    subgraph Pilar3["Pilar 3: GitOps & Living Knowledge"]
        I["Repositori Remote Git (GitHub/GitLab)"] --> J["Dual-Direction Sync (aina kb sync)"]
        J --> K["AI Proposes, Human Merges: Branch & Draft PR"]
    end

    subgraph Pilar4["Pilar 4: Anti-Entropy & Lifecycle Governance"]
        L["Metadata Dokumen: review_after / sunset_date"] --> M["Deterministic Linter (aina kb lint --auto-heal)"]
        M --> N["Peringatan Dokumen Basi & Auto-Catalog Grooming"]
    end
```

---

## 3. Matriks Rencana Implementasi Bertahap (Phased Roadmap)

Roadmap disusun dalam 4 tahapan modular yang saling melengkapi (melanjutkan Fase 1–8 sebelumnya):

| Fase | Dokumen Rencana | Sasaran Utama | Komponen Target | Status |
|:---|:---|:---|:---|:---:|
| **Fase 9** | [`PHASE_9_INSTANT_KNOWLEDGE_SEARCH.md`](phases/PHASE_9_INSTANT_KNOWLEDGE_SEARCH.md) | Mesin pencarian teks kilat berkecepatan tinggi (<5ms) di seluruh Company Brain tanpa memuat seluruh berkas ke konteks LLM. | `src/core/domain/knowledge/searcher.rs`, CLI `aina kb search` | **Produksi (Ready)** |
| **Fase 10** | [`PHASE_10_ASSET_AND_FILE_CUSTODIANSHIP_CLI.md`](phases/PHASE_10_ASSET_AND_FILE_CUSTODIANSHIP_CLI.md) | Antarmuka CLI native dan verifikasi audit untuk kepemilikan berkas terbatas (`RESTRICTED`), izin kustodian, dan daftar pemegang akses. | `src/adapters/driving/cli/commands/asset.rs`, `knowledge/manifests/` | **Produksi (Ready)** |
| **Fase 11** | [`PHASE_11_GITOPS_REMOTE_SYNC_AND_COLLABORATION.md`](phases/PHASE_11_GITOPS_REMOTE_SYNC_AND_COLLABORATION.md) | Integrasi Git remote dua arah (*git pull/push*), pola kolaborasi *AI Proposes via Branch / Draft PR*, dan auto-sync background. | `src/core/domain/knowledge/git_sync.rs`, CLI `aina kb sync` | **Desain (Ready)** |
| **Fase 12** | [`PHASE_12_KNOWLEDGE_LIFECYCLE_AND_SUNSET_GOVERNANCE.md`](phases/PHASE_12_KNOWLEDGE_LIFECYCLE_AND_SUNSET_GOVERNANCE.md) | Tata kelola masa berlaku SOP (`review_after`, `sunset_date`), pencegahan aturan usang, dan sitasi provenansi ketat. | `src/core/domain/knowledge/linter.rs`, `skills/knowledge-curator` | **Produksi (Ready)** |

---

## 4. Sinergi dengan Ekosistem Skill Agen

Penyempurnaan ini terhubung langsung ke skill resmi Aina:

1. **`skills/knowledge-curator/` ([SKILL.md](../../skills/knowledge-curator/SKILL.md))**:
   - Memanfaatkan `aina kb search` untuk menemukan paragraf SOP spesifik dalam hitungan milidetik.
   - Menjalankan SOP 6 (File & Asset Custodianship) menggunakan CLI `aina asset`.
2. **`skills/policy-broker/` ([SKILL.md](../../skills/policy-broker/SKILL.md))**:
   - Menerjemahkan niat delegasi izin berkas (*"Aina, izinkan Budi baca laporan Q3"*) menjadi eksekusi `aina asset grant`.
3. **`skills/self-observability/` ([SKILL.md](../../skills/self-observability/SKILL.md))**:
   - Memantau kebersihan indeks dan melaporkan dokumen yang mendekati tanggal kadaluwarsa (*sunset date*).

---

## 5. Verifikasi Berbasis Bukti (*Evidence-First Verification*)

Sesuai filosofi *Old Coder*:
1. **Uji Kecepatan Pencarian**: Kueri `aina kb search` wajib mengembalikan hasil relevan dalam waktu $<10\text{ms}$ pada repositori dengan ratusan berkas.
2. **Uji Isolasi Kustodian**: Berkas berstatus `RESTRICTED` tidak boleh dibocorkan ke nomor yang tidak terdaftar di `authorized_users`.
3. **Uji Linter & Auto-Healing**: Perubahan berkas dokumen baru otomatis tercermin di `knowledge/index.md` tanpa kesalahan format.
