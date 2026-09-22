use chrono::{DateTime, NaiveDate, NaiveDateTime, TimeZone, Utc};

fn month_str_to_num(m: &str) -> Option<u32> {
    match m.to_ascii_lowercase().as_str() {
        "jan" => Some(1),
        "feb" => Some(2),
        "mar" => Some(3),
        "apr" => Some(4),
        "may" => Some(5),
        "jun" => Some(6),
        "jul" => Some(7),
        "aug" => Some(8),
        "sep" => Some(9),
        "oct" => Some(10),
        "nov" => Some(11),
        "dec" => Some(12),
        _ => None,
    }
}

/// Parse RFC3164 timestamp like "Oct 11 22:14:15" or "Oct  5 08:00:01" into `DateTime<Utc>` with year inference.
/// Uses `now` as reference for year inference; if the resulting datetime is in the future, subtract one year.
pub fn rfc3164_to_datetime(timestamp: &str, now: DateTime<Utc>) -> Result<DateTime<Utc>, String> {
    // timestamp is already formatted as "Mon DD HH:MM:SS" e.g. "Oct 11 22:14:15"
    // Split by whitespace: month, day, time
    let parts: Vec<&str> = timestamp.split_whitespace().collect();
    if parts.len() != 3 {
        return Err(format!("invalid RFC3164 timestamp '{timestamp}'"));
    }
    let month = month_str_to_num(parts[0]).ok_or_else(|| format!("invalid month '{}'", parts[0]))?;
    let day: u32 = parts[1].parse().map_err(|_| format!("invalid day '{}'", parts[1]))?;
    let time_parts: Vec<&str> = parts[2].split(':').collect();
    if time_parts.len() != 3 {
        return Err(format!("invalid time '{}'", parts[2]));
    }
    let hour: u32 = time_parts[0].parse().map_err(|_| format!("invalid hour '{}'", time_parts[0]))?;
    let minute: u32 = time_parts[1].parse().map_err(|_| format!("invalid minute '{}'", time_parts[1]))?;
    let second: u32 = time_parts[2].parse().map_err(|_| format!("invalid second '{}'", time_parts[2]))?;

    let year = now.format("%Y").to_string().parse::<i32>().unwrap_or(1970);
    let naive = NaiveDate::from_ymd_opt(year, month, day)
        .and_then(|d| d.and_hms_opt(hour, minute, second))
        .ok_or_else(|| format!("invalid date '{timestamp}'"))?;
    let mut dt = Utc.from_utc_datetime(&naive);
    // If future, assume previous year (log from Dec in Jan)
    if dt > now {
        if let Some(prev) = NaiveDate::from_ymd_opt(year - 1, month, day).and_then(|d| d.and_hms_opt(hour, minute, second)) {
            dt = Utc.from_utc_datetime(&prev);
        }
    }
    Ok(dt)
}

pub fn parse_human_time(s: &str) -> Result<DateTime<Utc>, String> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Err("empty time string".to_string());
    }
    let lower = trimmed.to_ascii_lowercase();

    // Handle special keywords
    if lower == "now" {
        return Ok(Utc::now());
    }
    if lower == "today" {
        let now = Utc::now();
        let date = now.date_naive();
        let naive = date.and_hms_opt(0, 0, 0).unwrap();
        return Ok(Utc.from_utc_datetime(&naive));
    }
    if lower == "yesterday" {
        let now = Utc::now();
        let date = now.date_naive() - chrono::Duration::days(1);
        let naive = date.and_hms_opt(0, 0, 0).unwrap();
        return Ok(Utc.from_utc_datetime(&naive));
    }

    // Handle "now-2h", "now - 2h", "now-15days"
    if lower.starts_with("now") {
        let rest = trimmed[3..].trim();
        if rest.starts_with('-') || rest.starts_with('+') {
            let dur_str = rest[1..].trim();
            if !dur_str.is_empty() {
                let dur = humantime::parse_duration(dur_str)
                    .map_err(|e| format!("invalid duration '{dur_str}': {e}"))?;
                let chrono_dur = chrono::Duration::from_std(dur).map_err(|e| e.to_string())?;
                if rest.starts_with('-') {
                    return Ok(Utc::now() - chrono_dur);
                } else {
                    return Ok(Utc::now() + chrono_dur);
                }
            }
        }
    }

    // Handle "... ago" suffix
    if lower.ends_with("ago") {
        let without_ago = trimmed[..trimmed.len() - 3].trim();
        // humantime can parse "15 days", "2h", etc. with spaces
        if let Ok(dur) = humantime::parse_duration(without_ago) {
            let chrono_dur = chrono::Duration::from_std(dur).map_err(|e| e.to_string())?;
            return Ok(Utc::now() - chrono_dur);
        }
        // Try without spaces for cases like "15days"
        let no_space: String = without_ago.chars().filter(|c| !c.is_whitespace()).collect();
        if let Ok(dur) = humantime::parse_duration(&no_space) {
            let chrono_dur = chrono::Duration::from_std(dur).map_err(|e| e.to_string())?;
            return Ok(Utc::now() - chrono_dur);
        }
        return Err(format!("invalid relative time '{s}'"));
    }

    // Try humantime duration directly as "2h", "15days" meaning ago
    if let Ok(dur) = humantime::parse_duration(trimmed) {
        let chrono_dur = chrono::Duration::from_std(dur).map_err(|e| e.to_string())?;
        return Ok(Utc::now() - chrono_dur);
    }
    // Also try with spaces removed
    let no_space: String = trimmed.chars().filter(|c| !c.is_whitespace()).collect();
    if let Ok(dur) = humantime::parse_duration(&no_space) {
        // Only treat as duration if the original contained time unit hints and not a date
        // To avoid mis-parsing "2024-01-01" as duration, check if it contains '-' and digit
        // but humantime would fail for date anyway, so safe
        if trimmed.chars().any(|c| c.is_ascii_alphabetic()) {
            let chrono_dur = chrono::Duration::from_std(dur).map_err(|e| e.to_string())?;
            return Ok(Utc::now() - chrono_dur);
        }
    }

    // Try absolute datetime parsing (UTC)
    // RFC3339 first
    if let Ok(dt) = DateTime::parse_from_rfc3339(trimmed) {
        return Ok(dt.with_timezone(&Utc));
    }
    // Try "2024-01-01T10:00:00" without tz
    if let Ok(naive) = NaiveDateTime::parse_from_str(trimmed, "%Y-%m-%dT%H:%M:%S") {
        return Ok(Utc.from_utc_datetime(&naive));
    }
    if let Ok(naive) = NaiveDateTime::parse_from_str(trimmed, "%Y-%m-%d %H:%M:%S") {
        return Ok(Utc.from_utc_datetime(&naive));
    }
    // Date only
    if let Ok(date) = NaiveDate::parse_from_str(trimmed, "%Y-%m-%d") {
        let naive = date.and_hms_opt(0, 0, 0).unwrap();
        return Ok(Utc.from_utc_datetime(&naive));
    }
    // Try with humantime-like "2024/01/01"
    if let Ok(date) = NaiveDate::parse_from_str(trimmed, "%Y/%m/%d") {
        let naive = date.and_hms_opt(0, 0, 0).unwrap();
        return Ok(Utc.from_utc_datetime(&naive));
    }

    Err(format!("invalid time '{s}' — try '2024-01-01', '2024-01-01T10:00:00', '15 days ago', '2h ago', 'now-2h', 'today', 'yesterday'"))
}

