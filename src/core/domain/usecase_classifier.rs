use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UseCaseCategory {
    CodeAndDevOps,
    DocumentAndVision,
    DataAnalysis,
    InformationAndResearch,
    TaskAndScheduling,
    KnowledgeAndContext,
    SystemAndControl,
    MediaAndWhatsApp,
    CasualAndConsultation,
    Other,
}

impl UseCaseCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            UseCaseCategory::CodeAndDevOps => "code_and_devops",
            UseCaseCategory::DocumentAndVision => "document_and_vision",
            UseCaseCategory::DataAnalysis => "data_analysis",
            UseCaseCategory::InformationAndResearch => "information_and_research",
            UseCaseCategory::TaskAndScheduling => "task_and_scheduling",
            UseCaseCategory::KnowledgeAndContext => "knowledge_and_context",
            UseCaseCategory::SystemAndControl => "system_and_control",
            UseCaseCategory::MediaAndWhatsApp => "media_and_whatsapp",
            UseCaseCategory::CasualAndConsultation => "casual_and_consultation",
            UseCaseCategory::Other => "other",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            UseCaseCategory::CodeAndDevOps => "Code & DevOps",
            UseCaseCategory::DocumentAndVision => "Dokumen & OCR Vision",
            UseCaseCategory::DataAnalysis => "Analisis Data & Statistik",
            UseCaseCategory::InformationAndResearch => "Informasi & Riset Web",
            UseCaseCategory::TaskAndScheduling => "Pengingat & Penjadwalan",
            UseCaseCategory::KnowledgeAndContext => "Memori & Konteks Obrolan",
            UseCaseCategory::SystemAndControl => "Kontrol Sistem & Admin",
            UseCaseCategory::MediaAndWhatsApp => "Media & WhatsApp Story",
            UseCaseCategory::CasualAndConsultation => "Konsultasi & Obrolan Santai",
            UseCaseCategory::Other => "Lainnya",
        }
    }

    pub fn from_str(s: &str) -> Self {
        let clean = s.trim().to_lowercase();
        match clean.as_str() {
            "code_and_devops" | "coding" | "devops" | "code" => UseCaseCategory::CodeAndDevOps,
            "document_and_vision" | "document" | "vision" | "ocr" | "pdf" => {
                UseCaseCategory::DocumentAndVision
            }
            "data_analysis" | "data" | "analysis" | "analytics" => UseCaseCategory::DataAnalysis,
            "information_and_research" | "research" | "search" | "info" => {
                UseCaseCategory::InformationAndResearch
            }
            "task_and_scheduling" | "schedule" | "task" | "reminder" => {
                UseCaseCategory::TaskAndScheduling
            }
            "knowledge_and_context" | "knowledge" | "context" | "memory" => {
                UseCaseCategory::KnowledgeAndContext
            }
            "system_and_control" | "system" | "admin" | "control" => {
                UseCaseCategory::SystemAndControl
            }
            "media_and_whatsapp" | "media" | "whatsapp" | "story" | "image" => {
                UseCaseCategory::MediaAndWhatsApp
            }
            "casual_and_consultation" | "casual" | "chat" | "consultation" | "general" => {
                UseCaseCategory::CasualAndConsultation
            }
            _ => UseCaseCategory::Other,
        }
    }
}

pub struct UseCaseClassifier;

