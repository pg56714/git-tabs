//! Author pictures, the way Fork shows them: GitHub's for GitHub addresses, Gravatar's for the
//! rest, fetched in the background and kept on disk. With no network, or no picture for an
//! address, `get` says None and the caller draws initials instead.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, SystemTime};

/// How long "there is no picture" is believed before asking again.
const MISS_FOR: Duration = Duration::from_secs(3 * 24 * 3600);
const MAX_PARALLEL: usize = 4;

fn md5_hex(data: &[u8]) -> String {
    const S: [u32; 64] = [
        7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 6, 10, 15, 21, 6, 10, 15,
        21, 6, 10, 15, 21, 6, 10, 15, 21,
    ];
    let k: Vec<u32> = (0..64).map(|i| ((i as f64 + 1.0).sin().abs() * 4294967296.0) as u32).collect();
    let (mut a0, mut b0, mut c0, mut d0) = (0x67452301u32, 0xefcdab89u32, 0x98badcfeu32, 0x10325476u32);
    let mut msg = data.to_vec();
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend(((data.len() as u64) * 8).to_le_bytes());
    for chunk in msg.chunks(64) {
        let m: Vec<u32> = chunk.chunks(4).map(|w| u32::from_le_bytes([w[0], w[1], w[2], w[3]])).collect();
        let (mut a, mut b, mut c, mut d) = (a0, b0, c0, d0);
        for i in 0..64 {
            let (f, g) = match i / 16 {
                0 => ((b & c) | (!b & d), i),
                1 => ((d & b) | (!d & c), (5 * i + 1) % 16),
                2 => (b ^ c ^ d, (3 * i + 5) % 16),
                _ => (c ^ (b | !d), (7 * i) % 16),
            };
            let f = f.wrapping_add(a).wrapping_add(k[i]).wrapping_add(m[g]);
            a = d;
            d = c;
            c = b;
            b = b.wrapping_add(f.rotate_left(S[i]));
        }
        a0 = a0.wrapping_add(a);
        b0 = b0.wrapping_add(b);
        c0 = c0.wrapping_add(c);
        d0 = d0.wrapping_add(d);
    }
    [a0, b0, c0, d0].iter().flat_map(|x| x.to_le_bytes()).map(|b| format!("{b:02x}")).collect()
}

/// Where the picture for a commit email comes from.
pub fn url(email: &str) -> Option<String> {
    let e = email.trim().to_lowercase();
    if !e.contains('@') {
        return None;
    }
    if let Some(user) = e.strip_suffix("@users.noreply.github.com") {
        // "12345+name" carries the account number; an old "name" alone is the login.
        return Some(match user.split_once('+') {
            Some((id, _)) if id.chars().all(|c| c.is_ascii_digit()) && !id.is_empty() => format!("https://avatars.githubusercontent.com/u/{id}?s=64"),
            Some((_, name)) => format!("https://github.com/{name}.png?size=64"),
            None => format!("https://github.com/{user}.png?size=64"),
        });
    }
    if e.ends_with(".local") || e.ends_with(".invalid") || e.ends_with("@localhost") {
        return None;
    }
    Some(format!("https://www.gravatar.com/avatar/{}?s=64&d=404", md5_hex(e.as_bytes())))
}

/// "owner/repo" of a github.com remote URL.
pub fn github_slug(remote: &str) -> Option<String> {
    let r = remote.trim();
    // github.com, or an ssh alias for it such as "github-work-emu".
    let at = r.find("github")?;
    let rest = r[at..].split_once([':', '/'])?.1.trim_start_matches([':', '/']);
    let mut it = rest.trim_end_matches('/').trim_end_matches(".git").split('/');
    match (it.next(), it.next()) {
        (Some(o), Some(n)) if !o.is_empty() && !n.is_empty() => Some(format!("{o}/{n}")),
        _ => None,
    }
}

enum State {
    Loading,
    Have(PathBuf),
    None,
}

/// Fetches `url` into the file; true when a picture arrived.
pub type Fetch = Arc<dyn Fn(&str, &Path, Option<&str>) -> bool + Send + Sync>;

/// (account, token) of every account the GitHub CLI is logged in to on github.com.
pub type Accounts = Arc<dyn Fn() -> Vec<(String, String)> + Send + Sync>;

pub struct Cache {
    dir: PathBuf,
    fetch: Fetch,
    accounts: Accounts,
    known: OnceLock<Vec<(String, String)>>,
    /// Which account could read which repo, so the next lookup starts there.
    served_by: Mutex<HashMap<String, String>>,
    map: Mutex<HashMap<String, State>>,
    busy: AtomicUsize,
    changed: AtomicBool,
    off: bool,
}