pub fn datetime_to_micros(dt: DateTime<Utc>) -> u64 {
    let dur = dt.timestamp_micros();
    if dur < 0 { 0 } else { dur as u64 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn parse_absolute_date() {
        let dt = parse_human_time("2024-01-01").expect("parse");
        assert_eq!(dt, Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap());
    }

    #[test]
    fn parse_absolute_datetime() {
        let dt = parse_human_time("2024-01-01T10:00:00").expect("parse");
        assert_eq!(dt, Utc.with_ymd_and_hms(2024, 1, 1, 10, 0, 0).unwrap());
    }

    #[test]
    fn parse_relative_ago() {
        let before = Utc::now();
        let dt = parse_human_time("2h ago").expect("parse");
        let after = Utc::now();
        // dt should be ~2h before now
        let diff = before.signed_duration_since(dt).num_seconds();
        assert!(diff >= 7100 && diff <= 7300, "diff {diff}");
        let _ = after;
    }

    #[test]
    fn parse_now_minus() {
        let dt = parse_human_time("now-2h").expect("parse");
        let diff = Utc::now().signed_duration_since(dt).num_seconds();
        assert!(diff >= 7100 && diff <= 7300);
    }

    #[test]
    fn parse_today_yesterday() {
        let today = parse_human_time("today").expect("parse");
        assert_eq!(today.format("%H:%M:%S").to_string(), "00:00:00");
        let yest = parse_human_time("yesterday").expect("parse");
        assert!(yest < today);
    }

    #[test]
    fn parse_invalid_returns_err() {
        assert!(parse_human_time("not-a-date").is_err());
        assert!(parse_human_time("").is_err());
    }

    #[test]
    fn rfc3164_basic() {
        let now = Utc.with_ymd_and_hms(2024, 10, 12, 0, 0, 0).unwrap();
        let dt = rfc3164_to_datetime("Oct 11 22:14:15", now).expect("parse");
        assert_eq!(dt, Utc.with_ymd_and_hms(2024, 10, 11, 22, 14, 15).unwrap());
    }

    #[test]
    fn rfc3164_double_space_day() {
        let now = Utc.with_ymd_and_hms(2024, 10, 12, 0, 0, 0).unwrap();
        let dt = rfc3164_to_datetime("Oct  5 08:00:01", now).expect("parse");
        assert_eq!(dt, Utc.with_ymd_and_hms(2024, 10, 5, 8, 0, 1).unwrap());
    }

    #[test]
    fn rfc3164_year_rollover() {
        // Log from Dec 31 when now is Jan 1 next year -> should infer previous year
        let now = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap();
        let dt = rfc3164_to_datetime("Dec 31 23:59:59", now).expect("parse");
        assert_eq!(dt, Utc.with_ymd_and_hms(2024, 12, 31, 23, 59, 59).unwrap());
    }

    #[test]
    fn rfc3164_future_is_previous_year() {
        let now = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
        // Oct 11 is in future relative to Jan 1 2024, so should be 2023
        let dt = rfc3164_to_datetime("Oct 11 22:14:15", now).expect("parse");
        assert_eq!(dt, Utc.with_ymd_and_hms(2023, 10, 11, 22, 14, 15).unwrap());
    }
}
