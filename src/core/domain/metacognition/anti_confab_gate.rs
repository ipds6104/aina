use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AntiConfabResolution {
    /// Akui ada perbedaan antara data sistem internal dan observasi pengguna, lalu picu investigasi ground truth
    AcknowledgeAndAudit {
        internal_claim: String,
        user_observation: String,
        suggested_investigation: String,
    },
    /// Akui ketidaktahuan kausal secara transparan tanpa mengarang pembenaran
    AdmitIgnorance {
        context: String,
        reason: String,
    },
    /// Koreksi langsung berdasarkan bukti faktual terverifikasi
    DirectCorrection {
        factual_truth: String,
    },
    /// Tidak ada diskrepansi (konsisten)
    NoDiscrepancy,
}

pub struct AntiConfabGate;

impl AntiConfabGate {
    /// Mengevaluasi apakah ada diskrepansi antara laporan pengguna dan status internal sistem
    pub fn evaluate_discrepancy(
        internal_status_summary: &str,
        user_feedback: &str,
    ) -> AntiConfabResolution {
        let user_lower = user_feedback.to_lowercase();
        let internal_lower = internal_status_summary.to_lowercase();

        // 1. Kasus Caption WhatsApp Story
        let is_caption_query = user_lower.contains("caption") || user_lower.contains("keterangan");
        let is_missing_report = user_lower.contains("tidak muncul")
            || user_lower.contains("tidak ada")
            || user_lower.contains("kosong")
            || user_lower.contains("tidak memberikan caption");

        if is_caption_query && is_missing_report {
            if internal_lower.contains("caption") || internal_lower.contains("text") {
                return AntiConfabResolution::AcknowledgeAndAudit {
                    internal_claim: "Data gateway mencatat ada parameter caption saat pengiriman media".to_string(),
                    user_observation: "Layar HP pengguna tidak menampilkan caption overlay di bawah gambar".to_string(),
                    suggested_investigation: "Investigasi pemetaan field di kode gateway Whatsmeow (status.Text vs status.Caption), JANGAN mengklaim WhatsApp HP sengaja menyembunyikan caption!".to_string(),
                };
            }
        }

        // 2. Kasus File / Dokumen Tersimpan
        let is_file_query = user_lower.contains("file") || user_lower.contains("berkas") || user_lower.contains("tersimpan");
        let is_missing_file = user_lower.contains("belum ada") || user_lower.contains("tidak ketemu") || user_lower.contains("hilang");

        if is_file_query && is_missing_file {
            return AntiConfabResolution::AcknowledgeAndAudit {
                internal_claim: "Sistem mengira file telah tersimpan di direktori".to_string(),
                user_observation: "File tidak ditemukan di path yang ditentukan".to_string(),
                suggested_investigation: "Periksa filesystem aktual via ls / view_file, jangan berasumsi file tersimpan sebelum exit code terverifikasi".to_string(),
            };
        }

        AntiConfabResolution::NoDiscrepancy
    }

    /// Menghasilkan petunjuk sistem untuk mencegah konfabulasi saat terjadi anomali
    pub fn format_epistemic_guardrail(resolution: &AntiConfabResolution) -> Option<String> {
        match resolution {
            AntiConfabResolution::AcknowledgeAndAudit {
                internal_claim,
                user_observation,
                suggested_investigation,
            } => Some(format!(
                "⚠️ [EPISTEMIC VIGILANCE - DISKREPANSI TERDETEKSI]:\n\
                • Klaim Internal: {}\n\
                • Observasi Pengguna: {}\n\
                • ATURAN MUTLAK: DILARANG KERAS mengarang pembenaran spekulatif (seperti menuduh aplikasi pihak ketiga tanpa bukti log).\n\
                • Tindakan yang Diwajibkan: {}\n\
                • Format Jawaban: Akui perbedaan fakta ini secara jujur dan transparan kepada pengguna.",
                internal_claim, user_observation, suggested_investigation
            )),
            AntiConfabResolution::AdmitIgnorance { context, reason } => Some(format!(
                "⚠️ [EPISTEMIC VIGILANCE - AKUI KETIDAKTAHUAN]:\n\
                Konteks: {}. Alasan: {}. Jangan berspekulasi.",
                context, reason
            )),
            _ => None,
        }
    }
}
