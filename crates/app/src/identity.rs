//! Identity helpers. Identities are picked by hand per repo; the old automatic rules are only
//! matched here once, to pin the repos they covered (`match_identity`), and are then dropped.

use gitcore::config::Identity;
use serde_json::Value;

/// git@github.com:org/x.git, https://github.com/org/x, ssh://git@host/org/x -> host/org/x(.git)
pub fn norm_url(u: &str) -> String {
    let mut s = u.trim().to_string();
    let strip_user = |s: &str, prefix: &str| -> Option<String> {
        let rest = s.strip_prefix(prefix)?;
        // drop "user@" before the first slash
        match rest.split_once('/') {
            Some((head, tail)) if head.contains('@') => Some(format!(
                "{}/{tail}",
                head.rsplit('@').next().unwrap_or(head)
            )),
            _ => Some(rest.to_string()),
        }
    };
    if let Some(r) = strip_user(&s, "ssh://") {
        s = r;
    } else if let Some(rest) = s.strip_prefix("git@") {
        s = rest.replacen(':', "/", 1);
    } else if let Some(r) = strip_user(&s, "https://") {
        s = r;
    } else if let Some(r) = strip_user(&s, "http://") {
        s = r;
    }
    s
}

/// Glob where `*` matches anything; `ci` ignores case.
fn glob(pattern: &str, text: &str, ci: bool) -> bool {
    let fold = |s: &str| if ci { s.to_lowercase() } else { s.to_string() };
    let p: Vec<char> = fold(pattern).chars().collect();
    let t: Vec<char> = fold(text).chars().collect();
    let (mut pi, mut ti, mut star, mut mark) = (0usize, 0usize, None::<usize>, 0usize);
    while ti < t.len() {
        if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            mark = ti;
            pi += 1;
        } else if pi < p.len() && p[pi] == t[ti] {
            pi += 1;
            ti += 1;
        } else if let Some(s) = star {
            pi = s + 1;
            mark += 1;
            ti = mark;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

fn norm_dir(p: &str) -> String {
    let mut s = p.replace('\\', "/").to_lowercase();
    if !s.ends_with('/') {
        s.push('/');
    }
    s
}

/// Which identity the rules give a repo (pins are handled by the caller). The last match
/// wins, like git: includeIf lines are written in identity order.
pub fn match_identity<'a>(
    ids: &'a [Identity],
    repo_path: &str,
    remote_url: Option<&str>,
) -> Option<&'a Identity> {
    let url = norm_url(remote_url.unwrap_or(""));
    let dir = norm_dir(repo_path);
    let mut hit = None;
    for id in ids {
        for r in &id.rules {
            let v = r.v.trim();
            if v.is_empty() {
                continue;
            }
            if r.t == "remote" && !url.is_empty() {
                let pat = norm_url(v);
                // git compares the URL with includeIf case-sensitively; so does this.
                if glob(&pat, &url, false) || glob(&pat, url.trim_end_matches(".git"), false) {
                    hit = Some(id);
                }
            }
            if r.t == "dir" && dir.starts_with(&norm_dir(v)) {
                hit = Some(id);
            }
        }
    }
    hit
}

pub fn new_identity(existing: &[Identity]) -> Identity {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    Identity {
        id: format!("id{stamp:x}"),
        name: crate::i18n::tr("新 identity {}", &[], &[(existing.len() + 1).to_string()]),
        short: String::new(),
        color: String::new(),
        key: String::new(),
        account: String::new(),
        verified: false,
        user: String::new(),
        email: String::new(),
        host: "github.com".into(),
        rules: vec![],
    }
}

fn norm_path(p: &str) -> String {
    p.replace('\\', "/").to_lowercase()
}

fn file_name(p: &str) -> &str {
    p.rsplit(['/', '\\']).next().unwrap_or(p)
}

