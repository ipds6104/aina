use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScheduledTaskType {
    AgentAction,
    DirectNotification,
}

impl ScheduledTaskType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ScheduledTaskType::AgentAction => "agent_action",
            ScheduledTaskType::DirectNotification => "direct_notification",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().trim() {
            "agent_action" | "agent" | "action" | "research" => ScheduledTaskType::AgentAction,
            _ => ScheduledTaskType::DirectNotification,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledTask {
    pub id: i64,
    pub title: String,
    pub task_type: ScheduledTaskType,
    pub target_jid: String,
    pub payload: String,
    pub schedule_type: String, // "once", "daily", "interval"
    pub schedule_expr: String, // "YYYY-MM-DD HH:MM", "HH:MM", or "<seconds>"
    pub next_run_epoch: i64,
    pub last_run_epoch: Option<i64>,
    pub last_status: Option<String>,
    pub last_error: Option<String>,
    pub last_duration_secs: Option<f64>,
    pub is_active: bool,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledTaskRun {
    pub id: i64,
    pub task_id: i64,
    pub task_title: String,
    pub target_jid: String,
    pub status: String, // "success" or "failed"
    pub duration_secs: f64,
    pub error_message: Option<String>,
    pub output_preview: Option<String>,
    pub executed_at_epoch: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulerDiagnostics {
    pub total_tasks: usize,
    pub active_tasks: usize,
    pub total_runs: usize,
    pub successful_runs: usize,
    pub failed_runs: usize,
    pub last_run: Option<ScheduledTaskRun>,
    pub last_failure: Option<ScheduledTaskRun>,
    pub next_task: Option<ScheduledTask>,
}

#[derive(Debug, Clone)]
pub struct NewScheduledTask {
    pub title: String,
    pub task_type: ScheduledTaskType,
    pub target_jid: String,
    pub payload: String,
    pub schedule_type: String,
    pub schedule_expr: String,
    pub next_run_epoch: i64,
}

pub struct ScheduleParser;

impl ScheduleParser {
    /// Computes epoch seconds for the next occurrence of `HH:MM` in the given timezone offset (hours).
    pub fn compute_next_daily_epoch(hour: u32, minute: u32, offset_hours: i32, now_epoch: i64) -> i64 {
        let local_secs = now_epoch + (offset_hours as i64 * 3600);
        let days = local_secs.div_euclid(86400);
        let rem_secs = local_secs.rem_euclid(86400);
        let current_day_target_local_secs = (days * 86400) + (hour as i64 * 3600) + (minute as i64 * 60);

        let target_local_secs = if (rem_secs / 60) >= ((hour as i64 * 60) + minute as i64) {
            // Already passed today in local time, schedule for tomorrow (+86400)
            current_day_target_local_secs + 86400
        } else {
            current_day_target_local_secs
        };

        target_local_secs - (offset_hours as i64 * 3600)
    }

    /// Flexible parser for `schedule_expr`:
    /// - "daily" with "07:30" or "7:30" or "07.30"
    /// - "once" with "+2m", "+10m", "+1h", "22:26", or "2026-09-14 22:26"
    /// - "interval" with seconds (e.g. "3600", "60m")
    pub fn compute_next_run(
        schedule_type: &str,
        schedule_expr: &str,
        offset_hours: i32,
        now_epoch: i64,
    ) -> anyhow::Result<i64> {
        let clean_type = schedule_type.to_lowercase().trim().to_string();
        let clean_expr = schedule_expr.trim();

        match clean_type.as_str() {
            "daily" => {
                let (hh, mm) = Self::parse_time_hh_mm(clean_expr)?;
                Ok(Self::compute_next_daily_epoch(hh, mm, offset_hours, now_epoch))
            }
            "once" => {
                // Check relative offsets: "+2m", "+30m", "+1h", "+120s"
                if clean_expr.starts_with('+') {
                    let rel = &clean_expr[1..];
                    if let Some(mins) = rel.strip_suffix('m').or_else(|| rel.strip_suffix("min")) {
                        let m: i64 = mins.parse()?;
                        return Ok(now_epoch + (m * 60));
                    }
                    if let Some(hrs) = rel.strip_suffix('h').or_else(|| rel.strip_suffix("jam")) {
                        let h: i64 = hrs.parse()?;
                        return Ok(now_epoch + (h * 3600));
                    }
                    if let Some(secs) = rel.strip_suffix('s').or_else(|| rel.strip_suffix("detik")) {
                        let s: i64 = secs.parse()?;
                        return Ok(now_epoch + s);
                    }
                }

                // Check "HH:MM" for today/tomorrow once
                if clean_expr.contains(':') || clean_expr.contains('.') {
                    if let Ok((hh, mm)) = Self::parse_time_hh_mm(clean_expr) {
                        return Ok(Self::compute_next_daily_epoch(hh, mm, offset_hours, now_epoch));
                    }
                }

                // Fallback: check if integer epoch seconds was passed
                if let Ok(epoch) = clean_expr.parse::<i64>() {
                    return Ok(epoch);
                }

                anyhow::bail!(
                    "Format waktu 'once' tidak valid: '{}'. Contoh: '+2m', '+1h', '22:26', atau epoch timestamp.",
                    clean_expr
                );
            }
            "interval" => {
                let secs = if clean_expr.ends_with('m') {
                    let m: i64 = clean_expr.trim_end_matches('m').parse()?;
                    m * 60
                } else if clean_expr.ends_with('h') {
                    let h: i64 = clean_expr.trim_end_matches('h').parse()?;
                    h * 3600
                } else {
                    clean_expr.parse::<i64>()?
                };
                Ok(now_epoch + secs.max(10))
            }
            other => anyhow::bail!("Tipe jadwal tidak didukung: '{}'. Pilihan: 'once', 'daily', 'interval'.", other),
        }
    }

    fn parse_time_hh_mm(expr: &str) -> anyhow::Result<(u32, u32)> {
        let cleaned = expr.replace('.', ":");
        let parts: Vec<&str> = cleaned.split(':').collect();
        if parts.len() < 2 {
            anyhow::bail!("Format jam harus HH:MM (contoh: '07:30', '22:26')");
        }
        let hh: u32 = parts[0].trim().parse()?;
        let mm: u32 = parts[1].trim().parse()?;
        if hh > 23 || mm > 59 {
            anyhow::bail!("Jam ({}) atau menit ({}) di luar batas valid (00:00 - 23:59)", hh, mm);
        }
        Ok((hh, mm))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_relative_once() {
        let now = 10000;
        let res = ScheduleParser::compute_next_run("once", "+5m", 7, now).unwrap();
        assert_eq!(res, 10000 + 300);

        let res2 = ScheduleParser::compute_next_run("once", "+2h", 7, now).unwrap();
        assert_eq!(res2, 10000 + 7200);
    }

    #[test]
    fn test_parse_daily() {
        // Assume now is 1700000000
        let now = 1700000000;
        let next = ScheduleParser::compute_next_run("daily", "07:30", 7, now).unwrap();
        assert!(next >= now);
    }

    #[test]
    fn test_task_type_parsing() {
        assert_eq!(ScheduledTaskType::from_str("agent"), ScheduledTaskType::AgentAction);
        assert_eq!(ScheduledTaskType::from_str("research"), ScheduledTaskType::AgentAction);
        assert_eq!(ScheduledTaskType::from_str("notify"), ScheduledTaskType::DirectNotification);
    }
}
