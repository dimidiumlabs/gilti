// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

use maud::{Markup, Render, html};

use crate::styles::classes::relative_time;

pub struct RelativeTime {
    pub timestamp: i64,
}

impl RelativeTime {
    /// Formats a timestamp using the same relative-age policy as the component.
    pub fn label(timestamp: i64) -> String {
        label_at(timestamp, now())
    }
}

impl Render for RelativeTime {
    fn render(&self) -> Markup {
        let (amount, unit) = parts_at(self.timestamp, now());
        let datetime = rfc3339(self.timestamp);
        html! {
            time class=(relative_time::ROOT)
                    datetime=(&datetime)
                    title=(&datetime)
                    data-relative-time=""
                    data-timestamp=(self.timestamp)
                    data-unit=(unit) {
                (amount) " " (suffix(unit)) " ago"
            }
        }
    }
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs().min(i64::MAX as u64) as i64)
}

fn label_at(timestamp: i64, now: i64) -> String {
    let (amount, unit) = parts_at(timestamp, now);
    format!("{amount} {} ago", suffix(unit))
}

fn parts_at(timestamp: i64, now: i64) -> (i64, &'static str) {
    let seconds = now.saturating_sub(timestamp).max(0);
    if seconds < 2 * 60 * 60 {
        (seconds / 60, "minutes")
    } else if seconds < 2 * 24 * 60 * 60 {
        (seconds / 3_600, "hours")
    } else if seconds < 14 * 24 * 60 * 60 {
        (seconds / 86_400, "days")
    } else if seconds < 60 * 24 * 60 * 60 {
        (seconds / 604_800, "weeks")
    } else if seconds < 730 * 24 * 60 * 60 {
        (seconds / 2_592_000, "months")
    } else {
        (seconds / 31_536_000, "years")
    }
}

fn suffix(unit: &str) -> &'static str {
    match unit {
        "minutes" => "min.",
        "hours" => "hours",
        "days" => "days",
        "weeks" => "weeks",
        "months" => "months",
        "years" => "years",
        _ => unreachable!("relative time units are fixed"),
    }
}

fn rfc3339(timestamp: i64) -> String {
    const MIN: i64 = -62_135_596_800; // 0001-01-01T00:00:00Z
    const MAX: i64 = 253_402_300_799; // 9999-12-31T23:59:59Z
    let timestamp = timestamp.clamp(MIN, MAX);
    let days = timestamp.div_euclid(86_400);
    let seconds = timestamp.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        seconds / 3_600,
        seconds % 3_600 / 60,
        seconds % 60
    )
}

// Converts days since 1970-01-01 to a proleptic Gregorian date.
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let month_part = (5 * doy + 2) / 153;
    let day = doy - (153 * month_part + 2) / 5 + 1;
    let month = month_part + if month_part < 10 { 3 } else { -9 };
    (year + i64::from(month <= 2), month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_and_clamps_rfc3339_datetimes() {
        assert_eq!(rfc3339(0), "1970-01-01T00:00:00Z");
        assert_eq!(rfc3339(-1), "1969-12-31T23:59:59Z");
        assert_eq!(rfc3339(i64::MIN), "0001-01-01T00:00:00Z");
        assert_eq!(rfc3339(i64::MAX), "9999-12-31T23:59:59Z");
    }

    #[test]
    fn future_timestamps_are_zero_minutes_old() {
        assert_eq!(label_at(101, 100), "0 min. ago");
    }

    #[test]
    fn formats_each_relative_time_boundary() {
        let now = 10_000_000;
        for (age, expected) in [
            (0, "0 min. ago"),
            (7_199, "119 min. ago"),
            (7_200, "2 hours ago"),
            (172_799, "47 hours ago"),
            (172_800, "2 days ago"),
            (1_209_599, "13 days ago"),
            (1_209_600, "2 weeks ago"),
            (5_183_999, "8 weeks ago"),
            (5_184_000, "2 months ago"),
            (63_071_999, "24 months ago"),
            (63_072_000, "2 years ago"),
        ] {
            assert_eq!(label_at(now - age, now), expected);
        }
    }

    #[test]
    fn renders_semantic_relative_time_attributes() {
        let rendered = RelativeTime { timestamp: 0 }.render().into_string();
        assert!(rendered.contains("<time"));
        assert!(rendered.contains("datetime=\"1970-01-01T00:00:00Z\""));
        assert!(rendered.contains("title=\"1970-01-01T00:00:00Z\""));
        assert!(rendered.contains("data-relative-time=\"\""));
        assert!(rendered.contains("data-timestamp=\"0\""));
        assert!(rendered.contains("data-unit=\"years\""));
    }
}