impl UseCaseClassifier {
    /// Classifies an incoming message and execution context into a high-level UseCaseCategory.
    /// Blazing fast (<5us) with zero heavy allocations.
    pub fn classify(
        text: &str,
        has_media: bool,
        media_path: Option<&str>,
        tools_invoked: &[String],
    ) -> UseCaseCategory {
        let trimmed = text.trim();
        let lower = trimmed.to_lowercase();

        // 1. Built-in administrative & system commands
        if trimmed.starts_with('/')
            || tools_invoked.iter().any(|t| t.starts_with("builtin:"))
        {
            if trimmed.starts_with("/model")
                || trimmed.starts_with("/workspace")
                || trimmed.starts_with("/user")
                || trimmed.starts_with("/reset")
                || trimmed.starts_with("/clear")
                || trimmed.starts_with("/new")
                || trimmed.starts_with("/restart")
                || trimmed.starts_with("/status")
                || trimmed.starts_with("/help")
                || tools_invoked.iter().any(|t| {
                    t == "builtin:model"
                        || t == "builtin:workspace"
                        || t == "builtin:user_profile"
                        || t == "builtin:reset"
                        || t == "builtin:kill"
                })
            {
                return UseCaseCategory::SystemAndControl;
            }

            if tools_invoked.iter().any(|t| t == "builtin:scheduler") {
                return UseCaseCategory::TaskAndScheduling;
            }

            if tools_invoked.iter().any(|t| t == "builtin:media_gate") {
                return UseCaseCategory::MediaAndWhatsApp;
            }
        }

        // 2. Post-execution Tool Correlation (Strongest Runtime Evidence)
        if !tools_invoked.is_empty() {
            // Document extraction tools
            if tools_invoked
                .iter()
                .any(|t| t.contains("doc-extract") || t.contains("document-extractor"))
            {
                return UseCaseCategory::DocumentAndVision;
            }

            // Image generation & WhatsApp Story tools
            if tools_invoked
                .iter()
                .any(|t| t == "generate_image" || t.contains("wa_tool"))
            {
                return UseCaseCategory::MediaAndWhatsApp;
            }

            // Scheduling tools
            if tools_invoked
                .iter()
                .any(|t| t == "schedule" || t.contains("scheduler"))
            {
                return UseCaseCategory::TaskAndScheduling;
            }

            // Web search & research tools
            if tools_invoked
                .iter()
                .any(|t| t == "search_web" || t == "read_url_content")
            {
                return UseCaseCategory::InformationAndResearch;
            }

            // Code execution & editing tools
            if tools_invoked.iter().any(|t| {
                t == "run_command"
                    || t == "replace_file_content"
                    || t == "write_to_file"
                    || t == "fastgrep"
            }) {
                // Disambiguate data analysis vs software dev
                if Self::matches_data_analysis(&lower) {
                    return UseCaseCategory::DataAnalysis;
                }
                return UseCaseCategory::CodeAndDevOps;
            }
        }

        // 3. Media Type Analysis
        if has_media {
            if let Some(path) = media_path {
                let p_lower = path.to_lowercase();
                if p_lower.ends_with(".pdf")
                    || p_lower.ends_with(".doc")
                    || p_lower.ends_with(".docx")
                {
                    return UseCaseCategory::DocumentAndVision;
                }
                if p_lower.ends_with(".csv")
                    || p_lower.ends_with(".xlsx")
                    || p_lower.ends_with(".xls")
                {
                    return UseCaseCategory::DataAnalysis;
                }
                if p_lower.ends_with(".png")
                    || p_lower.ends_with(".jpg")
                    || p_lower.ends_with(".jpeg")
                    || p_lower.ends_with(".webp")
                {
                    if Self::matches_document_or_ocr(&lower) {
                        return UseCaseCategory::DocumentAndVision;
                    }
                    return UseCaseCategory::MediaAndWhatsApp;
                }
            } else if Self::matches_document_or_ocr(&lower) {
                return UseCaseCategory::DocumentAndVision;
            } else {
                return UseCaseCategory::MediaAndWhatsApp;
            }
        }

        // 4. Intent Keywords Pattern Matching (Pre-execution and general heuristic)
        if Self::matches_task_and_scheduling(&lower) {
            return UseCaseCategory::TaskAndScheduling;
        }

        if Self::matches_code_and_devops(&lower) {
            return UseCaseCategory::CodeAndDevOps;
        }

        if Self::matches_document_or_ocr(&lower) {
            return UseCaseCategory::DocumentAndVision;
        }

        if Self::matches_data_analysis(&lower) {
            return UseCaseCategory::DataAnalysis;
        }

        if Self::matches_information_and_research(&lower) {
            return UseCaseCategory::InformationAndResearch;
        }

        if Self::matches_knowledge_and_context(&lower) {
            return UseCaseCategory::KnowledgeAndContext;
        }

        if Self::matches_media_and_whatsapp(&lower) {
            return UseCaseCategory::MediaAndWhatsApp;
        }

        if Self::matches_casual_or_consultation(&lower) {
            return UseCaseCategory::CasualAndConsultation;
        }

        // Short conversational default
        if trimmed.len() <= 40 {
            UseCaseCategory::CasualAndConsultation
        } else {
            UseCaseCategory::CasualAndConsultation
        }
    }

