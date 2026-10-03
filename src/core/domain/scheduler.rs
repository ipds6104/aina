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

use chrono::{DateTime, Datelike, Duration, FixedOffset, NaiveDate, Weekday};

pub struct ScheduleParser;

impl ScheduleParser {
    /// Helper to get local DateTime from epoch and offset_hours.
    fn to_local_datetime(now_epoch: i64, offset_hours: i32) -> (DateTime<FixedOffset>, FixedOffset) {
        let tz = FixedOffset::east_opt(offset_hours * 3600)
            .unwrap_or_else(|| FixedOffset::east_opt(7 * 3600).unwrap());
        let dt = DateTime::from_timestamp(now_epoch, 0)
            .unwrap_or_default()
            .with_timezone(&tz);
        (dt, tz)
    }

    /// Computes epoch seconds for the next occurrence of `HH:MM` in daily recurring mode.
    pub fn compute_next_daily_epoch(hour: u32, minute: u32, offset_hours: i32, now_epoch: i64) -> i64 {
        let (local_dt, _) = Self::to_local_datetime(now_epoch, offset_hours);
        let today = local_dt.date_naive();
        if let Some(target_today) = today.and_hms_opt(hour, minute, 0) {
            if target_today.and_utc().timestamp() - (offset_hours as i64 * 3600) > now_epoch {
                return target_today.and_utc().timestamp() - (offset_hours as i64 * 3600);
            }
        }
        let tomorrow = today + Duration::days(1);
        let target_tomorrow = tomorrow.and_hms_opt(hour, minute, 0).unwrap();
        target_tomorrow.and_utc().timestamp() - (offset_hours as i64 * 3600)
    }

    /// Computes epoch seconds for the next occurrence of `HH:MM` on a workday (Monday through Friday).
    pub fn compute_next_workday_epoch(hour: u32, minute: u32, offset_hours: i32, now_epoch: i64) -> i64 {
        let (local_dt, _) = Self::to_local_datetime(now_epoch, offset_hours);
        let today = local_dt.date_naive();

        // Check if today is Mon..=Fri and time has not passed yet
        if today.weekday().number_from_monday() <= 5 {
            if let Some(target_today) = today.and_hms_opt(hour, minute, 0) {
                let target_epoch = target_today.and_utc().timestamp() - (offset_hours as i64 * 3600);
                if target_epoch > now_epoch {
                    return target_epoch;
                }
            }
        }

        // Search the next 7 days for the first Monday..=Friday
        for day_offset in 1..=7 {
            let candidate_date = today + Duration::days(day_offset);
            if candidate_date.weekday().number_from_monday() <= 5 {
                let target_dt = candidate_date.and_hms_opt(hour, minute, 0).unwrap();
                return target_dt.and_utc().timestamp() - (offset_hours as i64 * 3600);
            }
        }

        // Fallback
        Self::compute_next_daily_epoch(hour, minute, offset_hours, now_epoch)
    }

    /// Gets the last calendar date for the given year and month.
    pub fn get_last_calendar_date(year: i32, month: u32) -> NaiveDate {
        let (next_y, next_m) = if month == 12 {
            (year + 1, 1)
        } else {
            (year, month + 1)
        };
        NaiveDate::from_ymd_opt(next_y, next_m, 1)
            .unwrap()
            .pred_opt()
            .unwrap()
    }

    /// Gets the last business day / workday (Monday through Friday) of the month.
    /// If the last day of the month is Saturday, it steps back to Friday.
    /// If the last day of the month is Sunday, it steps back to Friday.
    pub fn get_last_workday_date(year: i32, month: u32) -> NaiveDate {
        let last_date = Self::get_last_calendar_date(year, month);
        match last_date.weekday() {
            Weekday::Sat => last_date - Duration::days(1),
            Weekday::Sun => last_date - Duration::days(2),
            _ => last_date,
        }
    }

    /// Gets H-1 of the last workday of the month (the business day immediately preceding the last workday).
    pub fn get_last_workday_minus_1_date(year: i32, month: u32) -> NaiveDate {
        let lw = Self::get_last_workday_date(year, month);
        match lw.weekday() {
            Weekday::Mon => lw - Duration::days(3), // Friday before Monday
            _ => lw - Duration::days(1),
        }
    }

