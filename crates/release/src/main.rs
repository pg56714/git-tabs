//! Works out the next version from Conventional Commits since the last tag, prepends the
//! release notes to CHANGELOG.md and bumps the version in Cargo.toml and Cargo.lock.
//!
//!   cargo run -p release            write the files (CI)
//!   cargo run -p release -- --dry-run   only print what would happen
//!
//! In GitHub Actions it also sets the outputs `skip`, `version` and `tag`, and writes
//! release-notes.md. Run it from the repository root.

use std::io::Write;
use std::process::Command;

#[derive(Debug, Clone, PartialEq)]
struct Change {
    sha: String,
    kind: String,
    scope: String,
    subject: String,
    breaking: bool,
}

/// `type(scope)!: subject`, or None when the line is not a Conventional Commit.
fn parse_header(h: &str) -> Option<(String, String, bool, String)> {
    let word = h.find(|c: char| !(c.is_ascii_alphanumeric() || c == '_')).unwrap_or(h.len());
    if word == 0 {
        return None;
    }
    let (kind, mut rest) = (&h[..word], &h[word..]);
    let mut scope = "";
    if let Some(r) = rest.strip_prefix('(') {
        let close = r.find(')')?;
        scope = &r[..close];
        rest = &r[close + 1..];
    }
    let (breaking, rest) = match rest.strip_prefix('!') {
        Some(r) => (true, r),
        None => (false, rest),
    };
    let subject = rest.strip_prefix(':')?.trim_start();
    if subject.is_empty() {
        return None;
    }
    Some((kind.to_ascii_lowercase(), scope.to_string(), breaking, subject.to_string()))
}

/// Parses `git log --format=%H%x1f%s%x1f%b%x1e`, without the release commits themselves.
fn parse_commits(log: &str) -> Vec<Change> {
    log.split('\x1e')
        .map(str::trim)
        .filter(|r| !r.is_empty())
        .filter_map(|r| {
            let mut f = r.split('\x1f');
            let (sha, subject, body) = (f.next()?, f.next()?, f.next().unwrap_or(""));
            let (kind, scope, bang, subject) = parse_header(subject)?;
            let breaking = bang || body.contains("BREAKING CHANGE") || body.contains("BREAKING-CHANGE");
            Some(Change { sha: sha.to_string(), kind, scope, subject, breaking })
        })
        .filter(|c| !(c.kind == "chore" && c.scope == "release"))
        .collect()
}

/// Next version, or None when nothing worth releasing changed. Before 1.0 a breaking change
/// bumps the minor.
fn next_version(current: &str, changes: &[Change]) -> Option<String> {
    let mut n = current.split('.').map(|p| p.parse::<u64>().unwrap_or(0));
    let (major, minor, patch) = (n.next().unwrap_or(0), n.next().unwrap_or(0), n.next().unwrap_or(0));
    if changes.iter().any(|c| c.breaking) {
        return Some(if major >= 1 { format!("{}.0.0", major + 1) } else { format!("{major}.{}.0", minor + 1) });
    }
    if changes.iter().any(|c| c.kind == "feat") {
        return Some(format!("{major}.{}.0", minor + 1));
    }
    if changes.iter().any(|c| c.kind == "fix" || c.kind == "perf") {
        return Some(format!("{major}.{minor}.{}", patch + 1));
    }
    None
}

type Section = (&'static str, fn(&Change) -> bool);

const SECTIONS: [Section; 5] = [
    ("Breaking changes", |c| c.breaking),
    ("Features", |c| c.kind == "feat" && !c.breaking),
    ("Fixes", |c| c.kind == "fix" && !c.breaking),
    ("Performance", |c| c.kind == "perf" && !c.breaking),
    ("Other", |c| !matches!(c.kind.as_str(), "feat" | "fix" | "perf") && !c.breaking),
];

fn notes(version: &str, date: &str, changes: &[Change], repo_url: &str) -> String {
    let mut out = vec![format!("## v{version} ({date})"), String::new()];
    for (title, pick) in SECTIONS {
        let items: Vec<&Change> = changes.iter().filter(|c| pick(c)).collect();
        if items.is_empty() {
            continue;
        }
        out.push(format!("### {title}"));
        out.push(String::new());
        for c in items {
            let scope = if c.scope.is_empty() { String::new() } else { format!("**{}:** ", c.scope) };
            out.push(format!("- {scope}{} ([{}]({repo_url}/commit/{}))", c.subject, &c.sha[..c.sha.len().min(7)], c.sha));
        }
        out.push(String::new());
    }
    out.join("\n")
}

/// Today's date (UTC) as YYYY-MM-DD, from the system clock.
fn today() -> String {
    let days = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() / 86_400).unwrap_or(0) as i64;
    civil(days)
}

