use serde::{Deserialize, Serialize};
use crate::core::domain::{NewScheduledTask, ScheduleParser, ScheduledTask, ScheduledTaskType};
use crate::core::ports::SessionStorePort;

pub const DEFAULT_BASELINE_JSON: &str = include_str!("../../../config/activities.default.json");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivityCategory {
    Status,
    Briefing,
    Reminder,
    Maintenance,
    Custom,
}

impl ActivityCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            ActivityCategory::Status => "status",
            ActivityCategory::Briefing => "briefing",
            ActivityCategory::Reminder => "reminder",
            ActivityCategory::Maintenance => "maintenance",
            ActivityCategory::Custom => "custom",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().trim() {
            "status" | "story" => ActivityCategory::Status,
            "briefing" | "agenda" => ActivityCategory::Briefing,
            "reminder" | "alarm" | "pengingat" => ActivityCategory::Reminder,
            "maintenance" | "groom" | "heal" => ActivityCategory::Maintenance,
            _ => ActivityCategory::Custom,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivitySource {
    Baseline,
    Custom,
}

impl ActivitySource {
    pub fn as_str(&self) -> &'static str {
        match self {
            ActivitySource::Baseline => "baseline",
            ActivitySource::Custom => "custom",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().trim() {
            "baseline" | "builtin" | "built-in" | "default" => ActivitySource::Baseline,
            _ => ActivitySource::Custom,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Activity {
    pub id: Option<i64>,
    pub slug: String,
    pub title: String,
    pub category: ActivityCategory,
    pub task_type: ScheduledTaskType,
    pub target_jid: String,
    pub schedule_type: String,
    pub schedule_expr: String,
    pub payload: String,
    pub source: ActivitySource,
    pub is_active: bool,
    #[serde(default)]
    pub next_run_epoch: Option<i64>,
    #[serde(default)]
    pub last_run_epoch: Option<i64>,
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
}

impl Activity {
    /// Compute the next execution epoch relative to reference time.
    pub fn compute_next_epoch(&self, timezone_offset_hours: i32, now_epoch: i64) -> anyhow::Result<i64> {
        ScheduleParser::compute_next_run(
            &self.schedule_type,
            &self.schedule_expr,
            timezone_offset_hours,
            now_epoch,
        )
    }

    pub fn to_new_scheduled_task(&self, fallback_now_epoch: i64) -> NewScheduledTask {
        NewScheduledTask {
            title: self.title.clone(),
            task_type: self.task_type,
            target_jid: self.target_jid.clone(),
            payload: self.payload.clone(),
            schedule_type: self.schedule_type.clone(),
            schedule_expr: self.schedule_expr.clone(),
            next_run_epoch: self.next_run_epoch.unwrap_or(fallback_now_epoch),
            slug: Some(self.slug.clone()),
            category: Some(self.category.as_str().to_string()),
            source: Some(self.source.as_str().to_string()),
            metadata: self.metadata.as_ref().map(|v| v.to_string()),
        }
    }
}

impl From<&ScheduledTask> for Activity {
    fn from(task: &ScheduledTask) -> Self {
        let slug = task.slug.clone().unwrap_or_else(|| format!("task_{}", task.id));
        let title = task.title.clone();
        let category = task.category.as_deref().map(ActivityCategory::from_str).unwrap_or(ActivityCategory::Custom);
        let source = task.source.as_deref().map(ActivitySource::from_str).unwrap_or(ActivitySource::Custom);
        let metadata = task.metadata.as_deref().and_then(|s| serde_json::from_str(s).ok());
        Activity {
            id: Some(task.id),
            slug,
            title,
            category,
            task_type: task.task_type,
            target_jid: task.target_jid.clone(),
            schedule_type: task.schedule_type.clone(),
            schedule_expr: task.schedule_expr.clone(),
            payload: task.payload.clone(),
            source,
            is_active: task.is_active,
            next_run_epoch: Some(task.next_run_epoch),
            last_run_epoch: task.last_run_epoch,
            metadata,
        }
    }
}

impl From<ScheduledTask> for Activity {
    fn from(task: ScheduledTask) -> Self {
        Activity::from(&task)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaselineActivityDefinition {
    pub slug: String,
    pub title: String,
    pub category: ActivityCategory,
    pub task_type: ScheduledTaskType,
    pub target_jid: String,
    pub schedule_type: String,
    pub schedule_expr: String,
    pub payload: String,
    #[serde(default = "default_true")]
    pub is_active: bool,
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
}

fn default_true() -> bool {
    true
}

impl BaselineActivityDefinition {
    pub fn to_activity(&self, timezone_offset_hours: i32, now_epoch: i64) -> Activity {
        let next_epoch = ScheduleParser::compute_next_run(
            &self.schedule_type,
            &self.schedule_expr,
            timezone_offset_hours,
            now_epoch,
        )
        .ok();

        Activity {
            id: None,
            slug: self.slug.clone(),
            title: self.title.clone(),
            category: self.category,
            task_type: self.task_type,
            target_jid: self.target_jid.clone(),
            schedule_type: self.schedule_type.clone(),
            schedule_expr: self.schedule_expr.clone(),
            payload: self.payload.clone(),
            source: ActivitySource::Baseline,
            is_active: self.is_active,
            next_run_epoch: next_epoch,
            last_run_epoch: None,
            metadata: self.metadata.clone(),
        }
    }
}

/// Helper to parse baseline activities catalog from raw JSON.
pub fn parse_baseline_catalog(json_content: &str) -> anyhow::Result<Vec<BaselineActivityDefinition>> {
    let list: Vec<BaselineActivityDefinition> = serde_json::from_str(json_content)?;
    Ok(list)
}

/// Loads default baseline activities embedded in the binary or overridden from config/activities.default.json.
pub fn load_default_baseline() -> Vec<BaselineActivityDefinition> {
    if let Ok(file_content) = std::fs::read_to_string("config/activities.default.json") {
        if let Ok(list) = parse_baseline_catalog(&file_content) {
            return list;
        }
    }
    parse_baseline_catalog(DEFAULT_BASELINE_JSON).unwrap_or_default()
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ReconcileReport {
    pub created_baseline: usize,
    pub existing_baseline: usize,
    pub created_custom: usize,
    pub updated_custom: usize,
    pub errors: Vec<String>,
}

pub struct ActivityReconciler;

impl ActivityReconciler {
    /// Reconciles baseline activities against the session store.
    /// Idempotent:
    /// - If a baseline task does not exist, it is inserted into the store with source="baseline".
    /// - If a baseline task already exists, its `is_active` state is preserved (user overrides are respected).
    pub async fn reconcile_baseline(
        store: &dyn SessionStorePort,
        baseline: &[BaselineActivityDefinition],
        timezone_offset_hours: i32,
        now_epoch: i64,
    ) -> anyhow::Result<ReconcileReport> {
        let mut report = ReconcileReport::default();

        for def in baseline {
            match store.get_scheduled_task_by_slug(&def.slug).await {
                Ok(Some(_existing)) => {
                    // Task already exists: preserve user state & active toggle.
                    report.existing_baseline += 1;
                }
                Ok(None) => {
                    let next_run = ScheduleParser::compute_next_run(
                        &def.schedule_type,
                        &def.schedule_expr,
                        timezone_offset_hours,
                        now_epoch,
                    )
                    .unwrap_or(now_epoch);

                    let new_task = NewScheduledTask {
                        title: def.title.clone(),
                        task_type: def.task_type,
                        target_jid: def.target_jid.clone(),
                        payload: def.payload.clone(),
                        schedule_type: def.schedule_type.clone(),
                        schedule_expr: def.schedule_expr.clone(),
                        next_run_epoch: next_run,
                        slug: Some(def.slug.clone()),
                        category: Some(def.category.as_str().to_string()),
                        source: Some(ActivitySource::Baseline.as_str().to_string()),
                        metadata: def.metadata.as_ref().map(|m| m.to_string()),
                    };

                    match store.create_scheduled_task(&new_task).await {
                        Ok(_) => {
                            report.created_baseline += 1;
                        }
                        Err(e) => {
                            report.errors.push(format!("Failed to insert baseline {}: {}", def.slug, e));
                        }
                    }
                }
                Err(e) => {
                    report.errors.push(format!("Failed to query task {}: {}", def.slug, e));
                }
            }
        }

        Ok(report)
    }

    /// Reconciles custom activity files from a directory (e.g. /app/data/activities or ./data/activities).
    /// Scans for .json files and upserts them as source="custom".
    pub async fn reconcile_custom_dir(
        store: &dyn SessionStorePort,
        dir: &std::path::Path,
        timezone_offset_hours: i32,
        now_epoch: i64,
    ) -> anyhow::Result<ReconcileReport> {
        let mut report = ReconcileReport::default();
        if !dir.exists() || !dir.is_dir() {
            return Ok(report);
        }

        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(e) => {
                report.errors.push(format!("Failed to read custom activities dir: {}", e));
                return Ok(report);
            }
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) == Some("json") {
                let content = match std::fs::read_to_string(&path) {
                    Ok(c) => c,
                    Err(e) => {
                        report.errors.push(format!("Failed to read file {}: {}", path.display(), e));
                        continue;
                    }
                };

                let items: Vec<BaselineActivityDefinition> = if let Ok(single) = serde_json::from_str::<BaselineActivityDefinition>(&content) {
                    vec![single]
                } else if let Ok(list) = serde_json::from_str::<Vec<BaselineActivityDefinition>>(&content) {
                    list
                } else {
                    report.errors.push(format!("Invalid activity JSON format in {}", path.display()));
                    continue;
                };

                for def in items {
                    match store.get_scheduled_task_by_slug(&def.slug).await {
                        Ok(Some(existing)) => {
                            let _ = store.update_scheduled_task(
                                existing.id,
                                Some(&def.title),
                                Some(&def.target_jid),
                                Some(&def.payload),
                                Some(&def.schedule_type),
                                Some(&def.schedule_expr),
                                Some(existing.next_run_epoch),
                                Some(def.category.as_str()),
                            ).await;
                            report.updated_custom += 1;
                        }
                        Ok(None) => {
                            let next_run = ScheduleParser::compute_next_run(
                                &def.schedule_type,
                                &def.schedule_expr,
                                timezone_offset_hours,
                                now_epoch,
                            )
                            .unwrap_or(now_epoch);

                            let new_task = NewScheduledTask {
                                title: def.title.clone(),
                                task_type: def.task_type,
                                target_jid: def.target_jid.clone(),
                                payload: def.payload.clone(),
                                schedule_type: def.schedule_type.clone(),
                                schedule_expr: def.schedule_expr.clone(),
                                next_run_epoch: next_run,
                                slug: Some(def.slug.clone()),
                                category: Some(def.category.as_str().to_string()),
                                source: Some(ActivitySource::Custom.as_str().to_string()),
                                metadata: def.metadata.as_ref().map(|m| m.to_string()),
                            };

                            if let Ok(_) = store.create_scheduled_task(&new_task).await {
                                report.created_custom += 1;
                            }
                        }
                        Err(e) => {
                            report.errors.push(format!("Failed checking slug {}: {}", def.slug, e));
                        }
                    }
                }
            }
        }

        Ok(report)
    }

    /// Full sync: baseline + custom files.
    pub async fn sync_all(
        store: &dyn SessionStorePort,
        baseline: &[BaselineActivityDefinition],
        custom_dir: Option<&std::path::Path>,
        timezone_offset_hours: i32,
        now_epoch: i64,
    ) -> anyhow::Result<ReconcileReport> {
        let mut report = Self::reconcile_baseline(store, baseline, timezone_offset_hours, now_epoch).await?;
        if let Some(dir) = custom_dir {
            let custom_report = Self::reconcile_custom_dir(store, dir, timezone_offset_hours, now_epoch).await?;
            report.created_custom += custom_report.created_custom;
            report.updated_custom += custom_report.updated_custom;
            report.errors.extend(custom_report.errors);
        }
        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_activity_category_parsing() {
        assert_eq!(ActivityCategory::from_str("status"), ActivityCategory::Status);
        assert_eq!(ActivityCategory::from_str("STORY"), ActivityCategory::Status);
        assert_eq!(ActivityCategory::from_str("briefing"), ActivityCategory::Briefing);
        assert_eq!(ActivityCategory::from_str("reminder"), ActivityCategory::Reminder);
        assert_eq!(ActivityCategory::from_str("maintenance"), ActivityCategory::Maintenance);
        assert_eq!(ActivityCategory::from_str("something_else"), ActivityCategory::Custom);
    }

    #[test]
    fn test_activity_source_parsing() {
        assert_eq!(ActivitySource::from_str("baseline"), ActivitySource::Baseline);
        assert_eq!(ActivitySource::from_str("builtin"), ActivitySource::Baseline);
        assert_eq!(ActivitySource::from_str("custom"), ActivitySource::Custom);
    }

    #[test]
    fn test_parse_baseline_catalog() {
        let raw = r#"[
            {
                "slug": "status_pagi",
                "title": "Status WhatsApp Aina - Pagi",
                "category": "status",
                "task_type": "agent_action",
                "target_jid": "status@broadcast",
                "schedule_type": "daily",
                "schedule_expr": "07:00",
                "payload": "Eksekusi status pagi via scripts/persona_status.py",
                "is_active": true
            }
        ]"#;

        let parsed = parse_baseline_catalog(raw).expect("Must parse valid JSON catalog");
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].slug, "status_pagi");
        assert_eq!(parsed[0].category, ActivityCategory::Status);
        assert_eq!(parsed[0].task_type, ScheduledTaskType::AgentAction);

        let act = parsed[0].to_activity(7, 1794000000);
        assert_eq!(act.source, ActivitySource::Baseline);
        assert!(act.next_run_epoch.is_some());
    }

    #[test]
    fn test_load_default_baseline_contains_standard_tasks() {
        let baseline = load_default_baseline();
        assert_eq!(baseline.len(), 8);

        let slugs: Vec<&str> = baseline.iter().map(|b| b.slug.as_str()).collect();
        assert!(slugs.contains(&"status_pagi"));
        assert!(slugs.contains(&"status_siang"));
        assert!(slugs.contains(&"status_senja"));
        assert!(slugs.contains(&"status_malam"));
        assert!(slugs.contains(&"morning_briefing"));
        assert!(slugs.contains(&"afternoon_deadline_check"));
        assert!(slugs.contains(&"eom_presence_h_minus_1"));
        assert!(slugs.contains(&"eom_presence_h"));
    }
}
