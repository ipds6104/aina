use crate::core::domain::{
    calculate_brier_score, calculate_calibration_stats, ComplexityTier, MetacognitiveCalibrationStats,
    MetacognitivePrediction, NewMetacognitivePrediction, TaskDomainType,
};
use rusqlite::{params, Connection};

pub fn record_metacognitive_prediction(
    conn: &Connection,
    pred: &NewMetacognitivePrediction,
) -> anyhow::Result<i64> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let risks_json = serde_json::to_string(&pred.identified_risks).unwrap_or_else(|_| "[]".to_string());

    conn.execute(
        "INSERT INTO metacognitive_predictions (
            prediction_id, action_audit_id, task_description, domain_type,
            predicted_probability, complexity_tier, identified_risks,
            fallback_strategy, created_at_epoch
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            pred.prediction_id,
            pred.action_audit_id,
            pred.task_description,
            pred.domain_type.as_str(),
            pred.predicted_probability,
            pred.complexity_tier.as_str(),
            risks_json,
            pred.fallback_strategy,
            now,
        ],
    )?;

    Ok(conn.last_insert_rowid())
}

pub fn resolve_metacognitive_prediction(
    conn: &Connection,
    prediction_id: &str,
    actual_outcome: f64,
    duration_secs: f64,
    error_detail: Option<&str>,
) -> anyhow::Result<()> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let mut stmt = conn.prepare("SELECT predicted_probability FROM metacognitive_predictions WHERE prediction_id = ?1")?;
    let predicted_prob: Option<f64> = stmt.query_row(params![prediction_id], |r| r.get(0)).ok();

    if let Some(p) = predicted_prob {
        let brier = calculate_brier_score(p, actual_outcome);
        conn.execute(
            "UPDATE metacognitive_predictions SET
                actual_outcome = ?1,
                brier_score = ?2,
                execution_duration_secs = ?3,
                error_detail = ?4,
                resolved_at_epoch = ?5
            WHERE prediction_id = ?6",
            params![actual_outcome, brier, duration_secs, error_detail, now, prediction_id],
        )?;
    }

    Ok(())
}

pub fn list_metacognitive_predictions(
    conn: &Connection,
    limit: usize,
    domain_filter: Option<&str>,
) -> anyhow::Result<Vec<MetacognitivePrediction>> {
    let query = if let Some(dom) = domain_filter {
        format!(
            "SELECT id, prediction_id, action_audit_id, task_description, domain_type,
                    predicted_probability, complexity_tier, identified_risks, fallback_strategy,
                    actual_outcome, brier_score, execution_duration_secs, error_detail,
                    created_at_epoch, resolved_at_epoch
             FROM metacognitive_predictions
             WHERE domain_type = '{}'
             ORDER BY created_at_epoch DESC LIMIT {}",
            dom.replace('\'', "''"),
            limit
        )
    } else {
        format!(
            "SELECT id, prediction_id, action_audit_id, task_description, domain_type,
                    predicted_probability, complexity_tier, identified_risks, fallback_strategy,
                    actual_outcome, brier_score, execution_duration_secs, error_detail,
                    created_at_epoch, resolved_at_epoch
             FROM metacognitive_predictions
             ORDER BY created_at_epoch DESC LIMIT {}",
            limit
        )
    };

    let mut stmt = conn.prepare(&query)?;
    let rows = stmt.query_map([], |row| {
        let risks_json: String = row.get(7)?;
        let risks: Vec<String> = serde_json::from_str(&risks_json).unwrap_or_default();
        let domain_str: String = row.get(4)?;
        let complexity_str: String = row.get(6)?;

        Ok(MetacognitivePrediction {
            id: row.get(0)?,
            prediction_id: row.get(1)?,
            action_audit_id: row.get(2)?,
            task_description: row.get(3)?,
            domain_type: TaskDomainType::from_str(&domain_str),
            predicted_probability: row.get(5)?,
            complexity_tier: ComplexityTier::from_str(&complexity_str),
            identified_risks: risks,
            fallback_strategy: row.get(8)?,
            actual_outcome: row.get(9)?,
            brier_score: row.get(10)?,
            execution_duration_secs: row.get(11)?,
            error_detail: row.get(12)?,
            created_at_epoch: row.get(13)?,
            resolved_at_epoch: row.get(14)?,
        })
    })?;

    let mut results = Vec::new();
    for r in rows {
        results.push(r?);
    }
    Ok(results)
}

pub fn get_metacognitive_calibration_stats(conn: &Connection) -> anyhow::Result<MetacognitiveCalibrationStats> {
    let predictions = list_metacognitive_predictions(conn, 500, None)?;
    Ok(calculate_calibration_stats(&predictions))
}