impl Cache {
    pub fn new(config_dir: &Path) -> Arc<Self> {
        let no_gh = std::env::var("GIT_TABS_NO_GH").is_ok_and(|v| v == "1");
        let accounts: Accounts = if no_gh { Arc::new(Vec::new) } else { Arc::new(gh_accounts) };
        Self::with_parts(config_dir, Arc::new(curl_fetch), accounts, std::env::var("GIT_TABS_NO_AVATARS").is_ok_and(|v| v == "1"))
    }

    pub fn with_fetch(config_dir: &Path, fetch: Fetch, off: bool) -> Arc<Self> {
        Self::with_parts(config_dir, fetch, Arc::new(Vec::new), off)
    }

    pub fn with_parts(config_dir: &Path, fetch: Fetch, accounts: Accounts, off: bool) -> Arc<Self> {
        Arc::new(Self { dir: config_dir.join("avatars"), fetch, accounts, known: OnceLock::new(), served_by: Mutex::new(HashMap::new()), map: Mutex::new(HashMap::new()), busy: AtomicUsize::new(0), changed: AtomicBool::new(false), off })
    }

    fn file(&self, key: &str) -> PathBuf {
        self.dir.join(format!("{}.png", md5_hex(key.as_bytes())))
    }

    /// The picture file for this email, when there is one. The first call for an address starts
    /// the download and returns None; `take_changed` says when something new has arrived.
    ///
    /// `hint` is (owner/repo, sha) of a commit by this address on GitHub: when Gravatar has no
    /// picture, GitHub's own answer for that commit (the account behind the address) is used.
    pub fn get(self: &Arc<Self>, email: &str, hint: Option<(&str, &str)>) -> Option<PathBuf> {
        let key = email.trim().to_lowercase();
        if self.off || key.is_empty() {
            return None;
        }
        let mut map = self.map.lock().ok()?;
        match map.get(&key) {
            Some(State::Have(p)) => return Some(p.clone()),
            Some(State::None | State::Loading) => return None,
            None => {}
        }
        let f = self.file(&key);
        let miss = f.with_extension("miss2");
        if f.is_file() {
            map.insert(key, State::Have(f.clone()));
            return Some(f);
        }
        let fresh_miss = std::fs::metadata(&miss).and_then(|m| m.modified()).is_ok_and(|t| SystemTime::now().duration_since(t).unwrap_or_default() < MISS_FOR);
        let Some(url) = url(&key).filter(|_| !fresh_miss) else {
            map.insert(key, State::None);
            return None;
        };
        if self.busy.load(Ordering::SeqCst) >= MAX_PARALLEL {
            // Not now; asked again on the next draw.
            return None;
        }
        map.insert(key.clone(), State::Loading);
        self.busy.fetch_add(1, Ordering::SeqCst);
        let me = self.clone();
        let hint = hint.map(|(s, c)| (s.to_string(), format!("https://api.github.com/repos/{s}/commits/{c}")));
        std::thread::spawn(move || {
            let _ = std::fs::create_dir_all(&me.dir);
            let tmp = f.with_extension("part");
            let ok = |p: &Path| std::fs::metadata(p).is_ok_and(|m| m.len() > 100);
            let mut got = (me.fetch)(&url, &tmp, None) && ok(&tmp);
            if !got && let Some((slug, api)) = hint {
                got = me.from_github(&slug, &api, &f, &tmp);
            }
            let got = got && std::fs::rename(&tmp, &f).is_ok();
            let _ = std::fs::remove_file(&tmp);
            if !got {
                let _ = std::fs::write(&miss, b"");
            }
            if let Ok(mut map) = me.map.lock() {
                map.insert(key, if got { State::Have(f) } else { State::None });
            }
            me.busy.fetch_sub(1, Ordering::SeqCst);
            if got {
                me.changed.store(true, Ordering::SeqCst);
            }
        });
        None
    }

    /// GitHub's answer for a commit: the account behind its author, and that account's picture.
    /// Tries the accounts the GitHub CLI knows (the one that read this repo last time first),
    /// since a private repo only answers to an account that can see it, and then no login.
    fn from_github(&self, slug: &str, api: &str, f: &Path, tmp: &Path) -> bool {
        let ok = |p: &Path| std::fs::metadata(p).is_ok_and(|m| m.len() > 100);
        let json = f.with_extension("json");
        let known = self.known.get_or_init(|| (self.accounts)());
        let first = self.served_by.lock().ok().and_then(|m| m.get(slug).cloned());
        let mut order: Vec<&(String, String)> = known.iter().collect();
        order.sort_by_key(|(name, _)| Some(name) != first.as_ref());
        let mut tries: Vec<Option<&(String, String)>> = order.into_iter().map(Some).collect();
        tries.push(None);
        for who in tries {
            if !(self.fetch)(api, &json, who.map(|w| w.1.as_str())) {
                continue;
            }
            let pic = std::fs::read_to_string(&json)
                .ok()
                .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
                .and_then(|v| v["author"]["avatar_url"].as_str().map(|u| format!("{u}{}s=64", if u.contains('?') { "&" } else { "?" })));
            let _ = std::fs::remove_file(&json);
            // The commit was readable; if its author has no account there is nothing more to find.
            if let (Some(w), Ok(mut m)) = (who, self.served_by.lock()) {
                m.insert(slug.to_string(), w.0.clone());
            }
            return pic.is_some_and(|u| (self.fetch)(&u, tmp, None) && ok(tmp));
        }
        let _ = std::fs::remove_file(&json);
        false
    }

