//! Small text helpers (ported from src/lib/format.ts).

use std::time::{SystemTime, UNIX_EPOCH};

pub fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// "3 分鐘前" style, falling back to a date after a week.
pub fn ago(unix: i64) -> String {
    ago_at(unix, now())
}

pub fn ago_at(unix: i64, now: i64) -> String {
    if unix == 0 {
        return String::new();
    }
    let s = (now - unix).max(0);
    if s < 60 {
        crate::i18n::t("剛剛").into()
    } else if s < 3600 {
        crate::i18n::tr("{} 分鐘前", &[], &[(s / 60).to_string()])
    } else if s < 86400 {
        crate::i18n::tr("{} 小時前", &[], &[(s / 3600).to_string()])
    } else if s < 86400 * 7 {
        crate::i18n::tr("{} 天前", &[], &[(s / 86400).to_string()])
    } else {
        date_time(unix)
    }
}

/// Local time as "YYYY/MM/DD HH:MM".
pub fn date_time(unix: i64) -> String {
    let (y, mo, d, h, mi) = civil(unix + local_offset_secs());
    format!("{y}/{mo:02}/{d:02} {h:02}:{mi:02}")
}

/// Seconds since the epoch to a calendar date and time (Howard Hinnant's algorithm).
fn civil(t: i64) -> (i64, u32, u32, u32, u32) {
    let days = t.div_euclid(86400);
    let rem = t.rem_euclid(86400);
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d, (rem / 3600) as u32, ((rem % 3600) / 60) as u32)
}

/// Offset of the local time zone from UTC, in seconds.
fn local_offset_secs() -> i64 {
    #[cfg(windows)]
    {
        unsafe extern "C" {
            fn _get_timezone(seconds: *mut i32) -> i32;
            fn _get_dstbias(seconds: *mut i32) -> i32;
        }
        let (mut tz, mut dst) = (0i32, 0i32);
        // SAFETY: both functions only write one i32 through the pointer we pass.
        unsafe {
            _get_timezone(&mut tz);
            _get_dstbias(&mut dst);
        }
        // _get_timezone is seconds west of UTC. A summer hour of error on a minute-level
        // timestamp is hidden by the relative "ago" form, so daylight saving is not tracked.
        -(tz as i64) - dst as i64
    }
    #[cfg(not(windows))]
    {
        0
    }
}

pub fn short(sha: &str) -> String {
    sha.chars().take(7).collect()
}

pub fn base_name(p: &str) -> String {
    p.trim_end_matches(['/', '\\']).rsplit(['/', '\\']).next().unwrap_or(p).to_string()
}

/// Every word of the query appears somewhere in the text, in any order.
pub fn fuzzy(query: &str, text: &str) -> bool {
    let t = text.to_lowercase();
    query.to_lowercase().split_whitespace().all(|w| t.contains(w))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_times() {
        assert_eq!(ago_at(1000, 1010), "剛剛");
        assert_eq!(ago_at(1000, 1000 + 120), "2 分鐘前");
        assert_eq!(ago_at(1000, 1000 + 7200), "2 小時前");
        assert_eq!(ago_at(1000, 1000 + 86400 * 3), "3 天前");
        assert_eq!(ago_at(0, 5), "");
    }

    #[test]
    fn civil_dates() {
        assert_eq!(civil(0), (1970, 1, 1, 0, 0));
        assert_eq!(civil(1_700_000_000), (2023, 11, 14, 22, 13));
        assert_eq!(civil(951_782_400 + 3600 * 5 + 60 * 7), (2000, 2, 29, 5, 7));
    }

    #[test]
    fn names_and_search() {
        assert_eq!(base_name("C:\\a\\b\\repo\\"), "repo");
        assert_eq!(base_name("/x/y/z"), "z");
        assert_eq!(short("abcdef0123"), "abcdef0");
        assert!(fuzzy("push my", "Push my-repo"));
        assert!(!fuzzy("pull my", "Push my-repo"));
    }
}
