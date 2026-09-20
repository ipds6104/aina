use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Kontrak formal tool: mendefinisikan kemampuan, batasan resource, dan mode kegagalan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolContract {
    pub name: String,
    pub description: String,
    pub capability_tags: Vec<String>,
    pub resource_intensity: String, // "light", "medium", "heavy"
    pub timeout_seconds: u64,
    pub supports_multimodal: bool,
    pub common_failure_modes: Vec<String>,
    pub forbidden_patterns: Vec<String>,
}

/// Batasan lingkungan eksekusi aktual (PRoot, ARM64, OS, Storage)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentBoundaries {
    pub os_name: String,
    pub architecture: String,
    pub container_context: String,
    pub safe_ram_mb: u64,
    pub hard_limit_ram_mb: u64,
    pub persistent_storage_paths: Vec<String>,
    pub temporary_storage_paths: Vec<String>,
    pub forbidden_operations: Vec<String>,
}

/// Profil model LLM yang sedang aktif
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelProfile {
    pub model_name: String,
    pub context_window_tokens: usize,
    pub supports_structured_json: bool,
    pub supports_vision: bool,
    pub supports_video_gen: bool,
    pub latency_tier: String, // "fast", "medium", "slow"
    pub known_biases: Vec<String>,
}

/// Snapshot representasi diri (R) lengkap dari agen
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCapabilityManifest {
    pub version: String,
    pub model_profile: ModelProfile,
    pub environment: EnvironmentBoundaries,
    pub tool_contracts: HashMap<String, ToolContract>,
    pub supported_domains: Vec<String>,
    pub unsupported_domains: Vec<String>,
}

