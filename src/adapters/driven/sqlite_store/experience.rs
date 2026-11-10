use crate::core::domain::{ExperienceRecord, NewExperienceRecord};
use rusqlite::{params, Connection};

pub fn record_experience(
    conn: &Connection,
    exp: &NewExperienceRecord,
) -> anyhow::Result<String> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let id = format!(
        "exp_{}_{}",
        now,
        &rand::random::<u32>().to_string()[..6.min(rand::random::<u32>().to_string().len())]
    );

    let confidence = exp.confidence.unwrap_or(1.0).clamp(0.0, 1.0);

    conn.execute(
        "INSERT INTO experience_records (
            id, domain, cue, lesson, evidence, confidence, task_id, created_at_epoch
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            id,
            exp.domain,
            exp.cue.trim(),
            exp.lesson.trim(),
            exp.evidence.trim(),
            confidence,
            exp.task_id,
            now,
        ],
    )?;

    Ok(id)
}

pub fn get_experience(
    conn: &Connection,
    id: &str,
) -> anyhow::Result<Option<ExperienceRecord>> {
    let mut stmt = conn.prepare(
        "SELECT id, domain, cue, lesson, evidence, confidence, task_id, created_at_epoch
         FROM experience_records WHERE id = ?1",
    )?;

    let mut rows = stmt.query(params![id])?;
    if let Some(row) = rows.next()? {
        Ok(Some(ExperienceRecord {
            id: row.get(0)?,
            domain: row.get(1)?,
            cue: row.get(2)?,
            lesson: row.get(3)?,
            evidence: row.get(4)?,
            confidence: row.get(5)?,
            task_id: row.get(6)?,
            created_at_epoch: row.get(7)?,
        }))
    } else {
        Ok(None)
    }
}

pub fn search_experiences(
    conn: &Connection,
    query: &str,
    domain: Option<&str>,
    limit: usize,
) -> anyhow::Result<Vec<ExperienceRecord>> {
    let pattern = format!("%{}%", query.trim().replace('%', "\\%"));

    let query_sql = if let Some(dom) = domain {
        format!(
            "SELECT id, domain, cue, lesson, evidence, confidence, task_id, created_at_epoch
             FROM experience_records
             WHERE domain = '{}' AND (cue LIKE ?1 OR lesson LIKE ?1 OR evidence LIKE ?1)
             ORDER BY created_at_epoch DESC LIMIT {}",
            dom.replace('\'', "''"),
            limit
        )
    } else {
        format!(
            "SELECT id, domain, cue, lesson, evidence, confidence, task_id, created_at_epoch
             FROM experience_records
             WHERE (cue LIKE ?1 OR lesson LIKE ?1 OR evidence LIKE ?1)
             ORDER BY created_at_epoch DESC LIMIT {}",
            limit
        )
    };

    let mut stmt = conn.prepare(&query_sql)?;
    let rows = stmt.query_map(params![pattern], |row| {
        Ok(ExperienceRecord {
            id: row.get(0)?,
            domain: row.get(1)?,
            cue: row.get(2)?,
            lesson: row.get(3)?,
            evidence: row.get(4)?,
            confidence: row.get(5)?,
            task_id: row.get(6)?,
            created_at_epoch: row.get(7)?,
        })
    })?;

    let mut records = Vec::new();
    for r in rows {
        records.push(r?);
    }
    Ok(records)
}

pub fn list_experiences(
    conn: &Connection,
    domain: Option<&str>,
    limit: usize,
) -> anyhow::Result<Vec<ExperienceRecord>> {
    let query_sql = if let Some(dom) = domain {
        format!(
            "SELECT id, domain, cue, lesson, evidence, confidence, task_id, created_at_epoch
             FROM experience_records
             WHERE domain = '{}'
             ORDER BY created_at_epoch DESC LIMIT {}",
            dom.replace('\'', "''"),
            limit
        )
    } else {
        format!(
            "SELECT id, domain, cue, lesson, evidence, confidence, task_id, created_at_epoch
             FROM experience_records
             ORDER BY created_at_epoch DESC LIMIT {}",
            limit
        )
    };

    let mut stmt = conn.prepare(&query_sql)?;
    let rows = stmt.query_map([], |row| {
        Ok(ExperienceRecord {
            id: row.get(0)?,
            domain: row.get(1)?,
            cue: row.get(2)?,
            lesson: row.get(3)?,
            evidence: row.get(4)?,
            confidence: row.get(5)?,
            task_id: row.get(6)?,
            created_at_epoch: row.get(7)?,
        })
    })?;

    let mut records = Vec::new();
    for r in rows {
        records.push(r?);
    }
    Ok(records)
}