    fn matches_code_and_devops(s: &str) -> bool {
        const PATTERNS: &[&str] = &[
            "code", "coding", "skrip", "script", "fungsi", "function", "bug", "error",
            "debug", "trace", "compile", "kompilasi", "rust", "python", "cargo", "bash",
            "terminal", "git", "github", "commit", "pull request", "deploy", "docker",
            "container", "linux", "command", "perintah", "syntax", "refactor", "endpoint",
            "backend", "frontend", "npm", "bun", "test", "benchmark", "ci/cd", "repositori",
        ];
        PATTERNS.iter().any(|p| s.contains(p))
    }

    fn matches_document_or_ocr(s: &str) -> bool {
        const PATTERNS: &[&str] = &[
            "pdf", "ekstrak tabel", "extract table", "dokumen", "document", "ocr",
            "scan", "surat", "kwitansi", "invoice", "berkas", "lampiran", "baca pdf",
            "ekstrak pdf", "tabel dari", "kolom tabel",
        ];
        PATTERNS.iter().any(|p| s.contains(p))
    }

    fn matches_data_analysis(s: &str) -> bool {
        const PATTERNS: &[&str] = &[
            "analisis data", "statistik", "rekap", "rekapitulasi", "grafik", "chart",
            "csv", "excel", "spreadsheet", "rata-rata", "median", "persentase",
            "tren data", "dataset", "hitung total", "kalkulasi", "agregasi",
        ];
        PATTERNS.iter().any(|p| s.contains(p))
    }

    fn matches_information_and_research(s: &str) -> bool {
        const PATTERNS: &[&str] = &[
            "cari ", "carikan", "searching", "siapa ", "apa itu ", "kapan ", "dimana ",
            "berita", "kabar terbaru", "kabar terkini", "kabar berita", "regulasi", "aturan", "peraturan", "artikel", "referensi",
            "website", "link ", "browsing", "informasi tentang", "cek google", "cek web",
            "cuaca", "kurs", "harga",
        ];
        PATTERNS.iter().any(|p| s.contains(p))
    }

    fn matches_task_and_scheduling(s: &str) -> bool {
        const PATTERNS: &[&str] = &[
            "ingatkan", "pengingat", "reminder", "jadwal", "jadwalkan", "alarm",
            "setiap hari", "nanti jam", "besok jam", "schedule", "cron", "agendakan",
            "agenda", "jangan lupa", "ingatkan saya",
        ];
        PATTERNS.iter().any(|p| s.contains(p))
    }

    fn matches_knowledge_and_context(s: &str) -> bool {
        const PATTERNS: &[&str] = &[
            "ingat ", "ingatkah", "kemarin", "obrolan lalu", "percakapan sebelumnya",
            "arsip", "siapa nama", "nomor telepon", "kontak", "profil", "riwayat chat",
            "history chat", "catatan rapat", "pembahasan kemarin",
        ];
        PATTERNS.iter().any(|p| s.contains(p))
    }

