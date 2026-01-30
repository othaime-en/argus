use chrono::{DateTime, Duration, Utc};

/// Format a DateTime as a human-readable string
pub fn format_datetime(dt: &DateTime<Utc>) -> String {
    dt.format("%Y-%m-%d %H:%M:%S UTC").to_string()
}

/// Format a DateTime as a relative time string (e.g., "2 hours ago")
pub fn format_relative(dt: &DateTime<Utc>) -> String {
    let now = Utc::now();
    let diff = now.signed_duration_since(*dt);

    if diff.num_seconds() < 0 {
        return "in the future".to_string();
    }

    if diff.num_seconds() < 60 {
        return format!("{}s ago", diff.num_seconds());
    }

    if diff.num_minutes() < 60 {
        return format!("{}m ago", diff.num_minutes());
    }

    if diff.num_hours() < 24 {
        return format!("{}h ago", diff.num_hours());
    }

    if diff.num_days() < 7 {
        return format!("{}d ago", diff.num_days());
    }

    if diff.num_weeks() < 4 {
        return format!("{}w ago", diff.num_weeks());
    }

    format!("{}mo ago", diff.num_days() / 30)
}

/// Format a Duration as a human-readable string
pub fn format_duration(duration: &Duration) -> String {
    let total_seconds = duration.num_seconds();

    if total_seconds < 0 {
        return "0s".to_string();
    }

    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;

    if hours > 0 {
        format!("{}h {}m {}s", hours, minutes, seconds)
    } else if minutes > 0 {
        format!("{}m {}s", minutes, seconds)
    } else {
        format!("{}s", seconds)
    }
}

/// Format a Duration as a compact string (e.g., "2h 30m")
pub fn format_duration_compact(duration: &Duration) -> String {
    let total_seconds = duration.num_seconds();

    if total_seconds < 0 {
        return "0s".to_string();
    }

    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;

    if hours > 0 {
        if minutes > 0 {
            format!("{}h {}m", hours, minutes)
        } else {
            format!("{}h", hours)
        }
    } else if minutes > 0 {
        format!("{}m", minutes)
    } else {
        format!("{}s", seconds)
    }
}

/// Parse a timestamp string into a DateTime
pub fn parse_timestamp(s: &str) -> Result<DateTime<Utc>, chrono::ParseError> {
    DateTime::parse_from_rfc3339(s).map(|dt| dt.with_timezone(&Utc))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(&Duration::seconds(30)), "30s");
        assert_eq!(format_duration(&Duration::seconds(90)), "1m 30s");
        assert_eq!(format_duration(&Duration::seconds(3665)), "1h 1m 5s");
    }

    #[test]
    fn test_format_duration_compact() {
        assert_eq!(format_duration_compact(&Duration::seconds(30)), "30s");
        assert_eq!(format_duration_compact(&Duration::seconds(90)), "1m");
        assert_eq!(format_duration_compact(&Duration::seconds(3665)), "1h 1m");
    }
}