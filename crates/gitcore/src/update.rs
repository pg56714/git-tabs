//! Looking for a newer release on GitHub, and replacing the running program with it.
//!
//! Downloads go through `curl`, which every system git-tabs runs on already has, so the app
//! carries no HTTP stack of its own. `GIT_TABS_UPDATE_URL` points the check somewhere else (a
//! test server, or a `file://` JSON file); `GIT_TABS_NO_UPDATE_CHECK=1` turns the check off.

use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::process::Command;

const API: &str = "https://api.github.com/repos/pg56714/git-tabs/releases/latest";

#[derive(Debug, Clone, Deserialize)]
pub struct Asset {
    pub name: String,
    pub browser_download_url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Release {
    pub tag_name: String,
    #[serde(default)]
    pub html_url: String,
    #[serde(default)]
    pub assets: Vec<Asset>,
}

impl Release {
    pub fn version(&self) -> &str {
        self.tag_name.trim_start_matches('v')
    }
}

/// How this copy of the program was installed, which decides how it can be replaced.
#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    /// A Linux AppImage: the file itself is swapped.
    AppImage(PathBuf),
    /// A Windows install under Program Files or AppData\Programs: the setup program does it.
    Installer,
    /// A loose Windows exe: swapped like the AppImage.
    Portable(PathBuf),
    /// .deb, .dmg and everything else: the release page is opened.
    Manual,
}

/// What the caller does after a successful `apply`.
#[derive(Debug, PartialEq)]
pub enum Outcome {
    /// Start this program again and quit.
    Restart(PathBuf),
    /// The installer is running; quit so it can replace the files.
    Quit,
    /// Show this page to the user.
    OpenPage(String),
}

/// "1.2.3" or "v1.2.3" (a "-beta" tail is ignored) as numbers.
fn numbers(v: &str) -> Option<Vec<u64>> {
    let v = v.trim().trim_start_matches('v');
    let core = v.split(['-', '+']).next()?;
    let n: Option<Vec<u64>> = core.split('.').map(|p| p.parse().ok()).collect();
    n.filter(|n| !n.is_empty())
}

/// Whether release tag `tag` is newer than `current`. Anything unreadable is not newer.
pub fn is_newer(current: &str, tag: &str) -> bool {
    match (numbers(current), numbers(tag)) {
        (Some(mut a), Some(mut b)) => {
            let n = a.len().max(b.len());
            a.resize(n, 0);
            b.resize(n, 0);
            b > a
        }
        _ => false,
    }
}

pub fn parse_release(json: &str) -> Result<Release, String> {
    serde_json::from_str(json).map_err(|e| format!("看不懂 release 資料：{e}"))
}

fn curl() -> Command {
    let mut c = Command::new("curl");
    c.args([
        "-fsSL",
        "-H",
        "User-Agent: git-tabs",
        "-H",
        "Accept: application/vnd.github+json",
    ]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000);
    }
    c
}

fn fetch_text(url: &str) -> Result<String, String> {
    let o = curl()
        .args(["--max-time", "30", url])
        .output()
        .map_err(|e| format!("找不到 curl：{e}"))?;
    if o.status.success() {
        Ok(String::from_utf8_lossy(&o.stdout).into_owned())
    } else {
        Err(String::from_utf8_lossy(&o.stderr).trim().to_string())
    }
}

/// The checksum of `name` in the text of a `SHA256SUMS` file (`<hex>  <name>` per line).
fn expected_sum(sums: &str, name: &str) -> Option<String> {
    sums.lines().find_map(|l| {
        let (hash, file) = l.trim().split_once(char::is_whitespace)?;
        (file.trim_start().trim_start_matches('*') == name).then(|| hash.to_ascii_lowercase())
    })
}

