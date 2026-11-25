//! UTC start times that are valid file names on every OS and sort by start time
//! as text. The suffix tells apart Sessions started in the same millisecond.

use std::time::{SystemTime, UNIX_EPOCH};

pub fn candidates(time: SystemTime) -> impl Iterator<Item = String> {
    let base = base(time);
    (1..).map(move |n: u32| {
        if n == 1 {
            base.clone()
        } else {
            format!("{base}-{n:03}")
        }
    })
}

/// IDs come from the UI and reach file names, so they're checked first.
pub fn is_valid(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn base(time: SystemTime) -> String {
    let ms = time
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let (secs, ms) = (ms / 1000, ms % 1000);
    let (days, secs) = (secs / 86_400, secs % 86_400);
    let (year, month, day) = civil_from_days(days as i64);
    let (hour, minute, second) = (secs / 3600, secs / 60 % 60, secs % 60);
    format!("{year:04}-{month:02}-{day:02}_{hour:02}-{minute:02}-{second:02}-{ms:03}Z")
}

/// Howard Hinnant's `civil_from_days`.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}
