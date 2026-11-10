//! State and actions of the bigger dialogs: add repo, settings (identities) and blame.

use std::collections::HashSet;
use std::path::PathBuf;

use gitcore::config::{Identity, Rule};
use gitcore::ops;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::{AppContext as _, Context, PathPromptOptions, Window};
use serde_json::json;

use crate::app::GitTabs;
use crate::identity::{from_found, new_identity, rule_for_url};
use crate::model::*;

fn input(window: &mut Window, cx: &mut Context<GitTabs>, value: &str, placeholder: &str) -> gpui_kit::Entity<InputState> {
    let (v, ph) = (value.to_string(), placeholder.to_string());
    cx.new(|cx| {
        let mut s = InputState::new(window, cx).placeholder(ph);
        s.set_value(v, window, cx);
        s
    })
}

fn clean(e: &str) -> String {
    e.trim_start_matches("Error: ").to_string()
}

impl GitTabs {
    // ----- add repo -----

    pub fn open_add_repo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.menu = None;
        // No built-in default: the folder is whatever the user scanned before, or empty.
        let dir = self.cfg.clone_dir.clone();
        let scan_dir = input(window, cx, &dir, crate::i18n::t("輸入或選擇要掃描的資料夾"));
        let url = input(window, cx, "", "git@github.com:org/repo.git");
        let clone_to = input(window, cx, "", crate::i18n::t("存放位置"));
        let sub = cx.subscribe_in(&scan_dir, window, |this, _, ev: &InputEvent, _, cx| {
            if matches!(ev, InputEvent::PressEnter { .. }) {
                this.add_repo_scan(cx);
            }
        });
        self.modal = Some(Modal::AddRepo(Box::new(AddRepoModal { scan_dir, found: Vec::new(), picked: HashSet::new(), pin: String::new(), url, clone_to, busy: false, err: None, _subs: vec![sub] })));
        if !dir.is_empty() {
            self.add_repo_scan(cx);
        }
        cx.notify();
    }

    pub fn add_repo_scan(&mut self, cx: &mut Context<Self>) {
        let Some(Modal::AddRepo(m)) = &mut self.modal else { return };
        m.err = None;
        let dir = m.scan_dir.read(cx).value().trim().to_string();
        if dir.is_empty() {
            m.err = Some(crate::i18n::t("先輸入或選擇要掃描的資料夾").into());
            m.found.clear();
            cx.notify();
            return;
        }
        if self.cfg.clone_dir != dir && std::path::Path::new(&dir).is_dir() {
            self.cfg.clone_dir = dir.clone();
            self.save_config();
        }
        self.work(cx, move || ops::scan(std::path::Path::new(&dir)), |this, found, cx| {
            if let Some(Modal::AddRepo(m)) = &mut this.modal {
                m.err = found.is_empty().then(|| crate::i18n::t("這個資料夾底下沒有找到 git repo").to_string());
                m.found = found;
                m.picked.retain(|p| m.found.contains(p));
            }
            cx.notify();
        });
    }

    pub fn add_repo_browse(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions { files: false, directories: true, multiple: false, prompt: Some(crate::i18n::t("選擇要掃描的資料夾").into()) });
        let Some(Modal::AddRepo(m)) = &self.modal else { return };
        let scan = m.scan_dir.clone();
        let handle = window.window_handle();
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = rx.await else { return };
            let Some(p) = paths.into_iter().next() else { return };
            let _ = handle.update(cx, |_, window, cx| scan.update(cx, |s, cx| s.set_value(p.display().to_string(), window, cx)));
            let _ = this.update(cx, |this, cx| this.add_repo_scan(cx));
        })
        .detach();
    }

    pub fn add_repo_pick_one(&mut self, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions { files: false, directories: true, multiple: false, prompt: Some(crate::i18n::t("選擇 repo 資料夾").into()) });
        let pin = match &self.modal {
            Some(Modal::AddRepo(m)) if !m.pin.is_empty() => Some(m.pin.clone()),
            _ => None,
        };
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = rx.await else { return };
            let _ = this.update(cx, |this, cx| {
                this.modal = None;
                this.add_repos(paths, pin, cx);
            });
        })
        .detach();
    }

    pub fn add_repo_add_picked(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(Modal::AddRepo(m)) = &self.modal else { return };
        let paths: Vec<PathBuf> = m.found.iter().filter(|p| m.picked.contains(*p)).map(PathBuf::from).collect();
        let pin = Some(m.pin.clone()).filter(|p| !p.is_empty());
        if paths.is_empty() {
            return;
        }
        self.close_modal(window, cx);
        self.add_repos(paths, pin, cx);
    }

    pub fn add_repo_clone(&mut self, cx: &mut Context<Self>) {
        let Some(Modal::AddRepo(m)) = &mut self.modal else { return };
        m.err = None;
        let url = m.url.read(cx).value().trim().to_string();
        if url.is_empty() {
            m.err = Some(crate::i18n::t("輸入 repo 網址").into());
            cx.notify();
            return;
        }
        let name = url.trim_end_matches(".git").rsplit(['/', ':']).next().unwrap_or("").to_string();
        let to = m.clone_to.read(cx).value().trim().to_string();
        let base = m.scan_dir.read(cx).value().trim_end_matches(['/', '\\']).to_string();
        let dir = if to.is_empty() { format!("{base}\\{name}") } else { to };
        let pin = Some(m.pin.clone()).filter(|p| !p.is_empty());
        m.busy = true;
        cx.notify();
        self.work(cx, move || ops::clone(&url, std::path::Path::new(&dir)), move |this, res, cx| {
            if let Some(Modal::AddRepo(m)) = &mut this.modal {
                m.busy = false;
            }
            match res {
                Ok(path) => {
                    this.modal = None;
                    this.pull_log();
                    this.add_repos(vec![PathBuf::from(&path)], pin, cx);
                    this.ok(crate::i18n::tr("已 clone {}", &[], &[(crate::format::base_name(&path)).to_string()]), cx);
                }
                Err(e) => {
                    if let Some(Modal::AddRepo(m)) = &mut this.modal {
                        m.err = Some(clean(&e));
                    }
                }
            }
            cx.notify();
        });
    }

    // ----- blame -----

    pub fn open_blame(&mut self, path: String, rev: Option<String>, cx: &mut Context<Self>) {
        let Some(repo) = self.active_path() else { return };
        self.menu = None;
        self.modal = Some(Modal::Blame(BlameModal { path: path.clone(), rev: rev.clone(), lines: Vec::new(), err: None, loading: true }));
        self.work(cx, move || ops::blame(&repo, &path, rev.as_deref()), |this, res, cx| {
            if let Some(Modal::Blame(m)) = &mut this.modal {
                m.loading = false;
                match res {
                    Ok(l) => m.lines = l,
                    Err(e) => m.err = Some(clean(&e)),
                }
            }
            cx.notify();
        });
        cx.notify();
    }

    // ----- settings -----

    fn make_draft(&mut self, id: Identity, window: &mut Window, cx: &mut Context<Self>) -> IdDraft {
        let rules = id.rules.iter().map(|r| RuleDraft { t: r.t.clone(), v: input(window, cx, &r.v, if r.t == "remote" { "github.com/your-org/*" } else { "D:\\work\\" }) }).collect();
        let slug: String = id.account.chars().chain(id.name.chars()).filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_').collect::<String>().to_lowercase();
        let slug = if slug.is_empty() { id.id.clone() } else { slug };
        let home = self.home_dir();
        IdDraft {
            name: input(window, cx, &id.name, crate::i18n::t("例如：個人 GitHub")),
            short: input(window, cx, &id.short, &crate::identity::auto_short(&id.name)),
            key: input(window, cx, &id.key, "C:\\Users\\you\\.ssh\\id_ed25519_xxx"),
            host: input(window, cx, &id.host, "github.com"),
            user: input(window, cx, &id.user, "Your Name"),
            email: input(window, cx, &id.email, "name@example.com"),
            gen_path: input(window, cx, &format!("{home}\\.ssh\\id_ed25519_{slug}"), ""),
            gen_pass: input(window, cx, "", crate::i18n::t("留空就不設密碼")),
            rules,
            test: None,
            testing: false,
            kg: KeyGen::default(),
            id,
        }
    }

    pub fn home_dir(&self) -> String {
        std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")).unwrap_or_else(|_| "C:\\Users\\user".into())
    }

    pub fn open_settings(&mut self, tab: SettingsTab, detected: Option<(Vec<Identity>, Option<String>)>, window: &mut Window, cx: &mut Context<Self>) {
        self.menu = None;
        let (ids, def) = match detected {
            Some((ids, def)) => (ids, def.unwrap_or_default()),
            None => (self.cfg.identities.clone(), self.cfg.default_identity.clone().unwrap_or_default()),
        };
        let notice = (!ids.is_empty() && self.cfg.identities.is_empty()).then(|| crate::i18n::tr("已從這台電腦找到 {} 個身分（~/.ssh 的 key 和現有的 git 設定）。確認名稱、email 和規則後按「儲存」才會生效；不需要的按 🗑 刪掉。", &[], &[(ids.len()).to_string()]));
        let drafts: Vec<IdDraft> = ids.into_iter().map(|i| self.make_draft(i, window, cx)).collect();
        let git = input(window, cx, &self.cfg.git_path.clone(), crate::i18n::t("留空就用 PATH 裡的 git"));
        let clone = input(window, cx, &self.cfg.clone_dir.clone(), crate::i18n::t("可留空"));
        self.modal = Some(Modal::Settings(Box::new(SettingsModal {
            tab,
            ids: drafts,
            default: def,
            theme: self.cfg.theme.clone(),
            git,
            fetch: self.cfg.fetch_minutes.to_string(),
            clone,
            err: None,
            saving: false,
            scanning: false,
            notice,
            _subs: Vec::new(),
        })));
        cx.notify();
    }

    pub fn settings_add(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let existing: Vec<Identity> = match &self.modal {
            Some(Modal::Settings(m)) => m.ids.iter().map(|d| d.id.clone()).collect(),
            _ => return,
        };
        let id = new_identity(&existing);
        let d = self.make_draft(id, window, cx);
        if let Some(Modal::Settings(m)) = &mut self.modal {
            m.ids.push(d);
        }
        cx.notify();
    }

    pub fn settings_remove(&mut self, i: usize, cx: &mut Context<Self>) {
        if let Some(Modal::Settings(m)) = &mut self.modal
            && i < m.ids.len()
        {
            let gone = m.ids.remove(i).id.id;
            if m.default == gone {
                m.default = m.ids.first().map(|d| d.id.id.clone()).unwrap_or_default();
            }
        }
        cx.notify();
    }

    pub fn settings_add_rule(&mut self, i: usize, window: &mut Window, cx: &mut Context<Self>) {
        let v = input(window, cx, "", "github.com/your-org/*");
        if let Some(Modal::Settings(m)) = &mut self.modal
            && let Some(d) = m.ids.get_mut(i)
        {
            d.rules.push(RuleDraft { t: "remote".into(), v });
        }
        cx.notify();
    }

    pub fn settings_scan(&mut self, cx: &mut Context<Self>) {
        let Some(Modal::Settings(m)) = &mut self.modal else { return };
        m.scanning = true;
        cx.notify();
        let ctx = self.ctx.clone();
        self.work(cx, move || gitcore::dispatch(&ctx, "identities_detect", json!({})), |this, res, cx| {
            let Some(Modal::Settings(m)) = &mut this.modal else { return };
            m.scanning = false;
            let existing: Vec<Identity> = m.ids.iter().map(|d| d.id.clone()).collect();
            match res {
                Ok(found) => {
                    let (ids, def) = from_found(&found, &existing);
                    let n = ids.len();
                    if m.default.is_empty()
                        && let Some(d) = def
                    {
                        m.default = d;
                    }
                    this.pending_ids = ids;
                    this.toast(if n > 0 { crate::i18n::tr("找到 {n} 個新的身分，確認後按儲存", &[("n", (n).to_string())], &[]) } else { crate::i18n::t("沒有找到新的 SSH key 或身分設定").into() }, ToastKind::Info, None, cx);
                }
                Err(_) => this.toast(crate::i18n::t("讀不到這台電腦的設定"), ToastKind::Err, None, cx),
            }
            cx.notify();
        });
    }

    /// Turns identities found by a scan into drafts (needs a window for the text boxes).
    pub fn settings_take_pending(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.pending_ids.is_empty() {
            return;
        }
        let ids = std::mem::take(&mut self.pending_ids);
        let drafts: Vec<IdDraft> = ids.into_iter().map(|i| self.make_draft(i, window, cx)).collect();
        if let Some(Modal::Settings(m)) = &mut self.modal {
            m.ids.extend(drafts);
        }
    }

    pub fn settings_browse_key(&mut self, i: usize, window: &mut Window, cx: &mut Context<Self>) {
        let home = self.home_dir();
        let _ = home;
        let rx = cx.prompt_for_paths(PathPromptOptions { files: true, directories: false, multiple: false, prompt: Some(crate::i18n::t("選擇 SSH 私鑰（不是 .pub）").into()) });
        let handle = window.window_handle();
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = rx.await else { return };
            let Some(p) = paths.into_iter().next() else { return };
            let path = p.display().to_string().trim_end_matches(".pub").to_string();
            let _ = this.update(cx, |this, cx| {
                if let Some(Modal::Settings(m)) = &mut this.modal
                    && let Some(d) = m.ids.get_mut(i)
                {
                    d.id.verified = false;
                    let key = d.key.clone();
                    let _ = handle.update(cx, |_, window, cx| key.update(cx, |s, cx| s.set_value(path, window, cx)));
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub fn settings_test(&mut self, i: usize, cx: &mut Context<Self>) {
        let Some(Modal::Settings(m)) = &mut self.modal else { return };
        let Some(d) = m.ids.get_mut(i) else { return };
        let key = d.key.read(cx).value().trim().to_string();
        if key.is_empty() {
            self.toast(crate::i18n::t("先填 SSH key 的路徑"), ToastKind::Info, None, cx);
            return;
        }
        d.testing = true;
        let host = { let h = d.host.read(cx).value().trim().to_string(); if h.is_empty() { "github.com".to_string() } else { h } };
        let id = d.id.id.clone();
        let ctx = self.ctx.clone();
        cx.notify();
        self.work(cx, move || gitcore::dispatch(&ctx, "ssh_test", json!({ "key": key, "host": host })), move |this, res, cx| {
            this.pull_log();
            if let Some(Modal::Settings(m)) = &mut this.modal
                && let Some(d) = m.ids.iter_mut().find(|d| d.id.id == id)
            {
                d.testing = false;
                match res {
                    Ok(v) => {
                        let ok = v["ok"].as_bool().unwrap_or(false);
                        let output = v["output"].as_str().unwrap_or("").to_string();
                        if ok && let Some(a) = v["account"].as_str() {
                            d.id.account = a.to_string();
                            d.id.verified = true;
                        }
                        d.test = Some(Ok(SshResult { ok, output }));
                    }
                    Err(e) => d.test = Some(Ok(SshResult { ok: false, output: e })),
                }
            }
            cx.notify();
        });
    }

    pub fn settings_gen(&mut self, i: usize, cx: &mut Context<Self>) {
        let Some(Modal::Settings(m)) = &mut self.modal else { return };
        let Some(d) = m.ids.get_mut(i) else { return };
        let email = d.email.read(cx).value().trim().to_string();
        if email.is_empty() {
            d.kg.err = Some(crate::i18n::t("先填 user.email，會寫進 key 的註解").into());
            cx.notify();
            return;
        }
        let (path, pass) = (d.gen_path.read(cx).value().to_string(), d.gen_pass.read(cx).value().to_string());
        d.kg.busy = true;
        d.kg.err = None;
        let id = d.id.id.clone();
        let ctx = self.ctx.clone();
        let p2 = path.clone();
        cx.notify();
        self.work(cx, move || gitcore::dispatch(&ctx, "ssh_keygen", json!({ "path": p2, "email": email, "passphrase": pass })), move |this, res, cx| {
            this.pull_log();
            if let Some(Modal::Settings(m)) = &mut this.modal
                && let Some(d) = m.ids.iter_mut().find(|d| d.id.id == id)
            {
                d.kg.busy = false;
                match res {
                    Ok(v) => {
                        d.kg.public = v.as_str().map(String::from);
                        d.id.verified = false;
                        d.kg.path = path;
                    }
                    Err(e) => d.kg.err = Some(clean(&e)),
                }
            }
            cx.notify();
        });
    }

    pub fn settings_save(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(Modal::Settings(m)) = &mut self.modal else { return };
        m.err = None;
        let mut ids: Vec<Identity> = Vec::new();
        for d in &m.ids {
            let mut id = d.id.clone();
            id.name = d.name.read(cx).value().trim().to_string();
            id.short = d.short.read(cx).value().trim().to_string();
            id.key = d.key.read(cx).value().trim().to_string();
            id.host = d.host.read(cx).value().trim().to_string();
            id.user = d.user.read(cx).value().trim().to_string();
            id.email = d.email.read(cx).value().trim().to_string();
            id.rules = d.rules.iter().map(|r| Rule { t: r.t.clone(), v: r.v.read(cx).value().trim().to_string() }).collect();
            if id.name.is_empty() {
                m.err = Some(crate::i18n::t("每個身分都要有名稱").into());
                cx.notify();
                return;
            }
            if !id.email.is_empty() && !(id.email.contains('@') && id.email.split('@').nth(1).is_some_and(|d| d.contains('.')) && !id.email.contains(char::is_whitespace)) {
                m.err = Some(crate::i18n::tr("「{}」的 email 格式不對", &[], &[(id.name).to_string()]));
                cx.notify();
                return;
            }
            ids.push(id);
        }
        m.saving = true;
        let def = Some(m.default.clone()).filter(|d| ids.iter().any(|i| &i.id == d));
        let (theme, git, clone) = (m.theme.clone(), m.git.read(cx).value().trim().to_string(), m.clone.read(cx).value().trim().to_string());
        let fetch: u32 = m.fetch.parse().unwrap_or(0);
        let ctx = self.ctx.clone();
        let (ids2, def2) = (ids.clone(), def.clone());
        cx.notify();
        self.work(cx, move || gitcore::dispatch(&ctx, "identities_apply", json!({ "identities": ids2, "default": def2 })), move |this, res, cx| {
            match res {
                Ok(_) => {
                    this.cfg.identities = ids.clone();
                    this.cfg.default_identity = def;
                    this.cfg.theme = theme;
                    this.cfg.git_path = git.clone();
                    this.cfg.fetch_minutes = fetch;
                    this.cfg.clone_dir = clone;
                    gitcore::runner::set_git_exe(&git);
                    this.save_config();
                    // Pinned repos keep their own copy of the identity; refresh it.
                    let pinned: Vec<(String, Option<String>)> = this.cfg.repos.iter().filter(|r| r.pin.is_some()).map(|r| (r.path.clone(), r.pin.clone())).collect();
                    for (path, pin) in pinned {
                        let ident = pin.as_ref().and_then(|p| ids.iter().find(|i| &i.id == p)).cloned();
                        if ident.is_none()
                            && let Some(e) = this.cfg.repos.iter_mut().find(|x| x.path == path)
                        {
                            e.pin = None;
                        }
                        let (c, v) = (this.ctx.clone(), json!({ "repo": path, "identity": ident }));
                        this.work(cx, move || drop(gitcore::dispatch(&c, "identity_pin", v)), |_, _, _| {});
                    }
                    this.save_config();
                    this.pull_log();
                    this.ok(crate::i18n::t("設定已儲存"), cx);
                    this.modal = None;
                    this.reapply_theme(cx);
                }
                Err(e) => {
                    if let Some(Modal::Settings(m)) = &mut this.modal {
                        m.saving = false;
                        m.err = Some(clean(&e));
                    }
                }
            }
            cx.notify();
        });
    }

    /// The rule a fresh repo would get, for the "指定給…" helper in settings.
    pub fn settings_assign(&mut self, repo_idx: usize, ident: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(r) = self.repos.get(repo_idx) else { return };
        let rule = r.summary.as_ref().and_then(|s| s.remote_url.as_deref()).and_then(rule_for_url);
        let (t, v) = match rule {
            Some(v) => ("remote", v),
            None => ("dir", r.path.display().to_string()),
        };
        let inp = input(window, cx, &v, "");
        if let Some(Modal::Settings(m)) = &mut self.modal
            && let Some(d) = m.ids.get_mut(ident)
        {
            d.rules.push(RuleDraft { t: t.into(), v: inp });
        }
        cx.notify();
    }
}