fn sha256_hex(p: &Path) -> Result<String, String> {
    use sha2::{Digest, Sha256};
    let mut f = std::fs::File::open(p).map_err(|e| e.to_string())?;
    let mut h = Sha256::new();
    std::io::copy(&mut f, &mut h).map_err(|e| e.to_string())?;
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// Checks the downloaded `file` against the release's `SHA256SUMS`. A release without that file
/// (made before it existed) is accepted; one that has it but does not list this file, or lists a
/// different checksum, is refused.
fn verify(rel: &Release, asset: &Asset, file: &Path) -> Result<(), String> {
    let Some(sums) = rel.assets.iter().find(|a| a.name == "SHA256SUMS") else {
        return Ok(());
    };
    let text =
        fetch_text(&sums.browser_download_url).map_err(|e| format!("無法取得 SHA256SUMS：{e}"))?;
    let want = expected_sum(&text, &asset.name)
        .ok_or_else(|| format!("SHA256SUMS 沒有列出 {}", asset.name))?;
    if sha256_hex(file)? != want {
        return Err("下載的檔案和 SHA256SUMS 不符（檔案損壞或被改過）".into());
    }
    Ok(())
}

fn download(url: &str, to: &Path) -> Result<(), String> {
    let o = curl()
        .args(["--max-time", "900", "-o"])
        .arg(to)
        .arg(url)
        .output()
        .map_err(|e| format!("找不到 curl：{e}"))?;
    if o.status.success() {
        Ok(())
    } else {
        let _ = std::fs::remove_file(to);
        Err(String::from_utf8_lossy(&o.stderr).trim().to_string())
    }
}

/// The latest release when it is newer than `current`.
pub fn check(current: &str) -> Result<Option<Release>, String> {
    if std::env::var("GIT_TABS_NO_UPDATE_CHECK").is_ok_and(|v| v == "1") {
        return Ok(None);
    }
    let url = std::env::var("GIT_TABS_UPDATE_URL").unwrap_or_else(|_| API.to_string());
    let rel = parse_release(&fetch_text(&url)?)?;
    Ok(is_newer(current, &rel.tag_name).then_some(rel))
}

/// The kind of the program that is running now.
pub fn kind() -> Kind {
    let exe = std::env::current_exe().unwrap_or_default();
    if cfg!(target_os = "linux") {
        return match std::env::var("APPIMAGE") {
            Ok(p) if !p.is_empty() => Kind::AppImage(PathBuf::from(p)),
            _ => Kind::Manual,
        };
    }
    if cfg!(windows) {
        let s = exe.display().to_string().to_lowercase().replace('/', "\\");
        return if s.contains("\\program files") || s.contains("\\appdata\\local\\programs\\") {
            Kind::Installer
        } else {
            Kind::Portable(exe)
        };
    }
    Kind::Manual
}

/// The release file that fits `kind`.
pub fn asset_for<'a>(rel: &'a Release, kind: &Kind) -> Option<&'a Asset> {
    let find = |f: &dyn Fn(&str) -> bool| rel.assets.iter().find(|a| f(&a.name.to_lowercase()));
    match kind {
        Kind::AppImage(_) => find(&|n| {
            n.ends_with(".appimage")
                && (n.contains("amd64") || n.contains("x86_64") || !cfg!(target_arch = "aarch64"))
        }),
        Kind::Installer => find(&|n| n.ends_with("-setup.exe")),
        Kind::Portable(_) => find(&|n| n.ends_with(".exe") && !n.contains("setup")),
        Kind::Manual => None,
    }
}

/// Downloads the release file for this copy and puts it in place. A download that is not a
/// program (an error page, a cut-off file) never replaces anything.
pub fn apply(rel: &Release) -> Result<Outcome, String> {
    apply_to(rel, &kind())
}

pub fn apply_to(rel: &Release, kind: &Kind) -> Result<Outcome, String> {
    let page = || {
        Outcome::OpenPage(if rel.html_url.is_empty() {
            "https://github.com/pg56714/git-tabs/releases/latest".to_string()
        } else {
            rel.html_url.clone()
        })
    };
    let Some(asset) = asset_for(rel, kind) else {
        return Ok(page());
    };
    match kind {
        Kind::AppImage(target) | Kind::Portable(target) => {
            let name = target
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            // Next to the target, so the final rename stays on one disk.
            let new = target.with_file_name(format!("{name}.new"));
            download(&asset.browser_download_url, &new).map_err(|e| format!("下載失敗：{e}"))?;
            if let Err(e) = check_program(&new).and_then(|_| verify(rel, asset, &new)) {
                let _ = std::fs::remove_file(&new);
                return Err(e);
            }
            replace(&new, target)?;
            Ok(Outcome::Restart(target.clone()))
        }
        Kind::Installer => {
            let to = std::env::temp_dir().join(&asset.name);
            download(&asset.browser_download_url, &to).map_err(|e| format!("下載失敗：{e}"))?;
            if let Err(e) = check_program(&to).and_then(|_| verify(rel, asset, &to)) {
                let _ = std::fs::remove_file(&to);
                return Err(e);
            }
            Command::new(&to)
                .spawn()
                .map_err(|e| format!("無法啟動安裝程式：{e}"))?;
            Ok(Outcome::Quit)
        }
        Kind::Manual => Ok(page()),
    }
}

