//! Identities: which SSH key / name / email a repo uses.
//!
//! Rules become `includeIf` entries in the global gitconfig, each pointing at a small
//! config file this app writes, so plain `git` in a terminal picks the same identity.

use crate::config::{Identity, Managed};
use crate::runner::{command, run};
use serde::Serialize;
use std::path::{Path, PathBuf};

fn slash(p: &str) -> String {
    p.replace('\\', "/")
}

pub fn ssh_command(key: &str) -> String {
    format!("ssh -i \"{}\" -o IdentitiesOnly=yes", slash(key.trim()))
}

fn identity_file(cfg_dir: &Path, id: &str) -> PathBuf {
    let safe: String = id.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect();
    cfg_dir.join("identities").join(format!("{safe}.gitconfig"))
}

fn git_cfg(args: &[&str]) -> Result<(), String> {
    let o = run(None, args, None, true);
    if o.ok() { Ok(()) } else { Err(o.message()) }
}

/// includeIf conditions for one rule.
pub fn conditions(t: &str, v: &str) -> Vec<String> {
    let v = v.trim();
    if v.is_empty() {
        return vec![];
    }
    if t == "dir" {
        let mut p = slash(v);
        if !p.ends_with('/') {
            p.push('/');
        }
        return vec![format!("gitdir/i:{p}")];
    }
    // "github.com/org/*" -> ssh and https URL globs
    let v = v.trim_start_matches("https://").trim_start_matches("ssh://").trim_start_matches("git@");
    let v = v.replacen(':', "/", 1);
    let (host, rest) = v.split_once('/').unwrap_or((v.as_str(), "**"));
    let rest = if rest.ends_with('*') && !rest.ends_with("**") { format!("{rest}*") } else { rest.to_string() };
    vec![
        format!("hasconfig:remote.*.url:git@{host}:{rest}"),
        format!("hasconfig:remote.*.url:https://{host}/{rest}"),
        format!("hasconfig:remote.*.url:ssh://git@{host}/{rest}"),
    ]
}

/// Rewrites this app's part of the global gitconfig. Returns what was added.
pub fn apply(cfg_dir: &Path, ids: &[Identity], default: Option<&str>, old: &[Managed]) -> Result<Vec<Managed>, String> {
    for m in old {
        let _ = run(None, &["config", "--global", "--fixed-value", "--unset-all", &m.key, &m.value], None, true);
    }
    let dir = cfg_dir.join("identities");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let mut added = Vec::new();
    let mut add = |key: String, value: String| -> Result<(), String> {
        git_cfg(&["config", "--global", "--add", &key, &value])?;
        added.push(Managed { key, value });
        Ok(())
    };
    let mut files = Vec::new();
    for id in ids {
        let f = identity_file(cfg_dir, &id.id);
        let fs = f.display().to_string();
        if !id.user.trim().is_empty() {
            git_cfg(&["config", "-f", &fs, "user.name", id.user.trim()])?;
        }
        if !id.email.trim().is_empty() {
            git_cfg(&["config", "-f", &fs, "user.email", id.email.trim()])?;
        }
        if !id.key.trim().is_empty() {
            git_cfg(&["config", "-f", &fs, "core.sshCommand", &ssh_command(&id.key)])?;
        }
        if !f.exists() {
            std::fs::write(&f, "").map_err(|e| e.to_string())?;
        }
        files.push((id, slash(&fs)));
    }
    // The fallback goes first so any matching includeIf further down wins.
    if let Some((_, f)) = files.iter().find(|(id, _)| Some(id.id.as_str()) == default) {
        add("include.path".into(), f.clone())?;
    }
    for (id, f) in &files {
        for r in &id.rules {
            for c in conditions(&r.t, &r.v) {
                add(format!("includeIf.{c}.path"), f.clone())?;
            }
        }
    }
    Ok(added)
}

/// Removes a key from the repo's own config. Git exits 5 when it was not set; that is not worth
/// a red line in the command log, so only a real removal is recorded.
fn unset_local(repo: &Path, key: &str) {
    let t = std::time::Instant::now();
    let o = run(Some(repo), &["config", "--local", "--unset-all", key], None, false);
    if o.ok() {
        crate::runner::record(&repo.display().to_string(), format!("git config --local --unset-all {key}"), t.elapsed().as_millis() as u64, true, "");
    }
}