/// Identities for what is already on this computer: one per existing includeIf file, then one
/// per SSH key no include uses.
/// `found` is the JSON the `identities_detect` command returns.
pub fn from_found(found: &Value, existing: &[Identity]) -> Vec<Identity> {
    let text = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    let (f_name, f_email) = (text(found, "name"), text(found, "email"));
    let mut ids: Vec<Identity> = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    let add = |ids: &mut Vec<Identity>, existing: &[Identity], make: &dyn Fn(&mut Identity)| {
        let all: Vec<Identity> = existing
            .iter()
            .cloned()
            .chain(ids.iter().cloned())
            .collect();
        let mut id = new_identity(&all);
        make(&mut id);
        ids.push(id);
    };
    // One identity per include file, however many conditions point at it.
    for inc in found
        .get("includes")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let path = text(inc, "path");
        if seen.contains(&norm_path(&path)) {
            continue;
        }
        seen.push(norm_path(&path));
        let file = file_name(&path);
        let mut base = file
            .trim_end_matches(".gitconfig")
            .trim_end_matches(".config")
            .to_string();
        if let Some(rest) = base.strip_prefix(".gitconfig") {
            base = rest.trim_start_matches(['-', '_', '.']).to_string();
        }
        let email = text(inc, "email");
        if base.is_empty() {
            base = email.split('@').nth(1).unwrap_or("Identity").to_string();
        }
        let (n, e, k) = (text(inc, "name"), email, text(inc, "key"));
        add(&mut ids, existing, &|id| {
            id.name = base.clone();
            id.user = if n.is_empty() {
                f_name.clone()
            } else {
                n.clone()
            };
            id.email = if e.is_empty() {
                f_email.clone()
            } else {
                e.clone()
            };
            id.key = k.clone();
        });
    }
    let used: Vec<String> = existing
        .iter()
        .chain(ids.iter())
        .map(|i| norm_path(&i.key))
        .filter(|k| !k.is_empty())
        .collect();
    for k in found
        .get("keys")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let path = text(k, "path");
        if used.contains(&norm_path(&path)) {
            continue;
        }
        let file = file_name(&path).to_string();
        let is_default = matches!(file.as_str(), "id_ed25519" | "id_rsa" | "id_ecdsa");
        let hosts: Vec<(String, String)> = k
            .get("hosts")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|h| {
                Some((
                    h.get(0)?.as_str()?.to_string(),
                    h.get(1)?.as_str()?.to_string(),
                ))
            })
            .collect();
        let alias = hosts.iter().find(|(h, n)| h != n).map(|(h, _)| h.clone());
        let name = if is_default {
            "預設 key".to_string()
        } else {
            alias.unwrap_or_else(|| {
                let stripped = file
                    .strip_prefix("id_ed25519_")
                    .or_else(|| file.strip_prefix("id_rsa_"))
                    .or_else(|| file.strip_prefix("id_ecdsa_"))
                    .unwrap_or(&file);
                stripped.to_string()
            })
        };
        let comment = text(k, "comment");
        let comment_is_email = comment.split_once('@').is_some_and(|(u, d)| {
            !u.is_empty() && !u.contains(char::is_whitespace) && d.contains('.') && !d.contains('@')
        });
        let host = hosts
            .first()
            .map(|h| h.1.clone())
            .unwrap_or_else(|| "github.com".into());
        add(&mut ids, existing, &|id| {
            id.name = if name.is_empty() {
                file.clone()
            } else {
                name.clone()
            };
            id.user = f_name.clone();
            id.email = if comment_is_email {
                comment.clone()
            } else {
                f_email.clone()
            };
            id.key = path.clone();
            id.host = host.clone();
        });
    }
    ids
}

#[cfg(test)]
mod tests {
    use super::*;
    use gitcore::config::Rule;

    fn id(name: &str, rules: &[(&str, &str)]) -> Identity {
        let mut i = new_identity(&[]);
        i.name = name.into();
        i.rules = rules
            .iter()
            .map(|(t, v)| Rule {
                t: (*t).into(),
                v: (*v).into(),
            })
            .collect();
        i
    }

    #[test]
    fn urls_normalise() {
        assert_eq!(norm_url("git@github.com:org/x.git"), "github.com/org/x.git");
        assert_eq!(
            norm_url("https://user@github.com/org/x"),
            "github.com/org/x"
        );
        assert_eq!(norm_url("ssh://git@host/org/x"), "host/org/x");
    }

    #[test]
    fn rules_match_by_remote_or_dir_and_last_match_wins() {
        let a = id("A", &[("remote", "github.com/acme/*")]);
        let b = id("B", &[("dir", "D:\\work\\")]);
        let ids = vec![a, b];
        assert_eq!(
            match_identity(&ids, "C:\\x", Some("git@github.com:acme/app.git"))
                .unwrap()
                .name,
            "A"
        );
        assert_eq!(
            match_identity(&ids, "d:\\work\\proj", None).unwrap().name,
            "B"
        );
        assert_eq!(
            match_identity(&ids, "d:\\work\\proj", Some("git@github.com:acme/app.git"))
                .unwrap()
                .name,
            "B",
            "later identity wins"
        );
        assert!(match_identity(&ids, "C:\\x", Some("git@github.com:other/app.git")).is_none());
    }

    #[test]
    fn detects_identities_from_includes_and_keys() {
        let v = serde_json::json!({
            "name": "Sam", "email": "sam@x.com",
            "keys": [
                {"path": "C:/u/.ssh/id_ed25519", "comment": "sam@x.com", "hosts": []},
                {"path": "C:/u/.ssh/work", "comment": "host", "hosts": [["github-work", "github.com"]]}
            ],
            "includes": [
                {"condition": "gitdir/i:D:/work/", "path": "C:/u/.gitconfig-work", "name": "Sam W", "email": "w@corp.com", "key": "C:/u/.ssh/work"}
            ]
        });
        let ids = from_found(&v, &[]);
        assert_eq!(ids.len(), 2, "the work key is already used by the include");
        assert!(ids[0].rules.is_empty());
        assert_eq!(ids[0].email, "w@corp.com");
        assert_eq!(ids[1].name, "預設 key");
    }
}