/// At least a megabyte, and starting like an executable (ELF, or MZ on Windows).
fn check_program(p: &Path) -> Result<(), String> {
    use std::io::Read;
    let mut head = [0u8; 4];
    let mut f = std::fs::File::open(p).map_err(|e| e.to_string())?;
    let len = f.metadata().map_err(|e| e.to_string())?.len();
    f.read_exact(&mut head)
        .map_err(|_| "下載的檔案不完整".to_string())?;
    if len < 1_000_000 || !(head == *b"\x7fELF" || head[..2] == *b"MZ") {
        return Err("下載的檔案不是程式（可能被中斷或是錯誤頁面）".into());
    }
    Ok(())
}

/// Puts `new` where `target` is. A running program can be renamed on both Windows and Linux,
/// not overwritten, so the old file moves aside first and is put back if the move fails.
fn replace(new: &Path, target: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(new, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| e.to_string())?;
    }
    let name = target
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let old = target.with_file_name(format!("{name}.old"));
    let _ = std::fs::remove_file(&old);
    if target.exists() {
        std::fs::rename(target, &old).map_err(|e| format!("無法取代目前的程式：{e}"))?;
    }
    if let Err(e) = std::fs::rename(new, target) {
        let _ = std::fs::rename(&old, target);
        return Err(format!("無法換上新版：{e}"));
    }
    Ok(())
}

/// Removes what a finished update left next to the program (the old file, on Windows).
pub fn tidy() {
    if let Kind::AppImage(p) | Kind::Portable(p) = kind() {
        let name = p
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let _ = std::fs::remove_file(p.with_file_name(format!("{name}.old")));
    }
}