    /// Whether a picture has arrived since the last call.
    pub fn take_changed(&self) -> bool {
        self.changed.swap(false, Ordering::SeqCst)
    }
}

fn curl_fetch(url: &str, to: &Path, token: Option<&str>) -> bool {
    let mut c = std::process::Command::new("curl");
    c.args(["-fsSL", "--max-time", "15", "--max-filesize", "2000000", "-H", "User-Agent: git-tabs"]);
    // The token goes in a header file, not on the command line, where other programs could read it.
    let header = token.and_then(|t| {
        static N: AtomicUsize = AtomicUsize::new(0);
        let p = std::env::temp_dir().join(format!("git-tabs-h-{}-{}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
        std::fs::write(&p, format!("Authorization: Bearer {t}\n")).ok().map(|_| p)
    });
    if let Some(h) = &header {
        c.arg("-H").arg(format!("@{}", h.display()));
    }
    c.arg("-o").arg(to).arg(url);
    c.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000);
    }
    let ok = c.status().is_ok_and(|s| s.success());
    if let Some(h) = header {
        let _ = std::fs::remove_file(h);
    }
    ok
}

fn gh(args: &[&str]) -> Option<String> {
    let mut c = std::process::Command::new("gh");
    c.args(args).stdin(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000);
    }
    let o = c.output().ok().filter(|o| o.status.success())?;
    Some(String::from_utf8_lossy(&o.stdout).into_owned())
}

/// The account names in `gh auth status` output ("Logged in to github.com account NAME (keyring)").
fn account_names(status: &str) -> Vec<String> {
    status
        .lines()
        .filter_map(|l| l.split_once("Logged in to github.com account ").map(|x| x.1))
        .filter_map(|r| r.split_whitespace().next().map(String::from))
        .collect()
}