    /// Computes epoch seconds for monthly tasks (specific day, last day, last workday, or H-1 last workday).
    pub fn compute_next_monthly_epoch(
        mode: &str,
        day_param: u32,
        hour: u32,
        minute: u32,
        offset_hours: i32,
        now_epoch: i64,
    ) -> i64 {
        let (local_dt, _) = Self::to_local_datetime(now_epoch, offset_hours);
        let current_year = local_dt.year();
        let current_month = local_dt.month();

        // 1. Try current month
        let candidate_curr = Self::resolve_monthly_date(mode, day_param, current_year, current_month);
        let curr_target = candidate_curr.and_hms_opt(hour, minute, 0).unwrap();
        let curr_epoch = curr_target.and_utc().timestamp() - (offset_hours as i64 * 3600);
        if curr_epoch > now_epoch {
            return curr_epoch;
        }

        // 2. Next month
        let (next_y, next_m) = if current_month == 12 {
            (current_year + 1, 1)
        } else {
            (current_year, current_month + 1)
        };
        let candidate_next = Self::resolve_monthly_date(mode, day_param, next_y, next_m);
        let next_target = candidate_next.and_hms_opt(hour, minute, 0).unwrap();
        next_target.and_utc().timestamp() - (offset_hours as i64 * 3600)
    }

    fn resolve_monthly_date(mode: &str, day_param: u32, year: i32, month: u32) -> NaiveDate {
        match mode {
            "last_workday" | "lw" => Self::get_last_workday_date(year, month),
            "last_workday_minus_1" | "lw-1" | "h-1_workday" => {
                Self::get_last_workday_minus_1_date(year, month)
            }
            "last" | "l" => Self::get_last_calendar_date(year, month),
            _ => {
                // Fixed day of month clamped to the max day of that month
                let max_day = Self::get_last_calendar_date(year, month).day();
                let clamped_day = day_param.clamp(1, max_day);
                NaiveDate::from_ymd_opt(year, month, clamped_day).unwrap()
            }
        }
    }