/// Starts `program` again, detached.
pub fn restart(program: &Path) -> Result<(), String> {
    let mut c = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000 | 0x0000_0008);
    }
    c.spawn().map(|_| ()).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_by_number() {
        assert!(is_newer("0.4.1", "v0.4.2"));
        assert!(is_newer("0.4.9", "0.5.0"));
        assert!(is_newer("0.4.1", "v0.10.0"), "not as text");
        assert!(is_newer("0.4", "0.4.1"));
        assert!(!is_newer("0.4.1", "v0.4.1"));
        assert!(!is_newer("0.5.0", "v0.4.9"));
        assert!(!is_newer("0.4.1", "nightly"), "unreadable is not newer");
        assert!(!is_newer("0.4.1", "v0.4.1-beta"));
    }

    fn release(base: &str) -> Release {
        let json = format!(
            r#"{{"tag_name":"v9.9.9","html_url":"https://example.test/r","assets":[
            {{"name":"git-tabs_9.9.9_x64-setup.exe","browser_download_url":"{base}/setup.exe"}},
            {{"name":"git-tabs_windows_x64.exe","browser_download_url":"{base}/portable.exe"}},
            {{"name":"git-tabs_9.9.9_amd64.deb","browser_download_url":"{base}/x.deb"}},
            {{"name":"git-tabs_9.9.9_amd64.AppImage","browser_download_url":"{base}/app.AppImage"}}]}}"#
        );
        parse_release(&json).unwrap()
    }

    #[test]
    fn each_kind_gets_its_own_file() {
        let r = release("https://x");
        assert_eq!(r.version(), "9.9.9");
        assert!(
            asset_for(&r, &Kind::Installer)
                .unwrap()
                .name
                .ends_with("-setup.exe")
        );
        assert_eq!(
            asset_for(&r, &Kind::Portable("a".into())).unwrap().name,
            "git-tabs_windows_x64.exe"
        );
        assert!(
            asset_for(&r, &Kind::AppImage("a".into()))
                .unwrap()
                .name
                .ends_with(".AppImage")
        );
        assert!(asset_for(&r, &Kind::Manual).is_none());
    }

    fn url(p: &Path) -> String {
        format!(
            "file:///{}",
            p.display()
                .to_string()
                .replace('\\', "/")
                .trim_start_matches('/')
        )
    }

    fn fake_program(p: &Path, tag: &str) {
        let mut b = b"\x7fELF".to_vec();
        b.extend(tag.as_bytes());
        b.resize(1_200_000, 0);
        std::fs::write(p, b).unwrap();
    }

    /// The whole swap against a local "release": the file is replaced and the old one is kept aside.
    #[test]
    fn an_appimage_is_replaced_by_the_download() {
        let d = tempfile::tempdir().unwrap();
        let served = d.path().join("served.AppImage");
        fake_program(&served, "NEW");
        let target = d.path().join("git-tabs.AppImage");
        fake_program(&target, "OLD");
        let json = d.path().join("latest.json");
        std::fs::write(&json, format!(r#"{{"tag_name":"v9.9.9","assets":[{{"name":"git-tabs_9.9.9_amd64.AppImage","browser_download_url":"{}"}}]}}"#, url(&served))).unwrap();
        // SAFETY: only this test in the binary reads these.
        unsafe { std::env::set_var("GIT_TABS_UPDATE_URL", url(&json)) };
        let rel = check("0.4.1").unwrap().expect("a newer release");
        assert_eq!(rel.version(), "9.9.9");
        let out = apply_to(&rel, &Kind::AppImage(target.clone())).unwrap();
        assert_eq!(out, Outcome::Restart(target.clone()));
        assert!(std::fs::read(&target).unwrap().starts_with(b"\x7fELFNEW"));
        assert!(
            std::fs::read(d.path().join("git-tabs.AppImage.old"))
                .unwrap()
                .starts_with(b"\x7fELFOLD")
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&target).unwrap().permissions().mode() & 0o111,
                0o111,
                "executable"
            );
        }
        // Same version: nothing to do.
        assert!(check("9.9.9").unwrap().is_none());
    }

    #[test]
    fn a_download_that_is_not_a_program_replaces_nothing() {
        let d = tempfile::tempdir().unwrap();
        let served = d.path().join("page.html");
        std::fs::write(&served, "<html>Not Found</html>").unwrap();
        let target = d.path().join("git-tabs.AppImage");
        fake_program(&target, "OLD");
        let rel = release(&url(d.path()));
        let mut rel = rel;
        for a in &mut rel.assets {
            a.browser_download_url = url(&served);
        }
        assert!(apply_to(&rel, &Kind::AppImage(target.clone())).is_err());
        assert!(std::fs::read(&target).unwrap().starts_with(b"\x7fELFOLD"));
        assert!(!d.path().join("git-tabs.AppImage.new").exists());
        // A missing file is an error too.
        for a in &mut rel.assets {
            a.browser_download_url = url(&d.path().join("nope"));
        }
        assert!(apply_to(&rel, &Kind::AppImage(target)).is_err());
    }

    #[test]
    fn reads_a_checksum_list() {
        let sums = "ABC123  git-tabs_windows_x64.exe
0f0f *other.AppImage
";
        assert_eq!(
            expected_sum(sums, "git-tabs_windows_x64.exe").as_deref(),
            Some("abc123")
        );
        assert_eq!(
            expected_sum(sums, "other.AppImage").as_deref(),
            Some("0f0f")
        );
        assert_eq!(expected_sum(sums, "missing.exe"), None);
    }

    #[test]
    fn a_checksum_mismatch_blocks_the_swap() {
        let d = tempfile::tempdir().unwrap();
        let (served, sums, target) = (
            d.path().join("served.AppImage"),
            d.path().join("SHA256SUMS"),
            d.path().join("git-tabs.AppImage"),
        );
        fake_program(&served, "NEW");
        fake_program(&target, "OLD");
        let rel = |sums_url: String| Release {
            tag_name: "v9.9.9".into(),
            html_url: String::new(),
            assets: vec![
                Asset {
                    name: "git-tabs_9.9.9_amd64.AppImage".into(),
                    browser_download_url: url(&served),
                },
                Asset {
                    name: "SHA256SUMS".into(),
                    browser_download_url: sums_url,
                },
            ],
        };
        // A wrong checksum: refused, and the old file stays.
        std::fs::write(
            &sums,
            format!(
                "{}  git-tabs_9.9.9_amd64.AppImage
",
                "0".repeat(64)
            ),
        )
        .unwrap();
        let err = apply_to(&rel(url(&sums)), &Kind::AppImage(target.clone())).unwrap_err();
        assert!(err.contains("SHA256SUMS"), "{err}");
        assert!(std::fs::read(&target).unwrap().starts_with(b"\x7fELFOLD"));
        assert!(!d.path().join("git-tabs.AppImage.new").exists());
        // A file the list does not name: refused too.
        std::fs::write(
            &sums,
            format!(
                "{}  other.AppImage
",
                sha256_hex(&served).unwrap()
            ),
        )
        .unwrap();
        assert!(apply_to(&rel(url(&sums)), &Kind::AppImage(target.clone())).is_err());
        // The right checksum: swapped.
        std::fs::write(
            &sums,
            format!(
                "{}  git-tabs_9.9.9_amd64.AppImage
",
                sha256_hex(&served).unwrap()
            ),
        )
        .unwrap();
        assert!(apply_to(&rel(url(&sums)), &Kind::AppImage(target.clone())).is_ok());
        assert!(std::fs::read(&target).unwrap().starts_with(b"\x7fELFNEW"));
    }
}
