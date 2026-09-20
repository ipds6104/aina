use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComplexityTier {
    Low,
    Medium,
    High,
    Critical,
}

impl ComplexityTier {
    pub fn as_str(&self) -> &'static str {
        match self {
            ComplexityTier::Low => "low",
            ComplexityTier::Medium => "medium",
            ComplexityTier::High => "high",
            ComplexityTier::Critical => "critical",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "critical" => ComplexityTier::Critical,
            "high" => ComplexityTier::High,
            "medium" => ComplexityTier::Medium,
            _ => ComplexityTier::Low,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskDomainType {
    CodingAgent,
    PersonaStatus,
    DocExtract,
    ScheduleTask,
    GeneralQuery,
}

impl TaskDomainType {
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskDomainType::CodingAgent => "coding_agent",
            TaskDomainType::PersonaStatus => "persona_status",
            TaskDomainType::DocExtract => "doc_extract",
            TaskDomainType::ScheduleTask => "schedule_task",
            TaskDomainType::GeneralQuery => "general_query",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "coding_agent" | "coding" => TaskDomainType::CodingAgent,
            "persona_status" | "persona" | "story" => TaskDomainType::PersonaStatus,
            "doc_extract" | "document" | "ocr" => TaskDomainType::DocExtract,
            "schedule_task" | "scheduler" => TaskDomainType::ScheduleTask,
            _ => TaskDomainType::GeneralQuery,
        }
    }
}

/// Draf entri baru untuk prediksi pre-flight sebelum dieksekusi
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewMetacognitivePrediction {
    pub prediction_id: String,
    pub action_audit_id: Option<i64>,
    pub task_description: String,
    pub domain_type: TaskDomainType,
    pub predicted_probability: f64, // P(sukses) [0.0 - 1.0]
    pub complexity_tier: ComplexityTier,
    pub identified_risks: Vec<String>,
    pub fallback_strategy: Option<String>,
}

/// Entri lengkap prediksi metakognitif yang tersimpan di database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetacognitivePrediction {
    pub id: i64,
    pub prediction_id: String,
    pub action_audit_id: Option<i64>,
    pub task_description: String,
    pub domain_type: TaskDomainType,
    pub predicted_probability: f64,
    pub complexity_tier: ComplexityTier,
    pub identified_risks: Vec<String>,
    pub fallback_strategy: Option<String>,
    pub actual_outcome: Option<f64>, // 1.0 (sukses), 0.0 (gagal)
    pub brier_score: Option<f64>,    // (predicted - actual)^2
    pub execution_duration_secs: Option<f64>,
    pub error_detail: Option<String>,
    pub created_at_epoch: i64,
    pub resolved_at_epoch: Option<i64>,
}

/// Bucket keandalan untuk kurva kalibrasi (Reliability Diagram)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReliabilityBucket {
    pub range_start: f64,
    pub range_end: f64,
    pub count: usize,
    pub mean_predicted: f64,
    pub observed_frequency: f64,
}

/// Statistik kalibrasi metakognitif agregat
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetacognitiveCalibrationStats {
    pub total_predictions: usize,
    pub resolved_predictions: usize,
    pub mean_brier_score: f64,
    pub base_rate: f64,
    pub brier_skill_score: f64,
    pub calibration_status: String,
    pub reliability_buckets: Vec<ReliabilityBucket>,
    pub domain_brier_scores: HashMap<String, f64>,
}

/// Menghitung Brier score untuk satu pasang prediksi dan luaran riil: (P - Y)^2
pub fn calculate_brier_score(predicted_prob: f64, actual_outcome: f64) -> f64 {
    let p = predicted_prob.clamp(0.0, 1.0);
    let y = actual_outcome.clamp(0.0, 1.0);
    (p - y).powi(2)
}

