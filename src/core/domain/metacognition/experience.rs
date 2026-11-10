//! Experience Reflection domain module inspired by Google Research Harness².
//!
//! Stores and manages distilled, evidence-backed transferable lessons (`cue`, `lesson`, `evidence`, `confidence`).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExperienceRecord {
    pub id: String,
    pub domain: String,
    pub cue: String,
    pub lesson: String,
    pub evidence: String,
    pub confidence: f64,
    pub task_id: Option<String>,
    pub created_at_epoch: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewExperienceRecord {
    pub domain: String,
    pub cue: String,
    pub lesson: String,
    pub evidence: String,
    pub confidence: Option<f64>,
    pub task_id: Option<String>,
}

impl ExperienceRecord {
    pub fn new(
        id: impl Into<String>,
        domain: impl Into<String>,
        cue: impl Into<String>,
        lesson: impl Into<String>,
        evidence: impl Into<String>,
        confidence: f64,
        task_id: Option<String>,
        created_at_epoch: i64,
    ) -> Self {
        Self {
            id: id.into(),
            domain: domain.into(),
            cue: cue.into(),
            lesson: lesson.into(),
            evidence: evidence.into(),
            confidence: confidence.clamp(0.0, 1.0),
            task_id,
            created_at_epoch,
        }
    }

    /// Emits Markdown adhering to Harness² EXPERIENCE.md format.
    pub fn to_markdown(&self) -> String {
        let task_header = self.task_id.as_deref().unwrap_or(&self.id);
        let mut out = format!("# Experience — {}\n\n", task_header);
        out.push_str(&format!("- domain: `{}`\n", self.domain));
        out.push_str(&format!("- id: `{}`\n\n", self.id));
        out.push_str("## Transferable lesson\n");
        out.push_str(&format!("- **cue**: {}\n", self.cue.trim()));
        out.push_str(&format!("- **lesson**: {}\n", self.lesson.trim()));
        out.push_str(&format!("- **evidence**: {}\n", self.evidence.trim()));
        out.push_str(&format!("- **confidence**: {:.2}\n", self.confidence));
        out
    }

    /// Parses Markdown in Harness² EXPERIENCE.md format into an ExperienceRecord.
    pub fn from_markdown(content: &str, id: &str, default_domain: &str) -> Option<Self> {
        let mut cue = None;
        let mut lesson = None;
        let mut evidence = None;
        let mut confidence = 1.0;
        let mut domain = default_domain.to_string();
        let mut task_id = None;

        for line in content.lines() {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix("# Experience — ") {
                let tid = rest.trim();
                if !tid.is_empty() {
                    task_id = Some(tid.to_string());
                }
            } else if let Some(rest) = trimmed.strip_prefix("- domain: `") {
                if let Some(dom) = rest.strip_suffix('`') {
                    domain = dom.to_string();
                }
            } else if let Some(rest) = trimmed.strip_prefix("- **cue**:") {
                cue = Some(rest.trim().to_string());
            } else if let Some(rest) = trimmed.strip_prefix("- **lesson**:") {
                lesson = Some(rest.trim().to_string());
            } else if let Some(rest) = trimmed.strip_prefix("- **evidence**:") {
                evidence = Some(rest.trim().to_string());
            } else if let Some(rest) = trimmed.strip_prefix("- **confidence**:") {
                if let Ok(c) = rest.trim().parse::<f64>() {
                    confidence = c;
                }
            }
        }

        if let (Some(c), Some(l), Some(e)) = (cue, lesson, evidence) {
            Some(Self {
                id: id.to_string(),
                domain,
                cue: c,
                lesson: l,
                evidence: e,
                confidence,
                task_id,
                created_at_epoch: chrono::Utc::now().timestamp(),
            })
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_experience_markdown_roundtrip() {
        let record = ExperienceRecord::new(
            "exp_test_01",
            "coding_agent",
            "When executing 3D Blender render in PRoot ARM64 container",
            "Do not call blender directly; generate headless Python script or use generate_image",
            "Verified in task triage, avoiding exit code 127",
            0.95,
            Some("task_blender_01".to_string()),
            1731240000,
        );

        let md = record.to_markdown();
        assert!(md.contains("# Experience — task_blender_01"));
        assert!(md.contains("- **cue**: When executing 3D Blender render"));
        assert!(md.contains("- **confidence**: 0.95"));

        let parsed = ExperienceRecord::from_markdown(&md, "exp_test_01", "coding_agent").expect("Failed to parse");
        assert_eq!(parsed.id, "exp_test_01");
        assert_eq!(parsed.domain, "coding_agent");
        assert_eq!(parsed.task_id, Some("task_blender_01".to_string()));
        assert_eq!(parsed.cue, record.cue);
        assert_eq!(parsed.lesson, record.lesson);
        assert_eq!(parsed.evidence, record.evidence);
        assert!((parsed.confidence - 0.95).abs() < 1e-4);
    }
}
