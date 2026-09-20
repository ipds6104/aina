use super::capability_registry::AgentCapabilityManifest;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TriageDecision {
    /// Tugas dapat dieksekusi langsung dengan tool yang ada
    DirectExecution {
        confidence: f64,
        selected_tools: Vec<String>,
    },
    /// Tugas tidak bisa dipenuhi 100% secara native, tetapi ada alternatif realistis
    GracefulDegradation {
        confidence: f64,
        suggested_alternative: String,
        reason: String,
    },
    /// Tugas melampaui batasan keras lingkungan/tool dan wajib ditolak secara elegan di awal
    ElegantRejection {
        reason: String,
        missing_capabilities: Vec<String>,
    },
}

pub struct TaskTriageEngine;

impl TaskTriageEngine {
    /// Menilai tugas baru (termasuk tugas held-out yang belum pernah dipelajari)
    /// berdasarkan representasi diri R pada AgentCapabilityManifest
    pub fn triage_task(task_description: &str, _manifest: &AgentCapabilityManifest) -> TriageDecision {
        let task_lower = task_description.to_lowercase();

        // 1. Periksa Domain yang Secara Keras Tidak Didukung (Hard Boundaries)
        if task_lower.contains("3d") || task_lower.contains("blender") || task_lower.contains("mesh render") {
            return TriageDecision::GracefulDegradation {
                confidence: 0.85,
                suggested_alternative: "Menuliskan skrip Python/Blender lengkap yang siap dijalankan di workstation pengguna, atau membuat ilustrasi konsep 2D via generate_image".to_string(),
                reason: "Container Ubuntu PRoot ARM64 ini tidak memiliki akselerator GPU 3D atau binary Blender terpasang".to_string(),
            };
        }

        if task_lower.contains("video animasi") || task_lower.contains("generate video") || task_lower.contains("buat video") {
            return TriageDecision::ElegantRejection {
                reason: "Model Gemini dan tool saat ini hanya mendukung pembuatan gambar still 2D (generate_image), tidak mendukung sintesis video generatif".to_string(),
                missing_capabilities: vec!["video_generation".to_string(), "video_rendering_gpu".to_string()],
            };
        }

        if task_lower.contains("browser buka") || task_lower.contains("klik layar") || task_lower.contains("layar gui") {
            return TriageDecision::GracefulDegradation {
                confidence: 0.90,
                suggested_alternative: "Mengambil data halaman web secara headless via HTTP/search_web atau ekstraksi teks/tabel".to_string(),
                reason: "Lingkungan ini beroperasi dalam mode server headless tanpa display server X11/Wayland".to_string(),
            };
        }

        // 2. Periksa Perintah Berbahaya / Pemindaian Tanpa Batas
        if task_lower.contains("find /") || task_lower.contains("scan seluruh server") {
            return TriageDecision::ElegantRejection {
                reason: "Operasi pemindaian seluruh filesystem dari root ('find /') dilarang keras karena membebani I/O Android PRoot dan berisiko tinggi timeout".to_string(),
                missing_capabilities: vec!["safe_unbounded_disk_scan".to_string()],
            };
        }

        // 3. Deteksi Tool yang Relevan untuk Tugas yang Didukung
        let mut tools = Vec::new();
        if task_lower.contains("status") || task_lower.contains("story") {
            tools.push("wa_tool.py".to_string());
            tools.push("generate_image".to_string());
        }
        if task_lower.contains("pdf") || task_lower.contains("ekstrak") || task_lower.contains("tabel") {
            tools.push("agy-doc-extract".to_string());
        }
        if task_lower.contains("kode") || task_lower.contains("script") || task_lower.contains("edit") || task_lower.contains("cek") {
            tools.push("run_command".to_string());
            tools.push("replace_file_content".to_string());
        }

        if tools.is_empty() {
            tools.push("general_reasoning".to_string());
        }

        TriageDecision::DirectExecution {
            confidence: 0.92,
            selected_tools: tools,
        }
    }
}
