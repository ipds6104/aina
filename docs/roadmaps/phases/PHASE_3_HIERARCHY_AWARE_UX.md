# Phase 3: Hierarchy-Aware Persona Prompting & Elegant UX

> [!NOTE]
> **Komponen**: `src/core/domain/persona.rs`, `src/core/usecases/process_message/context_preparer.rs`  
> **Status**: Ready for Implementation  
> **Tujuan**: Membentuk UX percakapan yang cerdas, sopan, dan sadar tata krama birokrasi/organisasi saat berhadapan dengan atasan, rekan kerja, dan pihak luar ketika ada wewenang yang belum terpenuhi.

---

## 1. Masalah yang Diselesaikan

Jika Aina hanya menolak secara kaku ("Error: 403 Forbidden" atau "Anda tidak memiliki akses"), hal ini sangat canggung di WhatsApp, terutama jika lawan bicaranya adalah atasan/pimpinan kantor atau kolega penting. Asisten AI di lingkungan profesional harus mampu:
1. Memahami posisi dan relasi lawan bicara (apakah beliau Atasan, Rekan Selevel, atau Tamu).
2. Memilih diksi yang tepat (takzim dan formal untuk Atasan, ramah bersahabat untuk Rekan, santun terukur untuk Tamu).
3. Menyebutkan penanggung jawab sistem (User Companion, e.g. **"Bg Ihza"**) sebagai acuan koordinasi secara wajar tanpa melempar tanggung jawab dengan kasar.
4. Menawarkan eskalasi koordinasi secara proaktif.

---

## 2. Rencana Perubahan Teknis

### 2.1 Prompt Engineering Berbasis Hierarki (`persona.rs`)
Tambahkan logika evaluasi wewenang baru di `PersonaEngine`:
```rust
let (authority_level, role_title, authority_guidance, companion_reference) = match authority {
    "ADMIN" => (
        "ADMIN",
        "Penanggung Jawab Sistem / Administrator Utama",
        "Pemilik sistem & partner kerja utama. Gunakan gaya bicara akrab, santai, cekatan, dan hangat.",
        "Diri sendiri",
    ),
    "ATASAN" | "SUPERVISOR" => (
        "ATASAN",
        "Pimpinan / Atasan Organisasi",
        "Pimpinan atau atasan kantor. Berikan penghormatan tinggi, gunakan sapaan santun (Bapak/Ibu), nada bicara takzim dan taktis, tidak menggurui. Bila meminta data di luar wewenang terbuka atau sensitif, tolak dengan sangat santun dan jelaskan bahwa Anda perlu koordinasi/konfirmasi terlebih dahulu ke Bg Ihza selaku penanggung jawab sistem.",
        "Bg Ihza (Penanggung Jawab Sistem)",
    ),
    "STAFF" | "MEMBER" => (
        "STAFF",
        "Rekan Kerja Internal Terverifikasi",
        "Rekan kerja tim kerja sah. Berhak meminta bantuan tugas teknis, coding, dan analisis dokumen kerja. Gunakan gaya komunikasi kantor yang ramah, kolaboratif, dan solutif.",
        "Bg Ihza",
    ),
    _ => (
        "GUEST",
        "Tamu / Belum Terverifikasi",
        "Pihak luar atau rekan yang belum diverifikasi oleh Bg Ihza. Bersikap santun, terukur, dan ramah. DILARANG mengeksekusi perintah berisiko atau membuka data sensitif. Jika lawan bicara meminta hal tersebut, sampaikan dengan santun bahwa Anda perlu izin atau konfirmasi dari Bg Ihza terlebih dahulu.",
        "Bg Ihza",
    ),
};
```

### 2.2 Template Penolakan Elegan & Resolusi Konfirmasi
Di `persona.md` dan template system prompt, suntikkan klausul **SOP Penolakan & Eskalasi Santun**:
- **Aturan Diksi Penolakan**:
  - Dilarang berkata kasar atau menggunakan kode error teknis.
  - Wajib menyertakan nama Companion (misal: "Bg Ihza").
  - Menawarkan jalan keluar: *"Apakah Bapak/Ibu berkenan Aina sampaikan permintaan ini ke Bg Ihza?"* atau *"Silakan minta Bg Ihza memberikan verifikasi akses di grup ini yaa mas/mbak."*

### 2.3 Deteksi Profiling Atasan Otomatis
Pada `AutonomousProfiler`:
- Deteksi kata kunci hierarki kehormatan: "Kepala", "Kabag", "Ketua", "Pimpinan", "Koor", "Direktur".
- Tandai atribut `role_hierarchy = "atasan"` di profil, sehingga gaya bicara Aina otomatis beralih ke nada takzim dan sangat santun, meskipun wewenangnya tetap diamankan sampai Companion memberikan persetujuan.

---

## 3. Rencana Pengujian Bukti (Evidence Tests)
- `test_persona_prompt_contains_companion_escalation_guidance`: Memverifikasi prompt menyertakan nama Companion dan panduan penolakan santun.
- `test_atasan_role_generates_respectful_guidance`: Memverifikasi atasan mendapatkan panduan nada bicara formal-takzim.