/// Days since 1970-01-01 as a calendar date (Howard Hinnant's algorithm).
fn civil(days: i64) -> String {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

/// The version under `[workspace.package]`, the one place it lives.
fn workspace_version(cargo_toml: &str) -> Option<String> {
    let after = &cargo_toml[cargo_toml.find("[workspace.package]")?..];
    let line = after.lines().find(|l| l.trim_start().starts_with("version"))?;
    Some(line.split('"').nth(1)?.to_string())
}

/// Cargo.toml with the `[workspace.package]` version replaced.
fn set_workspace_version(cargo_toml: &str, version: &str) -> String {
    let Some(start) = cargo_toml.find("[workspace.package]") else { return cargo_toml.to_string() };
    let mut out = String::with_capacity(cargo_toml.len());
    out.push_str(&cargo_toml[..start]);
    let mut done = false;
    for line in cargo_toml[start..].split_inclusive('\n') {
        if !done && line.trim_start().starts_with("version") {
            let eol = &line[line.trim_end_matches(['\r', '\n']).len()..];
            out.push_str(&format!("version = \"{version}\"{eol}"));
            done = true;
        } else {
            out.push_str(line);
        }
    }
    out
}

/// Cargo.lock with the version of the workspace crates replaced.
fn set_lock_versions(lock: &str, version: &str) -> String {
    let mut out = String::with_capacity(lock.len());
    let mut ours = false;
    for line in lock.split_inclusive('\n') {
        let t = line.trim_end_matches(['\r', '\n']);
        if ours && t.starts_with("version = ") {
            let eol = &line[t.len()..];
            out.push_str(&format!("version = \"{version}\"{eol}"));
            ours = false;
            continue;
        }
        ours = t == "name = \"gitcore\"" || t == "name = \"git-tabs\"";
        out.push_str(line);
    }
    out
}

fn git(args: &[&str]) -> Result<String, String> {
    let o = Command::new("git").args(args).output().map_err(|e| e.to_string())?;
    if o.status.success() { Ok(String::from_utf8_lossy(&o.stdout).trim().to_string()) } else { Err(String::from_utf8_lossy(&o.stderr).trim().to_string()) }
}

/// Appends `key=value` to the file GitHub Actions reads step outputs from.
fn output(key: &str, value: &str) {
    if let Some(path) = std::env::var_os("GITHUB_OUTPUT")
        && let Ok(mut f) = std::fs::OpenOptions::new().append(true).create(true).open(path)
    {
        let _ = writeln!(f, "{key}={value}");
    }
}

fn main() -> Result<(), String> {
    let dry = std::env::args().any(|a| a == "--dry-run");
    // No tag yet means this is the first release.
    let last_tag = git(&["describe", "--tags", "--abbrev=0", "--match", "v*"]).unwrap_or_default();
    let range = format!("{last_tag}..HEAD");
    let mut log_args = vec!["log"];
    if !last_tag.is_empty() {
        log_args.push(&range);
    }
    log_args.push("--format=%H%x1f%s%x1f%b%x1e");
    let changes = parse_commits(&git(&log_args)?);
    let cargo = std::fs::read_to_string("Cargo.toml").map_err(|e| format!("Cargo.toml: {e}"))?;
    let current = workspace_version(&cargo).unwrap_or_else(|| "0.1.0".into());
    // The first release ships the version already in Cargo.toml.
    let version = if last_tag.is_empty() { Some(current) } else { next_version(last_tag.trim_start_matches('v'), &changes) };

    let Some(version) = version else {
        println!("Nothing to release since {last_tag}: no feat, fix, perf or breaking commits.");
        output("skip", "true");
        return Ok(());
    };
    let repo_url = match (std::env::var("GITHUB_SERVER_URL"), std::env::var("GITHUB_REPOSITORY")) {
        (Ok(server), Ok(repo)) => format!("{server}/{repo}"),
        _ => git(&["remote", "get-url", "origin"])?,
    };
    let repo_url = repo_url.trim_end_matches(".git").replacen("git@github.com:", "https://github.com/", 1);
    let text = notes(&version, &today(), &changes, &repo_url);
    println!("{} -> v{version}\n\n{text}", if last_tag.is_empty() { "(no tag yet)" } else { &last_tag });
    if dry {
        return Ok(());
    }

    let head = "# Changelog\n\n";
    let old = std::fs::read_to_string("CHANGELOG.md").unwrap_or_default();
    let old = old.replace("\r\n", "\n").replacen(head, "", 1);
    std::fs::write("CHANGELOG.md", format!("{head}{text}\n{old}")).map_err(|e| e.to_string())?;
    std::fs::write("release-notes.md", &text).map_err(|e| e.to_string())?;
    std::fs::write("Cargo.toml", set_workspace_version(&cargo, &version)).map_err(|e| e.to_string())?;
    // The workspace crates' entries in Cargo.lock carry the same version.
    let lock = std::fs::read_to_string("Cargo.lock").map_err(|e| format!("Cargo.lock: {e}"))?;
    std::fs::write("Cargo.lock", set_lock_versions(&lock, &version)).map_err(|e| e.to_string())?;

    output("skip", "false");
    output("version", &version);
    output("tag", &format!("v{version}"));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn log(rows: &[(&str, &str, &str)]) -> String {
        rows.iter().map(|(sha, s, b)| format!("{sha}\x1f{s}\x1f{b}\x1e")).collect::<Vec<_>>().join("\n")
    }

    fn change(kind: &str, breaking: bool) -> Change {
        Change { sha: "x".into(), kind: kind.into(), scope: String::new(), subject: "s".into(), breaking }
    }

    #[test]
    fn reads_conventional_commits_and_skips_release_commits() {
        let c = parse_commits(&log(&[
            ("a1", "feat(ui): tabs", ""),
            ("b2", "fix: crash", ""),
            ("c3", "chore(release): v0.1.0", ""),
            ("d4", "not conventional", ""),
            ("e5", "refactor!: new config", ""),
        ]));
        let got: Vec<(&str, &str, bool)> = c.iter().map(|x| (x.kind.as_str(), x.scope.as_str(), x.breaking)).collect();
        assert_eq!(got, vec![("feat", "ui", false), ("fix", "", false), ("refactor", "", true)]);
        assert!(parse_commits(&log(&[("f6", "fix: x", "BREAKING CHANGE: y")]))[0].breaking);
        assert!(parse_header("feat(: x").is_none() && parse_header(": x").is_none() && parse_header("fix:").is_none());
    }

    #[test]
    fn bumps_by_the_biggest_change() {
        assert_eq!(next_version("0.1.0", &[change("fix", false)]).as_deref(), Some("0.1.1"));
        assert_eq!(next_version("0.1.3", &[change("fix", false), change("feat", false)]).as_deref(), Some("0.2.0"));
        assert_eq!(next_version("0.4.2", &[change("fix", true)]).as_deref(), Some("0.5.0"));
        assert_eq!(next_version("1.4.2", &[change("fix", true)]).as_deref(), Some("2.0.0"));
        assert_eq!(next_version("0.1.0", &[change("docs", false), change("chore", false)]), None);
    }

    #[test]
    fn groups_notes_by_kind() {
        let c = parse_commits(&log(&[("abcdef123", "feat(ui): tabs", ""), ("bcdef1234", "docs: readme", "")]));
        let text = notes("0.2.0", "2026-11-09", &c, "https://github.com/o/r");
        assert!(text.contains("## v0.2.0 (2026-11-09)"));
        assert!(text.contains("### Features\n\n- **ui:** tabs ([abcdef1](https://github.com/o/r/commit/abcdef123))"));
        assert!(text.contains("### Other"));
    }

    #[test]
    fn dates_come_out_right() {
        assert_eq!(civil(0), "1970-01-01");
        assert_eq!(civil(20_774), "2026-11-17");
        assert_eq!(civil(11_016), "2000-02-29");
    }

    #[test]
    fn versions_are_read_and_replaced_in_place() {
        for eol in ["\n", "\r\n"] {
            let toml = format!("[workspace]{eol}members = []{eol}{eol}[workspace.package]{eol}version = \"0.9.0\"{eol}edition = \"2024\"{eol}");
            assert_eq!(workspace_version(&toml).as_deref(), Some("0.9.0"));
            let new = set_workspace_version(&toml, "0.10.0");
            assert_eq!(new, toml.replace("0.9.0", "0.10.0"));
            let lock = format!("[[package]]{eol}name = \"gitcore\"{eol}version = \"0.9.0\"{eol}{eol}[[package]]{eol}name = \"serde\"{eol}version = \"1.0.1\"{eol}{eol}[[package]]{eol}name = \"git-tabs\"{eol}version = \"0.9.0\"{eol}");
            let new = set_lock_versions(&lock, "0.10.0");
            assert_eq!(new, lock.replace("0.9.0", "0.10.0"));
            assert!(new.contains("version = \"1.0.1\""), "other crates keep theirs");
        }
    }
}
