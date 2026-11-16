//! Author pictures, the way Fork shows them: GitHub's for GitHub addresses, Gravatar's for the
//! rest, fetched in the background and kept on disk. With no network, or no picture for an
//! address, `get` says None and the caller draws initials instead.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
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
    let rest = r.split_once("github.com").map(|x| x.1)?.trim_start_matches([':', '/']);
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
pub type Fetch = Arc<dyn Fn(&str, &Path) -> bool + Send + Sync>;

pub struct Cache {
    dir: PathBuf,
    fetch: Fetch,
    map: Mutex<HashMap<String, State>>,
    busy: AtomicUsize,
    changed: AtomicBool,
    off: bool,
}

impl Cache {
    pub fn new(config_dir: &Path) -> Arc<Self> {
        Self::with_fetch(config_dir, Arc::new(curl_fetch), std::env::var("GIT_TABS_NO_AVATARS").is_ok_and(|v| v == "1"))
    }

    pub fn with_fetch(config_dir: &Path, fetch: Fetch, off: bool) -> Arc<Self> {
        Arc::new(Self { dir: config_dir.join("avatars"), fetch, map: Mutex::new(HashMap::new()), busy: AtomicUsize::new(0), changed: AtomicBool::new(false), off })
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
        let miss = f.with_extension("none");
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
        let hint = hint.map(|(s, c)| format!("https://api.github.com/repos/{s}/commits/{c}"));
        std::thread::spawn(move || {
            let _ = std::fs::create_dir_all(&me.dir);
            let tmp = f.with_extension("part");
            let ok = |p: &Path| std::fs::metadata(p).is_ok_and(|m| m.len() > 100);
            let mut got = (me.fetch)(&url, &tmp) && ok(&tmp);
            if !got && let Some(api) = hint {
                let json = f.with_extension("json");
                if (me.fetch)(&api, &json) {
                    let pic = std::fs::read_to_string(&json).ok().and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok()).and_then(|v| v["author"]["avatar_url"].as_str().map(|u| format!("{u}{}s=64", if u.contains('?') { "&" } else { "?" })));
                    got = pic.is_some_and(|u| (me.fetch)(&u, &tmp) && ok(&tmp));
                }
                let _ = std::fs::remove_file(&json);
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

    /// Whether a picture has arrived since the last call.
    pub fn take_changed(&self) -> bool {
        self.changed.swap(false, Ordering::SeqCst)
    }
}

fn curl_fetch(url: &str, to: &Path) -> bool {
    let mut c = std::process::Command::new("curl");
    c.args(["-fsSL", "--max-time", "15", "--max-filesize", "2000000", "-H", "User-Agent: git-tabs", "-o"]).arg(to).arg(url);
    c.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000);
    }
    c.status().is_ok_and(|s| s.success())
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
    fn github_slugs() {
        assert_eq!(github_slug("git@github.com:pg56714/git-tabs.git").unwrap(), "pg56714/git-tabs");
        assert_eq!(github_slug("https://alice@github.com/Org/repo").unwrap(), "Org/repo");
        assert_eq!(github_slug("git@github-work-emu:TRUSTA/x.git"), None);
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
        let fetch: Fetch = Arc::new(move |url, to| {
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
        let api: Fetch = Arc::new(|url, to| {
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
        // Switched off: never fetches.
        let off = Cache::with_fetch(tempfile::tempdir().unwrap().path(), Arc::new(|_, _| panic!("no network")), true);
        assert!(off.get("7+x@users.noreply.github.com", None).is_none());
    }
}
