# Phase 5: Structured Semantic Profiling & Dynamic Preferences

> [!NOTE]
> **Komponen Target**: `src/core/domain/profiler.rs`, `src/adapters/driven/sqlite_store/schema.rs`, `src/core/ports/session_store.rs`, `skills/user-profiler/`  
> **Status**: Selesai (Done - Implemented in `src/core/domain/persona.rs`, `src/adapters/driven/sqlite_store/session.rs`, and `skills/user-profiler/`)  
> **Standar Rujukan**: Stanford Generative Agents (Semantic Layer), MemGPT/Letta Structured Personas, PersonaMem (NeurIPS 2025/2026)  
> **Tujuan**: Mengubah profiling lawan bicara dari catatan teks datar (*flat string*) menjadi schema semantik terstruktur (gaya komunikasi, keahlian teknis, preferensi format, dan proyek aktif) tanpa mengorbankan isolasi wewenang keamanan.

---

## 1. Masalah yang Diselesaikan

Pada implementasi saat ini:
1. Kolom `notes` di tabel `user_profiles` hanya berupa akumulasi teks bebas (`"Preferensi panggilan: Mas Budi; Identifikasi peran: Tim IT"`).
2. Jika lawan bicara memiliki banyak preferensi (misal: gaya penjelasan singkat, lebih suka kode Python, alergi tabel, sedang fokus proyek Sensus), catatan menjadi tumpukan teks tidak terstruktur (*unstructured bloat*) yang membebani prompt LLM.
3. Ekstraksi masih mengandalkan pencocokan kata kunci statis di `AutonomousProfiler`, rentan terhadap kalimat sarkastik atau hipotesis (*false positives*).

---

## 2. Rencana Arsitektur & Perubahan Teknis

### 2.1 Peningkatan Skema Database (`schema.rs`)
Tambahkan kolom berbasis JSON terstruktur ke tabel `user_profiles`:
```sql
ALTER TABLE user_profiles ADD COLUMN communication_prefs TEXT DEFAULT '{}';
ALTER TABLE user_profiles ADD COLUMN active_projects TEXT DEFAULT '[]';
ALTER TABLE user_profiles ADD COLUMN durable_constraints TEXT DEFAULT '[]';
ALTER TABLE user_profiles ADD COLUMN confidence_score REAL DEFAULT 1.0;
```

Struktur JSON `communication_prefs`:
```json
{
  "verbosity": "concise",           // "concise" | "balanced" | "elaborate"
  "technical_depth": "advanced",     // "non_technical" | "intermediate" | "advanced"
  "preferred_format": "bullet_points", // "bullet_points" | "narrative" | "code_first"
  "code_language": "rust",          // bahasa pemrograman pilihan
  "honorific": "Mas"                // sapaan yang disukai
}
```

### 2.2 Model Domain & Struct (`src/core/ports/session_store.rs`)
Perbarui struct `UserProfile`:
```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct CommunicationPreferences {
    pub verbosity: Option<String>,
    pub technical_depth: Option<String>,
    pub preferred_format: Option<String>,
    pub code_language: Option<String>,
    pub honorific: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ActiveProject {
    pub name: String,
    pub role: Option<String>,
    pub updated_at: i64,
}

pub struct UserProfile {
    pub sender_jid: String,
    pub name: Option<String>,
    pub role: Option<String>,
    pub authority_level: String,
    pub notes: Option<String>,
    pub granted_by: Option<String>,
    pub granted_at: Option<i64>,
    pub role_hierarchy: Option<String>,
    pub communication_prefs: CommunicationPreferences,
    pub active_projects: Vec<ActiveProject>,
    pub durable_constraints: Vec<String>,
    pub confidence_score: f64,
}
```

### 2.3 Evolusi Ekstraksi: Pemisahan Regex Cepat vs Evaluasi Semantik
1. **Lapis 1 (Hot Path - Regex Ringan)**: Menangkap nama panggilan dan sapaan cepat dalam waktu sub-milidetik saat pesan masuk ([profiler.rs](file:///root/projects/aina/src/core/domain/profiler.rs)).
2. **Lapis 2 (Async Reflection / Agentic Consolidation)**: Menggunakan LLM secara asinkron atau memanggil sub-command `aina user profile-update` untuk menyaring preferensi komunikasi dan proyek aktif tanpa menghambat latensi respons chat.

### 2.4 Integrasi ke Persona Prompt (`src/core/domain/persona.rs`)
Suntikkan preferensi terstruktur ke dalam prompt sistem:
```markdown
[Preferensi Gaya Komunikasi Lawan Bicara]:
- Kerapatan Pesan: Singkat & to-the-point (hindari basa-basi panjang).
- Kedalaman Teknis: Mahir / Advanced (langsung berikan cuplikan kode).
- Proyek Aktif: Migrasi Database Sipedas (fokuskan konteks pada proyek ini).
- Batasan Khusus: Tidak menyukai tabel markdown, selalu sertakan contoh bash.
```

---

## 3. Integrasi Skill (`skills/user-profiler/`)
- Perbarui [skills/user-profiler/SKILL.md](file:///root/projects/aina/skills/user-profiler/SKILL.md) agar agen mengetahui kapan dan bagaimana membaca serta memperbarui preferensi semantik lawan bicara:
  - `aina user pref <sender_jid> --key verbosity --value concise`
  - `aina user project add <sender_jid> --name "Sensus 2026" --role "PIC"`

---

## 4. Kriteria Keberhasilan & Pengujian Bukti (Evidence Tests)
- `test_user_profile_serializes_and_deserializes_semantic_json`: Memverifikasi pembacaan dan penyimpanan field JSON di SQLite.
- `test_persona_prompt_renders_structured_preferences`: Memverifikasi prompt menyertakan panduan gaya komunikasi spesifik lawan bicara.
- `test_profile_update_does_not_mutate_security_authority`: Memverifikasi pembaruan preferensi gaya tidak mengubah `authority_level`.
