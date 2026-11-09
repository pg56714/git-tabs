//! The app's own settings file (open repos, identities, preferences).

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct RepoEntry {
    pub path: String,
    /// Identity id pinned to this repo, overriding the rules.
    pub pin: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct Rule {
    /// "remote" (URL glob like github.com/org/*) or "dir" (folder prefix)
    pub t: String,
    pub v: String,
}

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct Identity {
    pub id: String,
    pub name: String,
    pub short: String,
    pub color: String,
    pub key: String,
    pub account: String,
    pub verified: bool,
    pub user: String,
    pub email: String,
    pub host: String,
    pub rules: Vec<Rule>,
}

/// A line this app added to the global gitconfig, so it can take it out again.
#[derive(Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(default)]
pub struct Managed {
    pub key: String,
    pub value: String,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct AppConfig {
    pub repos: Vec<RepoEntry>,
    pub active: usize,
    pub identities: Vec<Identity>,
    pub default_identity: Option<String>,
    pub theme: String,
    pub git_path: String,
    pub fetch_minutes: u32,
    pub clone_dir: String,
    pub managed: Vec<Managed>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            repos: vec![],
            active: 0,
            identities: vec![],
            default_identity: None,
            theme: "system".into(),
            git_path: String::new(),
            fetch_minutes: 10,
            clone_dir: String::new(),
            managed: vec![],
        }
    }
}

fn file(dir: &Path) -> PathBuf {
    dir.join("config.json")
}

pub fn load(dir: &Path) -> AppConfig {
    std::fs::read_to_string(file(dir)).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

pub fn save(dir: &Path, cfg: &AppConfig) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let tmp = dir.join("config.json.tmp");
    let s = serde_json::to_string_pretty(cfg).map_err(|e| e.to_string())?;
    std::fs::write(&tmp, s).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, file(dir)).map_err(|e| e.to_string())
}