/// Menghitung statistik kalibrasi agregat dari kumpulan prediksi
pub fn calculate_calibration_stats(predictions: &[MetacognitivePrediction]) -> MetacognitiveCalibrationStats {
    let resolved: Vec<&MetacognitivePrediction> = predictions
        .iter()
        .filter(|p| p.actual_outcome.is_some() && p.brier_score.is_some())
        .collect();

    if resolved.is_empty() {
        return MetacognitiveCalibrationStats {
            total_predictions: predictions.len(),
            resolved_predictions: 0,
            mean_brier_score: 0.0,
            base_rate: 0.0,
            brier_skill_score: 0.0,
            calibration_status: "Insufficient Data (Belum Ada Riwayat Selesai)".to_string(),
            reliability_buckets: vec![],
            domain_brier_scores: HashMap::new(),
        };
    }

    let n = resolved.len() as f64;
    let sum_brier: f64 = resolved.iter().map(|p| p.brier_score.unwrap_or(0.0)).sum();
    let mean_brier = sum_brier / n;

    let success_count: usize = resolved
        .iter()
        .filter(|p| p.actual_outcome.unwrap_or(0.0) >= 0.5)
        .count();
    let base_rate = success_count as f64 / n;

    // Brier Score untuk acuan referensi base rate: BS_ref = base_rate * (1 - base_rate)
    let bs_ref = base_rate * (1.0 - base_rate);
    let bss = if bs_ref > 0.0001 {
        1.0 - (mean_brier / bs_ref)
    } else {
        0.0
    };

    // Evaluasi 5 Bucket Kalibrasi: [0.0-0.2], [0.2-0.4], [0.4-0.6], [0.6-0.8], [0.8-1.0]
    let bucket_ranges = [
        (0.0, 0.2),
        (0.2, 0.4),
        (0.4, 0.6),
        (0.6, 0.8),
        (0.8, 1.0),
    ];

    let mut buckets = Vec::new();
    for &(start, end) in &bucket_ranges {
        let in_bucket: Vec<&&MetacognitivePrediction> = resolved
            .iter()
            .filter(|p| {
                if end >= 1.0 {
                    p.predicted_probability >= start && p.predicted_probability <= end
                } else {
                    p.predicted_probability >= start && p.predicted_probability < end
                }
            })
            .collect();

        let count = in_bucket.len();
        if count > 0 {
            let mean_p: f64 = in_bucket.iter().map(|p| p.predicted_probability).sum::<f64>() / count as f64;
            let succ: usize = in_bucket.iter().filter(|p| p.actual_outcome.unwrap_or(0.0) >= 0.5).count();
            let obs_freq = succ as f64 / count as f64;
            buckets.push(ReliabilityBucket {
                range_start: start,
                range_end: end,
                count,
                mean_predicted: mean_p,
                observed_frequency: obs_freq,
            });
        } else {
            buckets.push(ReliabilityBucket {
                range_start: start,
                range_end: end,
                count: 0,
                mean_predicted: (start + end) / 2.0,
                observed_frequency: 0.0,
            });
        }
    }

    // Breakdown per domain
    let mut domain_groups: HashMap<String, (f64, usize)> = HashMap::new();
    for p in &resolved {
        let dom = p.domain_type.as_str().to_string();
        let entry = domain_groups.entry(dom).or_insert((0.0, 0));
        entry.0 += p.brier_score.unwrap_or(0.0);
        entry.1 += 1;
    }

    let mut domain_brier_scores = HashMap::new();
    for (dom, (total, count)) in domain_groups {
        domain_brier_scores.insert(dom, total / count as f64);
    }

    let status = if mean_brier < 0.15 {
        "Well Calibrated (Tinggi - Terkalibrasi Sangat Akurat)"
    } else if mean_brier < 0.22 {
        "Moderately Calibrated (Cukup Terkalibrasi - Di Atas Base Rate)"
    } else {
        "Poorly Calibrated (Perlu Penyesuaian Bobot Prediksi)"
    };

    MetacognitiveCalibrationStats {
        total_predictions: predictions.len(),
        resolved_predictions: resolved.len(),
        mean_brier_score: mean_brier,
        base_rate,
        brier_skill_score: bss,
        calibration_status: status.to_string(),
        reliability_buckets: buckets,
        domain_brier_scores,
    }
}