    /// Flexible parser for `schedule_expr`:
    /// - "daily": "08:00"
    /// - "workdays" / "weekday": "08:00" (Mon-Fri)
    /// - "last_workday": "08:00" (Last business day of month)
    /// - "last_workday_minus_1" / "h-1_workday": "08:00" (H-1 business day of month)
    /// - "monthly": "25 08:00", "last 08:00", "last_workday 08:00", "h-1_workday 08:00", "LW 08:00"
    /// - "once": "+2m", "+1h", "22:26", "2026-10-30 08:00", or epoch timestamp
    /// - "interval": seconds or "60m", "1h"
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
            "workdays" | "workday" | "weekdays" | "weekday" => {
                let (hh, mm) = Self::parse_time_hh_mm(clean_expr)?;
                Ok(Self::compute_next_workday_epoch(hh, mm, offset_hours, now_epoch))
            }
            "last_workday" | "monthly_last_workday" | "lw" => {
                let (hh, mm) = Self::parse_time_hh_mm(clean_expr)?;
                Ok(Self::compute_next_monthly_epoch(
                    "last_workday",
                    0,
                    hh,
                    mm,
                    offset_hours,
                    now_epoch,
                ))
            }
            "last_workday_minus_1" | "last_workday_h1" | "monthly_h1_last_workday" | "h-1_workday" | "lw-1" => {
                let (hh, mm) = Self::parse_time_hh_mm(clean_expr)?;
                Ok(Self::compute_next_monthly_epoch(
                    "last_workday_minus_1",
                    0,
                    hh,
                    mm,
                    offset_hours,
                    now_epoch,
                ))
            }
            "monthly" => {
                // Accepts: "25 08:00", "last 08:00", "last_workday 08:00", "h-1_workday 08:00", "LW 08:00", "L 08:00"
                let parts: Vec<&str> = clean_expr.split_whitespace().collect();
                if parts.len() < 2 {
                    anyhow::bail!(
                        "Format 'monthly' harus '<tanggal/rule> <HH:MM>' (contoh: '25 08:00', 'last_workday 08:00', 'h-1_workday 08:00', 'last 08:00')"
                    );
                }
                let rule_str = parts[0].to_lowercase();
                let time_str = parts[1];
                let (hh, mm) = Self::parse_time_hh_mm(time_str)?;

                match rule_str.as_str() {
                    "last_workday" | "lw" | "last-workday" => Ok(Self::compute_next_monthly_epoch(
                        "last_workday",
                        0,
                        hh,
                        mm,
                        offset_hours,
                        now_epoch,
                    )),
                    "last_workday_minus_1" | "last_workday_h1" | "h-1_workday" | "lw-1" | "h-1" => {
                        Ok(Self::compute_next_monthly_epoch(
                            "last_workday_minus_1",
                            0,
                            hh,
                            mm,
                            offset_hours,
                            now_epoch,
                        ))
                    }
                    "last" | "l" | "last_day" => Ok(Self::compute_next_monthly_epoch(
                        "last",
                        0,
                        hh,
                        mm,
                        offset_hours,
                        now_epoch,
                    )),
                    day_str => {
                        let day_num: u32 = day_str.parse().map_err(|_| {
                            anyhow::anyhow!("Tanggal bulan tidak valid: '{}'. Masukkan angka 1-31 atau keyword 'last_workday', 'h-1_workday', 'last'.", day_str)
                        })?;
                        Ok(Self::compute_next_monthly_epoch(
                            "day",
                            day_num,
                            hh,
                            mm,
                            offset_hours,
                            now_epoch,
                        ))
                    }
                }
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

                // Check full datetime format: "YYYY-MM-DD HH:MM" or "YYYY/MM/DD HH:MM"
                if clean_expr.contains('-') || clean_expr.contains('/') {
                    let normalized = clean_expr.replace('/', "-");
                    let parts: Vec<&str> = normalized.split_whitespace().collect();
                    if parts.len() == 2 {
                        let date_parts: Vec<&str> = parts[0].split('-').collect();
                        if date_parts.len() == 3 {
                            if let (Ok(y), Ok(m), Ok(d)) = (
                                date_parts[0].parse::<i32>(),
                                date_parts[1].parse::<u32>(),
                                date_parts[2].parse::<u32>(),
                            ) {
                                if let Ok((hh, mm)) = Self::parse_time_hh_mm(parts[1]) {
                                    if let Some(naive_date) = NaiveDate::from_ymd_opt(y, m, d) {
                                        if let Some(naive_dt) = naive_date.and_hms_opt(hh, mm, 0) {
                                            let target_epoch = naive_dt.and_utc().timestamp()
                                                - (offset_hours as i64 * 3600);
                                            return Ok(target_epoch);
                                        }
                                    }
                                }
                            }
                        }
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
                    "Format waktu 'once' tidak valid: '{}'. Contoh: '+2m', '+1h', '22:26', '2026-10-30 08:00', atau epoch timestamp.",
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
            other => anyhow::bail!(
                "Tipe jadwal tidak didukung: '{}'. Pilihan: 'once', 'daily', 'workdays', 'monthly', 'last_workday', 'last_workday_minus_1', 'interval'.",
                other
            ),
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
    use chrono::Timelike;

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

    #[test]
    fn test_parse_full_datetime_once() {
        // 2026-10-30 08:00 WIB (UTC+7) -> epoch = 1793322000
        let res = ScheduleParser::compute_next_run("once", "2026-10-30 08:00", 7, 1790000000).unwrap();
        let (dt, _) = ScheduleParser::to_local_datetime(res, 7);
        assert_eq!(dt.year(), 2026);
        assert_eq!(dt.month(), 10);
        assert_eq!(dt.day(), 30);
        assert_eq!(dt.hour(), 8);
        assert_eq!(dt.minute(), 0);
    }

    #[test]
    fn test_last_workday_october_2026() {
        // October 2026 has 31 days. Oct 31, 2026 is Saturday.
        // Therefore, the last workday (Mon-Fri) is Friday, Oct 30, 2026!
        let lw = ScheduleParser::get_last_workday_date(2026, 10);
        assert_eq!(lw, NaiveDate::from_ymd_opt(2026, 10, 30).unwrap());
        assert_eq!(lw.weekday(), Weekday::Fri);

        // H-1 last workday is Thursday, Oct 29, 2026!
        let h1 = ScheduleParser::get_last_workday_minus_1_date(2026, 10);
        assert_eq!(h1, NaiveDate::from_ymd_opt(2026, 10, 29).unwrap());
        assert_eq!(h1.weekday(), Weekday::Thu);
    }

    #[test]
    fn test_compute_next_run_last_workday_and_h1() {
        // Mid-October 2026: 2026-10-15 08:00 WIB
        let mid_oct = NaiveDate::from_ymd_opt(2026, 10, 15)
            .unwrap()
            .and_hms_opt(8, 0, 0)
            .unwrap()
            .and_utc()
            .timestamp() - (7 * 3600);

        // Next last_workday should be 2026-10-30 08:00 WIB
        let next_lw = ScheduleParser::compute_next_run("last_workday", "08:00", 7, mid_oct).unwrap();
        let (dt_lw, _) = ScheduleParser::to_local_datetime(next_lw, 7);
        assert_eq!(dt_lw.date_naive(), NaiveDate::from_ymd_opt(2026, 10, 30).unwrap());
        assert_eq!(dt_lw.hour(), 8);

        // Next last_workday_minus_1 should be 2026-10-29 08:00 WIB
        let next_h1 = ScheduleParser::compute_next_run("last_workday_minus_1", "08:00", 7, mid_oct).unwrap();
        let (dt_h1, _) = ScheduleParser::to_local_datetime(next_h1, 7);
        assert_eq!(dt_h1.date_naive(), NaiveDate::from_ymd_opt(2026, 10, 29).unwrap());
        assert_eq!(dt_h1.hour(), 8);
    }

    #[test]
    fn test_compute_next_run_workdays_skips_weekends() {
        // Saturday morning: 2026-10-03 08:00 WIB
        let sat_morning = NaiveDate::from_ymd_opt(2026, 10, 3)
            .unwrap()
            .and_hms_opt(8, 0, 0)
            .unwrap()
            .and_utc()
            .timestamp() - (7 * 3600);

        // Next workday run should be Monday, 2026-10-05 08:00 WIB!
        let next_workday = ScheduleParser::compute_next_run("workdays", "08:00", 7, sat_morning).unwrap();
        let (dt, _) = ScheduleParser::to_local_datetime(next_workday, 7);
        assert_eq!(dt.date_naive(), NaiveDate::from_ymd_opt(2026, 10, 5).unwrap());
        assert_eq!(dt.weekday(), Weekday::Mon);
        assert_eq!(dt.hour(), 8);
    }

    #[test]
    fn test_compute_next_run_monthly_specific_day() {
        // 2026-10-10 08:00 WIB
        let oct_10 = NaiveDate::from_ymd_opt(2026, 10, 10)
            .unwrap()
            .and_hms_opt(8, 0, 0)
            .unwrap()
            .and_utc()
            .timestamp() - (7 * 3600);

        // "25 08:00" -> should be 2026-10-25 08:00
        let next_25 = ScheduleParser::compute_next_run("monthly", "25 08:00", 7, oct_10).unwrap();
        let (dt_25, _) = ScheduleParser::to_local_datetime(next_25, 7);
        assert_eq!(dt_25.date_naive(), NaiveDate::from_ymd_opt(2026, 10, 25).unwrap());

        // After the 25th (e.g. 2026-10-26), next run should advance to 2026-11-25!
        let oct_26 = NaiveDate::from_ymd_opt(2026, 10, 26)
            .unwrap()
            .and_hms_opt(8, 0, 0)
            .unwrap()
            .and_utc()
            .timestamp() - (7 * 3600);
        let next_nov_25 = ScheduleParser::compute_next_run("monthly", "25 08:00", 7, oct_26).unwrap();
        let (dt_nov_25, _) = ScheduleParser::to_local_datetime(next_nov_25, 7);
        assert_eq!(dt_nov_25.date_naive(), NaiveDate::from_ymd_opt(2026, 11, 25).unwrap());
    }
}
