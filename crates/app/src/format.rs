//! Small text helpers.

use std::time::{SystemTime, UNIX_EPOCH};

pub fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
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
    let (y, mo, d, h, mi) = civil(unix + local_offset_secs(unix));
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

/// Days since 1970-01-01 of a calendar date (the inverse of `civil`).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// Offset of the local time zone from UTC at `unix`, in seconds, daylight saving included.
///
/// Asked of the C runtime's `localtime`, which reads the system zone itself. Reading its zone
/// variables directly gets the runtime's built-in US Pacific default, because a Rust program
/// never initialises them, and every date then came out hours off.
fn local_offset_secs(unix: i64) -> i64 {
    // The nine ints every C `struct tm` starts with; the fields after them differ per platform
    // and are not read, but the room is reserved because the callee writes them.
    #[repr(C)]
    #[derive(Default)]
    struct Tm {
        sec: i32,
        min: i32,
        hour: i32,
        mday: i32,
        mon: i32,
        year: i32,
        wday: i32,
        yday: i32,
        isdst: i32,
        _rest: [i64; 4],
    }
    unsafe extern "C" {
        #[cfg(windows)]
        fn _localtime64_s(out: *mut Tm, t: *const i64) -> i32;
        #[cfg(not(windows))]
        fn localtime_r(t: *const i64, out: *mut Tm) -> *mut Tm;
    }
    let mut tm = Tm::default();
    // SAFETY: `tm` is at least as large as the platform's `struct tm`, and both calls only
    // write into it through the pointer we pass.
    #[cfg(windows)]
    let ok = unsafe { _localtime64_s(&mut tm, &unix) } == 0;
    #[cfg(not(windows))]
    let ok = !unsafe { localtime_r(&unix, &mut tm) }.is_null();
    if !ok {
        return 0;
    }
    let local = days_from_civil(tm.year as i64 + 1900, tm.mon as i64 + 1, tm.mday as i64) * 86400
        + tm.hour as i64 * 3600
        + tm.min as i64 * 60
        + tm.sec as i64;
    local - unix
}

pub fn short(sha: &str) -> String {
    sha.chars().take(7).collect()
}

pub fn base_name(p: &str) -> String {
    p.trim_end_matches(['/', '\\'])
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(p)
        .to_string()
}

/// Whether two paths name the same folder, however git and the system each spell it
/// (`C:/a/b` and `c:\a\b\` on Windows).
pub fn same_path(a: &str, b: &str) -> bool {
    let n = |p: &str| {
        let p = p.replace('\\', "/").trim_end_matches('/').to_string();
        if cfg!(windows) { p.to_lowercase() } else { p }
    };
    n(a) == n(b)
}

/// Splits `fix(api): keep the peer` into its Conventional Commits prefix (`fix(api):`) and the
/// rest, so the list can draw the prefix bold as Fork does. Without such a prefix all is the rest.
pub fn split_subject(s: &str) -> (&str, &str) {
    let Some(i) = s.find(": ") else {
        return ("", s);
    };
    let head = s[..i].strip_suffix('!').unwrap_or(&s[..i]);
    let (kind, scope) = head.split_at(head.find('(').unwrap_or(head.len()));
    let word = !kind.is_empty() && kind.chars().all(|c| c.is_ascii_alphabetic());
    let scoped =
        scope.is_empty() || (scope.ends_with(')') && scope.matches(['(', ')']).count() == 2);
    if word && scoped {
        (&s[..i + 1], s[i + 2..].trim_start())
    } else {
        ("", s)
    }
}

/// Every word of the query appears somewhere in the text, in any order.
pub fn fuzzy(query: &str, text: &str) -> bool {
    let t = text.to_lowercase();
    query
        .to_lowercase()
        .split_whitespace()
        .all(|w| t.contains(w))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_compare_across_slashes() {
        assert!(same_path("/home/a/repo/", "/home/a/repo"));
        assert!(same_path("C:/a/b", "C:\\a\\b"));
        assert!(!same_path("/home/a/repo", "/home/a/repo2"));
    }

    #[test]
    fn conventional_prefix_is_split_off() {
        assert_eq!(
            split_subject("fix(api): keep the peer"),
            ("fix(api):", "keep the peer")
        );
        assert_eq!(
            split_subject("feat!: drop the old flag"),
            ("feat!:", "drop the old flag")
        );
        assert_eq!(split_subject("wip: before i18n"), ("wip:", "before i18n"));
        assert_eq!(
            split_subject("Merge pull request #19 from a/b: x"),
            ("", "Merge pull request #19 from a/b: x")
        );
        assert_eq!(split_subject("no prefix here"), ("", "no prefix here"));
        assert_eq!(split_subject("修正: 中文標題"), ("", "修正: 中文標題"));
    }

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

    #[test]
    fn calendar_round_trips() {
        for t in [0i64, 1794972822, 951782400, -86400] {
            let (y, m, d, _, _) = civil(t);
            assert_eq!(days_from_civil(y, m as i64, d as i64), t.div_euclid(86400));
        }
    }

    #[test]
    fn local_offset_is_a_real_zone() {
        // A whole number of minutes within the range of zones in use.
        let o = local_offset_secs(1794972822);
        assert!(o % 60 == 0 && (-12 * 3600..=14 * 3600).contains(&o), "{o}");
    }
}
