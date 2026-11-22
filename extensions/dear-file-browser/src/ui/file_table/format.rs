use std::cell::OnceCell;
use std::time::SystemTime;

use jiff::{Timestamp, tz::TimeZone};

pub(super) fn format_size(size: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    let s = size as f64;
    if s >= GB {
        format!("{:.2} GB", s / GB)
    } else if s >= MB {
        format!("{:.2} MB", s / MB)
    } else if s >= KB {
        format!("{:.0} KB", s / KB)
    } else {
        format!("{} B", size)
    }
}

/// Samples the clock and local zone on demand, once per table draw.
#[derive(Default)]
pub(super) struct ModifiedTimeFormat {
    now: OnceCell<Timestamp>,
    zone: OnceCell<TimeZone>,
}

impl ModifiedTimeFormat {
    pub(super) fn label(&self, modified: Option<SystemTime>) -> String {
        let Some(modified) = modified else {
            return String::new();
        };
        let Ok(modified) = Timestamp::try_from(modified) else {
            return "Unknown".into();
        };
        let age = self
            .now
            .get_or_init(Timestamp::now)
            .duration_since(modified);
        let seconds = age.unsigned_abs().as_secs();
        let (amount, unit) = match seconds {
            0..10 => return "just now".into(),
            10..60 => (seconds, 's'),
            60..3_600 => (seconds / 60, 'm'),
            3_600..86_400 => (seconds / 3_600, 'h'),
            86_400..604_800 => (seconds / 86_400, 'd'),
            _ => return self.absolute(modified, "%Y-%m-%d"),
        };
        if age.is_negative() {
            format!("in {amount}{unit}")
        } else {
            format!("{amount}{unit} ago")
        }
    }

    pub(super) fn tooltip(&self, modified: Option<SystemTime>) -> Option<String> {
        let modified = Timestamp::try_from(modified?).ok()?;
        Some(self.absolute(modified, "%Y-%m-%d %H:%M:%S"))
    }

    fn absolute(&self, timestamp: Timestamp, format: &str) -> String {
        let zone = self.zone.get_or_init(TimeZone::system);
        timestamp
            .to_zoned_datetime(zone.clone())
            .strftime(format)
            .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, UNIX_EPOCH};

    #[test]
    fn host_clock_and_zone_agree_on_the_local_date_of_an_older_file() {
        let modified =
            SystemTime::try_from(Timestamp::now() - Duration::from_secs(14 * 86_400)).unwrap();
        let dates = ModifiedTimeFormat::default();
        let label = dates.label(Some(modified));
        let tooltip = dates.tooltip(Some(modified)).unwrap();
        assert_eq!(tooltip.split_once(' ').unwrap().0, label);
    }

    #[test]
    fn relative_times_keep_the_direction_and_switch_to_local_dates_at_a_week() {
        let now = UNIX_EPOCH + Duration::from_secs(7 * 86_400);
        let dates = ModifiedTimeFormat {
            now: Timestamp::try_from(now).unwrap().into(),
            zone: TimeZone::fixed(jiff::tz::offset(-5)).into(),
        };

        for (seconds, past, future) in [
            (9, "just now", "just now"),
            (10, "10s ago", "in 10s"),
            (60, "1m ago", "in 1m"),
            (3_600, "1h ago", "in 1h"),
            (604_799, "6d ago", "in 6d"),
            (604_800, "1969-12-31", "1970-01-14"),
        ] {
            let delta = Duration::from_secs(seconds);
            assert_eq!(dates.label(Some(now - delta)), past);
            assert_eq!(dates.label(Some(now + delta)), future);
        }
        assert_eq!(
            dates
                .tooltip(Some(UNIX_EPOCH + Duration::from_secs(62)))
                .as_deref(),
            Some("1969-12-31 19:01:02")
        );
    }

    #[test]
    fn missing_and_unrepresentable_metadata_have_no_datetime_tooltip() {
        let dates = ModifiedTimeFormat::default();
        assert_eq!(dates.label(None), "");
        assert_eq!(dates.tooltip(None), None);
        // 10000-01-01 is outside Jiff's calendar even on platforms that can store it as file metadata.
        if let Some(too_late) = UNIX_EPOCH.checked_add(Duration::from_secs(253_402_300_800)) {
            assert_eq!(dates.label(Some(too_late)), "Unknown");
            assert_eq!(dates.tooltip(Some(too_late)), None);
        }
    }
}