    fn matches_media_and_whatsapp(s: &str) -> bool {
        const PATTERNS: &[&str] = &[
            "buat gambar", "generate gambar", "generate image", "lukis", "ilustrasi",
            "story wa", "status wa", "kontak wa", "kartu kontak", "audio note", "voice note",
            "caption gambar", "buat status",
        ];
        PATTERNS.iter().any(|p| s.contains(p))
    }

    fn matches_casual_or_consultation(s: &str) -> bool {
        const PATTERNS: &[&str] = &[
            "halo", "hai", "pagi", "siang", "sore", "malam", "terima kasih", "makasih",
            "assalamualaikum", "nasihat", "pendapat", "fikih", "fiqih", "bagaimana menurutmu",
            "curhat", "salam", "apa kabar", "terimakasih", "thanks", "tengkyu",
        ];
        PATTERNS.iter().any(|p| s.contains(p))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_and_control_classification() {
        assert_eq!(
            UseCaseClassifier::classify("/model set gemini-2.5-flash", false, None, &[]),
            UseCaseCategory::SystemAndControl
        );
        assert_eq!(
            UseCaseClassifier::classify("/workspace create project-x", false, None, &[]),
            UseCaseCategory::SystemAndControl
        );
        assert_eq!(
            UseCaseClassifier::classify("/reset", false, None, &[]),
            UseCaseCategory::SystemAndControl
        );
        assert_eq!(
            UseCaseClassifier::classify(
                "Tolong reset sesi",
                false,
                None,
                &["builtin:reset".to_string()]
            ),
            UseCaseCategory::SystemAndControl
        );
    }

    #[test]
    fn test_code_and_devops_classification() {
        assert_eq!(
            UseCaseClassifier::classify("Tolong buat skrip python untuk merge file csv", false, None, &[]),
            UseCaseCategory::CodeAndDevOps
        );
        assert_eq!(
            UseCaseClassifier::classify(
                "Jalankan unit test di cargo",
                false,
                None,
                &["run_command".to_string()]
            ),
            UseCaseCategory::CodeAndDevOps
        );
    }

    #[test]
    fn test_document_and_vision_classification() {
        assert_eq!(
            UseCaseClassifier::classify("Ini berkas laporan keuangan", true, Some("/tmp/laporan.pdf"), &[]),
            UseCaseCategory::DocumentAndVision
        );
        assert_eq!(
            UseCaseClassifier::classify(
                "Tolong baca tabel ini",
                true,
                Some("/tmp/foto.png"),
                &["agy-doc-extract".to_string()]
            ),
            UseCaseCategory::DocumentAndVision
        );
    }

    #[test]
    fn test_information_and_research_classification() {
        assert_eq!(
            UseCaseClassifier::classify("Carikan berita terbaru tentang AI di Indonesia", false, None, &[]),
            UseCaseCategory::InformationAndResearch
        );
        assert_eq!(
            UseCaseClassifier::classify(
                "Cek isi web ini",
                false,
                None,
                &["read_url_content".to_string()]
            ),
            UseCaseCategory::InformationAndResearch
        );
    }

    #[test]
    fn test_task_and_scheduling_classification() {
        assert_eq!(
            UseCaseClassifier::classify("Ingatkan saya rapat jam 3 sore nanti", false, None, &[]),
            UseCaseCategory::TaskAndScheduling
        );
        assert_eq!(
            UseCaseClassifier::classify("Jadwalkan backup mingguan", false, None, &["schedule".to_string()]),
            UseCaseCategory::TaskAndScheduling
        );
    }

    #[test]
    fn test_casual_and_consultation_classification() {
        assert_eq!(
            UseCaseClassifier::classify("Halo Aina, apa kabar?", false, None, &[]),
            UseCaseCategory::CasualAndConsultation
        );
        assert_eq!(
            UseCaseClassifier::classify("Terima kasih banyak yaa!", false, None, &[]),
            UseCaseCategory::CasualAndConsultation
        );
    }
}