/// Pins an identity to one repo by writing its settings into the repo's own config.
pub fn pin(repo: &Path, id: Option<&Identity>) -> Result<(), String> {
    match id {
        Some(id) => {
            for (k, v) in [("user.name", id.user.trim().to_string()), ("user.email", id.email.trim().to_string()), ("core.sshCommand", if id.key.trim().is_empty() { String::new() } else { ssh_command(&id.key) })] {
                if v.is_empty() {
                    unset_local(repo, k);
                } else {
                    let o = run(Some(repo), &["config", "--local", k, &v], None, true);
                    if !o.ok() {
                        return Err(o.message());
                    }
                }
            }
        }
        None => {
            for k in ["user.name", "user.email", "core.sshCommand"] {
                unset_local(repo, k);
            }
        }
    }
    Ok(())
}

#[derive(Serialize)]
pub struct SshTest {
    pub ok: bool,
    pub account: Option<String>,
    pub output: String,
}

/// `ssh -T git@host` with one key; GitHub answers "Hi <user>!" (and exits 1).
pub fn ssh_test(key: &str, host: &str) -> SshTest {
    let host = if host.trim().is_empty() { "github.com" } else { host.trim() };
    let t = std::time::Instant::now();
    let mut c = command("ssh");
    c.args(["-T", "-i", key.trim(), "-o", "IdentitiesOnly=yes", "-o", "BatchMode=yes", "-o", "StrictHostKeyChecking=accept-new", "-o", "ConnectTimeout=10", &format!("git@{host}")]);
    let (out, ok_run) = match c.output() {
        Ok(o) => (format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)).trim().to_string(), true),
        Err(e) => (format!("找不到 ssh：{e}"), false),
    };
    let account = out
        .split("Hi ")
        .nth(1)
        .and_then(|s| s.split('!').next())
        .or_else(|| out.split("Welcome to GitLab, @").nth(1).and_then(|s| s.split('!').next()))
        .map(|s| s.trim().to_string());
    crate::runner::record("", format!("ssh -T -i {} git@{host}", key.trim()), t.elapsed().as_millis() as u64, account.is_some(), &out);
    SshTest { ok: ok_run && account.is_some(), account, output: out }
}

/// Creates a new ed25519 key pair; never overwrites. Returns the public key.
pub fn keygen(path: &str, email: &str, passphrase: &str) -> Result<String, String> {
    let p = Path::new(path.trim());
    if p.exists() {
        return Err(format!("{} 已經存在，不會覆蓋", p.display()));
    }
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let mut c = command("ssh-keygen");
    c.args(["-q", "-t", "ed25519", "-C", email.trim(), "-f", path.trim(), "-N", passphrase]);
    let o = c.output().map_err(|e| format!("找不到 ssh-keygen：{e}"))?;
    crate::runner::record("", format!("ssh-keygen -t ed25519 -C {} -f {}", email.trim(), path.trim()), 0, o.status.success(), &String::from_utf8_lossy(&o.stderr));
    if !o.status.success() {
        return Err(String::from_utf8_lossy(&o.stderr).trim().to_string());
    }
    std::fs::read_to_string(format!("{}.pub", path.trim())).map(|s| s.trim().to_string()).map_err(|e| e.to_string())
}

#[derive(Serialize, Default)]
pub struct FoundKey {
    pub path: String,
    /// The comment at the end of the .pub line (often an email).
    pub comment: String,
    /// `Host` aliases in ~/.ssh/config that use this key, with their HostName.
    pub hosts: Vec<(String, String)>,
}

#[derive(Serialize, Default)]
pub struct FoundInclude {
    pub condition: String,
    pub path: String,
    pub name: String,
    pub email: String,
    pub key: String,
}

#[derive(Serialize, Default)]
pub struct Found {
    pub name: String,
    pub email: String,
    pub keys: Vec<FoundKey>,
    pub includes: Vec<FoundInclude>,
}

fn global(key: &str) -> String {
    let o = run(None, &["config", "--global", "--get", key], None, false);
    o.stdout.trim().to_string()
}

fn from_file(file: &str, key: &str) -> String {
    let o = run(None, &["config", "-f", file, "--get", key], None, false);
    o.stdout.trim().to_string()
}

/// `-i <path>` out of an ssh command line.
fn key_of(ssh: &str) -> String {
    let mut it = ssh.split_whitespace();
    while let Some(a) = it.next() {
        if a == "-i" {
            return it.next().unwrap_or_default().trim_matches(['"', '\'']).to_string();
        }
    }
    String::new()
}