fn gh_accounts() -> Vec<(String, String)> {
    let Some(status) = gh(&["auth", "status", "--hostname", "github.com"]) else { return vec![] };
    account_names(&status)
        .into_iter()
        .filter_map(|n| gh(&["auth", "token", "--hostname", "github.com", "--user", &n]).map(|t| (n, t.trim().to_string())))
        .filter(|(_, t)| !t.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn md5_matches_the_known_vectors() {
        assert_eq!(md5_hex(b""), "d41d8cd98f00b204e9800998ecf8427e");
        assert_eq!(md5_hex(b"abc"), "900150983cd24fb0d6963f7d28e17f72");
        assert_eq!(md5_hex(b"The quick brown fox jumps over the lazy dog"), "9e107d9d372bb6826bd81d3542a419d6");
        // Gravatar's own documented example.
        assert_eq!(md5_hex(b"myemailaddress@example.com"), "0bc83cb571cd1c50ba6f3e8a78ef1346");
    }

    #[test]
    fn account_names_come_out_of_gh_status() {
        let s = "github.com\n  ✓ Logged in to github.com account Sam-Hong_adataemu (keyring)\n  - Active account: true\n  ✓ Logged in to github.com account pg56714 (keyring)\n";
        assert_eq!(account_names(s), ["Sam-Hong_adataemu", "pg56714"]);
    }

    #[test]
    fn github_slugs() {
        assert_eq!(github_slug("git@github.com:pg56714/git-tabs.git").unwrap(), "pg56714/git-tabs");
        assert_eq!(github_slug("https://alice@github.com/Org/repo").unwrap(), "Org/repo");
        assert_eq!(github_slug("git@github-work-emu:TRUSTA-SW-FAE/x.git").unwrap(), "TRUSTA-SW-FAE/x");
        assert!(github_slug("https://gitlab.com/a/b").is_none());
    }

    #[test]
    fn where_each_address_gets_its_picture() {
        assert_eq!(url("293473+sandersn@users.noreply.github.com").unwrap(), "https://avatars.githubusercontent.com/u/293473?s=64");
        assert_eq!(url("sandersn@users.noreply.github.com").unwrap(), "https://github.com/sandersn.png?size=64");
        assert_eq!(url("  MyEmailAddress@example.com ").unwrap(), "https://www.gravatar.com/avatar/0bc83cb571cd1c50ba6f3e8a78ef1346?s=64&d=404");
        assert!(url("nobody").is_none());
        assert!(url("me@host.local").is_none());
    }

    fn wait(c: &Arc<Cache>, email: &str) -> Option<PathBuf> {
        for _ in 0..200 {
            if let Some(p) = c.get(email, None) {
                return Some(p);
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        None
    }

    #[test]
    fn a_picture_is_fetched_once_kept_and_missing_ones_are_not_asked_again() {
        let d = tempfile::tempdir().unwrap();
        let calls = Arc::new(AtomicUsize::new(0));
        let n = calls.clone();
        let fetch: Fetch = Arc::new(move |url, to, _| {
            n.fetch_add(1, Ordering::SeqCst);
            if url.contains("/u/7") {
                std::fs::write(to, vec![1u8; 500]).is_ok()
            } else {
                false
            }
        });
        let c = Cache::with_fetch(d.path(), fetch.clone(), false);
        assert!(c.get("7+x@users.noreply.github.com", None).is_none(), "first call only starts the download");
        let p = wait(&c, "7+x@users.noreply.github.com").expect("arrives");
        assert!(p.is_file());
        assert!(c.take_changed());
        assert!(!c.take_changed());
        // A new cache (the app restarted) reads the file, no download.
        let c2 = Cache::with_fetch(d.path(), fetch.clone(), false);
        assert!(c2.get("7+x@users.noreply.github.com", None).is_some());
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        // No picture: asked once, remembered across restarts.
        assert!(wait(&c2, "someone@example.com").is_none());
        let after = calls.load(Ordering::SeqCst);
        assert_eq!(after, 2);
        let c3 = Cache::with_fetch(d.path(), fetch, false);
        assert!(c3.get("someone@example.com", None).is_none());
        assert_eq!(calls.load(Ordering::SeqCst), after);
        // Gravatar has nothing, GitHub knows the account behind the commit.
        let api: Fetch = Arc::new(|url, to, _| {
            if url.contains("gravatar") {
                false
            } else if url.contains("api.github.com/repos/o/r/commits/abc") {
                std::fs::write(to, r#"{"author":{"login":"x","avatar_url":"https://avatars.githubusercontent.com/u/9?v=4"}}"#).is_ok()
            } else if url.starts_with("https://avatars.githubusercontent.com/u/9?v=4&s=64") {
                std::fs::write(to, vec![2u8; 500]).is_ok()
            } else {
                false
            }
        });
        let c4 = Cache::with_fetch(tempfile::tempdir().unwrap().path(), api, false);
        assert!(c4.get("me@gmail.com", Some(("o/r", "abc"))).is_none());
        for _ in 0..200 {
            if c4.get("me@gmail.com", Some(("o/r", "abc"))).is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(c4.get("me@gmail.com", Some(("o/r", "abc"))).is_some());
        // A private repo: only one of the CLI's accounts can read it, and it is asked first next time.
        let seen = Arc::new(Mutex::new(Vec::<String>::new()));
        let log = seen.clone();
        let private: Fetch = Arc::new(move |url, to, token| {
            if url.contains("gravatar") {
                false
            } else if url.contains("api.github.com/repos/o/priv/commits/") {
                log.lock().unwrap().push(token.unwrap_or("none").to_string());
                token == Some("tok-b") && std::fs::write(to, r#"{"author":{"avatar_url":"https://avatars.githubusercontent.com/u/5"}}"#).is_ok()
            } else if url.starts_with("https://avatars.githubusercontent.com/u/5?s=64") {
                std::fs::write(to, vec![3u8; 500]).is_ok()
            } else {
                false
            }
        });
        let accounts: Accounts = Arc::new(|| vec![("a".into(), "tok-a".into()), ("b".into(), "tok-b".into())]);
        let c5 = Cache::with_parts(tempfile::tempdir().unwrap().path(), private, accounts, false);
        for email in ["w@corp.com", "v@corp.com"] {
            let sha = if email.starts_with('w') { "1" } else { "2" };
            assert!(c5.get(email, Some(("o/priv", sha))).is_none());
            for _ in 0..300 {
                if c5.get(email, Some(("o/priv", sha))).is_some() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            assert!(c5.get(email, Some(("o/priv", sha))).is_some());
        }
        assert_eq!(*seen.lock().unwrap(), ["tok-a", "tok-b", "tok-b"], "the account that worked goes first next time");
        // Switched off: never fetches.
        let off = Cache::with_fetch(tempfile::tempdir().unwrap().path(), Arc::new(|_, _, _| panic!("no network")), true);
        assert!(off.get("7+x@users.noreply.github.com", None).is_none());
    }
}