impl AgentCapabilityManifest {
    /// Membuat manifest kemampuan bawaan dan lingkungan live Aina
    pub fn default_manifest() -> Self {
        let mut tools = HashMap::new();

        // 1. generate_image
        tools.insert(
            "generate_image".to_string(),
            ToolContract {
                name: "generate_image".to_string(),
                description: "Menghasilkan gambar still 2D berkualitas tinggi bergaya Makoto Shinkai".to_string(),
                capability_tags: vec!["2d_image".to_string(), "illustration".to_string(), "visual_avatar".to_string()],
                resource_intensity: "medium".to_string(),
                timeout_seconds: 90,
                supports_multimodal: true,
                common_failure_modes: vec![
                    "Menggambar alat kamera/tripod jika kata tersebut disebutkan dalam prompt adegan".to_string(),
                    "Visual drifting jika tidak menyertakan acuan character_sheet.png".to_string(),
                ],
                forbidden_patterns: vec![
                    "Jangan membuat video atau animasi bergerak (hanya gambar diam)".to_string(),
                    "Jangan menyebut tripod/penyangga ponsel sebagai objek visual".to_string(),
                ],
            },
        );

        // 2. run_command
        tools.insert(
            "run_command".to_string(),
            ToolContract {
                name: "run_command".to_string(),
                description: "Menjalankan perintah terminal Bash di lingkungan Ubuntu ARM64".to_string(),
                capability_tags: vec!["terminal".to_string(), "bash".to_string(), "filesystem".to_string()],
                resource_intensity: "variable".to_string(),
                timeout_seconds: 60,
                supports_multimodal: false,
                common_failure_modes: vec![
                    "Timeout jika memindai direktori root (misal: find /)".to_string(),
                    "Out-of-memory jika menjalankan kompilasi/binary berat di atas PRoot Android".to_string(),
                ],
                forbidden_patterns: vec![
                    "DILARANG menjalankan 'find /' atau pemindaian filesystem tanpa batas".to_string(),
                    "DILARANG menjalankan npm/npx langsung (wajib gunakan Bun: bun/bunx)".to_string(),
                ],
            },
        );

        // 3. wa_tool.py
        tools.insert(
            "wa_tool.py".to_string(),
            ToolContract {
                name: "wa_tool.py".to_string(),
                description: "Antarmuka gateway WhatsApp (Whatsmeow) untuk pesan, reaksi, dan status story".to_string(),
                capability_tags: vec!["messaging".to_string(), "whatsapp".to_string(), "status_story".to_string()],
                resource_intensity: "light".to_string(),
                timeout_seconds: 30,
                supports_multimodal: true,
                common_failure_modes: vec![
                    "Double-status jika memposting teks terpisah setelah media status".to_string(),
                    "Caption hilang jika form multipart tidak menyertakan field caption & text".to_string(),
                ],
                forbidden_patterns: vec![
                    "DILARANG memposting teks laporan teknis/konfirmasi ke status@broadcast".to_string(),
                    "DILARANG mengarang alasan bahwa WhatsApp HP menyembunyikan caption".to_string(),
                ],
            },
        );

        // 4. agy-doc-extract
        tools.insert(
            "agy-doc-extract".to_string(),
            ToolContract {
                name: "agy-doc-extract".to_string(),
                description: "Ekstraksi dokumen PDF dan gambar via CodeBuddy VLM 9Router dengan beban CPU 0%".to_string(),
                capability_tags: vec!["document_ocr".to_string(), "table_extraction".to_string(), "pdf".to_string()],
                resource_intensity: "light".to_string(),
                timeout_seconds: 120,
                supports_multimodal: true,
                common_failure_modes: vec![
                    "Koneksi timeout jika router 9Router offline atau kunci API kadaluarsa".to_string(),
                ],
                forbidden_patterns: vec![],
            },
        );

        // 5. replace_file_content / write_to_file
        tools.insert(
            "replace_file_content".to_string(),
            ToolContract {
                name: "replace_file_content".to_string(),
                description: "Menyunting baris kode spesifik secara presisi tanpa menimpa seluruh file".to_string(),
                capability_tags: vec!["code_edit".to_string(), "refactor".to_string()],
                resource_intensity: "light".to_string(),
                timeout_seconds: 15,
                supports_multimodal: false,
                common_failure_modes: vec![
                    "TargetContent tidak persis sama dengan isi file (whitespace/indentasi)".to_string(),
                ],
                forbidden_patterns: vec![
                    "Jangan menimpa seluruh file dengan replace_file_content".to_string(),
                ],
            },
        );

        Self {
            version: "2.1.0".to_string(),
            model_profile: ModelProfile {
                model_name: "gemini-3.8-flash-medium".to_string(),
                context_window_tokens: 1_048_576,
                supports_structured_json: true,
                supports_vision: true,
                supports_video_gen: false,
                latency_tier: "fast".to_string(),
                known_biases: vec![
                    "Kecenderungan untuk tampak tahu / konfabulasi saat terjadi anomali transmisi".to_string(),
                    "Kecenderungan overconfidence pada perintah pemindaian file luas".to_string(),
                ],
            },
            environment: EnvironmentBoundaries {
                os_name: "Ubuntu 26.04 LTS".to_string(),
                architecture: "aarch64 (ARM64)".to_string(),
                container_context: "PRoot Distro di atas Termux Android".to_string(),
                safe_ram_mb: 150,
                hard_limit_ram_mb: 512,
                persistent_storage_paths: vec![
                    "/app/data".to_string(),
                    "/root/projects/aina".to_string(),
                    "/root/.gemini/antigravity-cli".to_string(),
                ],
                temporary_storage_paths: vec!["/tmp".to_string()],
                forbidden_operations: vec![
                    "Pemindaian luas tanpa batas (find /)".to_string(),
                    "Penggunaan npm/npx langsung tanpa Bun".to_string(),
                    "Render 3D / kompilasi native yang melampaui RAM 512MB".to_string(),
                ],
            },
            tool_contracts: tools,
            supported_domains: vec![
                "software_engineering_rust_python_ts".to_string(),
                "whatsapp_communication_and_stories".to_string(),
                "document_and_table_extraction".to_string(),
                "autonomous_persona_and_wardrobe_rhythm".to_string(),
                "git_and_codebase_observability".to_string(),
            ],
            unsupported_domains: vec![
                "3d_mesh_rendering_and_blender".to_string(),
                "video_generation_or_editing".to_string(),
                "gui_desktop_browser_automation".to_string(),
                "kernel_driver_compilation".to_string(),
            ],
        }
    }

    /// Menghasilkan representasi teks R ringkas untuk diinjeksikan ke context prompt
    pub fn to_prompt_context(&self) -> String {
        let tools_list: Vec<String> = self
            .tool_contracts
            .iter()
            .map(|(k, v)| format!("`{}` ({})", k, v.capability_tags.join(", ")))
            .collect();

        format!(
            "🧠 [REPRESENTASI DIRI & BATASAN KAPABILITAS (METACONTEXT R)]:\n\
            • Identitas Engine: {} (Konteks: {} tokens, Multi-Modal Image: Ya, Video Gen: Tidak).\n\
            • Lingkungan: {} | Arsitektur: {} (Konteks: {}).\n\
            • Batas Memori & Keamanan: RAM aman <{}MB. Dilarang pemindaian luas tanpa batas (find /).\n\
            • Tools Resmi Terverifikasi: {}.\n\
            • Domain yang Didukung: {}.\n\
            • Domain yang TIDAK Didukung: {} (Tolak secara elegan dan transparan jika diminta).\n\
            • Disiplin Epistemik: Jika terjadi diskrepansi antara observasi pengguna dan data sistem, DILARANG MENGARANG ALASAN (konfabulasi). Akui ketidaktahuan atau lakukan audit ground truth.",
            self.model_profile.model_name,
            self.model_profile.context_window_tokens,
            self.environment.os_name,
            self.environment.architecture,
            self.environment.container_context,
            self.environment.safe_ram_mb,
            tools_list.join(", "),
            self.supported_domains.join(", "),
            self.unsupported_domains.join(", ")
        )
    }
}