fn expand(p: &str, home: &Path) -> String {
    let p = p.trim().trim_matches('"');
    match p.strip_prefix("~/").or_else(|| p.strip_prefix("~\\")) {
        Some(rest) => home.join(rest.replace(['/', '\\'], std::path::MAIN_SEPARATOR_STR)).display().to_string(),
        None => p.to_string(),
    }
}

/// What this computer already has: SSH keys in ~/.ssh, ~/.ssh/config hosts, and the global
/// git identity including any includeIf files. `skip` lists include files this app wrote.
pub fn detect(home: &Path, skip: &[String]) -> Found {
    let ssh = home.join(".ssh");
    let mut keys: Vec<FoundKey> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&ssh) {
        let mut paths: Vec<PathBuf> = rd.filter_map(|e| e.ok()).map(|e| e.path()).collect();
        paths.sort();
        for p in paths {
            let Some(pub_text) = p.extension().filter(|e| *e == "pub").and_then(|_| std::fs::read_to_string(&p).ok()) else { continue };
            let private = p.with_extension("");
            if !private.is_file() {
                continue;
            }
            let comment = pub_text.split_whitespace().nth(2).unwrap_or_default().to_string();
            keys.push(FoundKey { path: private.display().to_string(), comment, hosts: vec![] });
        }
    }
    // ~/.ssh/config: Host blocks with an IdentityFile point that key at a host.
    if let Ok(cfg) = std::fs::read_to_string(ssh.join("config")) {
        let mut hosts: Vec<String> = vec![];
        let mut hostname = String::new();
        let mut file = String::new();
        let flush = |hosts: &Vec<String>, hostname: &str, file: &str, keys: &mut Vec<FoundKey>| {
            if file.is_empty() {
                return;
            }
            let f = expand(file, home);
            let k = match keys.iter_mut().find(|k| k.path.replace('\\', "/").eq_ignore_ascii_case(&f.replace('\\', "/"))) {
                Some(k) => k,
                None => {
                    keys.push(FoundKey { path: f.clone(), ..Default::default() });
                    keys.last_mut().unwrap()
                }
            };
            for h in hosts {
                k.hosts.push((h.clone(), if hostname.is_empty() { h.clone() } else { hostname.to_string() }));
            }
        };
        for line in cfg.lines() {
            let l = line.trim();
            let (k, v) = l.split_once(|c: char| c.is_whitespace() || c == '=').map(|(a, b)| (a.to_ascii_lowercase(), b.trim().to_string())).unwrap_or_default();
            match k.as_str() {
                "host" | "match" => {
                    flush(&hosts, &hostname, &file, &mut keys);
                    hosts = if k == "host" { v.split_whitespace().filter(|h| !h.contains('*')).map(String::from).collect() } else { vec![] };
                    hostname.clear();
                    file.clear();
                }
                "hostname" => hostname = v,
                "identityfile" => file = v,
                _ => {}
            }
        }
        flush(&hosts, &hostname, &file, &mut keys);
    }
    let mut includes = Vec::new();
    let o = run(None, &["config", "--global", "--get-regexp", r"^includeif\..*\.path$"], None, false);
    for line in o.stdout.lines() {
        let Some((k, v)) = line.split_once(' ') else { continue };
        let cond = k.trim_start_matches("includeif.").trim_end_matches(".path").to_string();
        let path = expand(v, home);
        if skip.iter().any(|s| s.replace('\\', "/").eq_ignore_ascii_case(&path.replace('\\', "/"))) {
            continue;
        }
        includes.push(FoundInclude {
            name: from_file(&path, "user.name"),
            email: from_file(&path, "user.email"),
            key: key_of(&from_file(&path, "core.sshCommand")),
            condition: cond,
            path,
        });
    }
    Found { name: global("user.name"), email: global("user.email"), keys, includes }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rule_conditions() {
        assert_eq!(conditions("dir", "D:\\work"), vec!["gitdir/i:D:/work/"]);
        let c = conditions("remote", "github.com/acme/*");
        assert_eq!(c[0], "hasconfig:remote.*.url:git@github.com:acme/**");
        assert_eq!(c[1], "hasconfig:remote.*.url:https://github.com/acme/**");
        assert!(conditions("remote", " ").is_empty());
    }
}
