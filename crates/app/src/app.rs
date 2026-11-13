//! The window's state and everything that loads data (the Rust counterpart of src/store.ts).
//!
//! Git work always runs on the background executor; results come back to the UI thread
//! through [`GitTabs::work`]. Stale answers are dropped by comparing sequence numbers.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::time::{Duration, Instant};

use gitcore::config::{self, AppConfig, RepoEntry};
use gitcore::ops;
use gitcore::parse::FileDiff;
use gitcore::runner::LogEntry;
use gitcore::Ctx;
use gpui_kit::component::diff::{DiffFile, DiffMode, DiffState};
use gpui_kit::component::input::{InputEvent, InputState, TextareaState};
use gpui_kit::{
    AppContext as _, Context, Entity, FocusHandle, Render, Subscription, Task, Window, WindowAppearance,
};
use notify_debouncer_mini::{
    DebounceEventResult, Debouncer, new_debouncer, notify::RecommendedWatcher, notify::RecursiveMode,
};

use crate::model::*;
use crate::theme::{self, Pal};

mod acts {
    gpui_kit::actions!(
        git_tabs,
        [
            Refresh,
            Commit,
            CommitAndPush,
            OpenPalette,
            CloseOverlay,
            NextFile,
            PreviousFile,
            NextHunk,
            PreviousHunk,
            ToggleSeenAction,
            StageToggle,
            NextTab,
            PreviousTab,
            PaletteDown,
            PaletteUp,
            PaletteRun,
            ModalEnter,
            Tab1,
            Tab2,
            Tab3,
            Tab4,
            Tab5,
            Tab6,
            Tab7,
            Tab8,
            Tab9
        ]
    );
}
pub use acts::*;

#[derive(Clone, Copy, PartialEq)]
pub enum HistTab {
    Commit,
    Changes,
    Tree,
}

/// Runs queued stage/unstage calls strictly in the order they were issued.
pub struct Gate {
    turn: Mutex<u64>,
    cv: Condvar,
}

pub struct GitTabs {
    pub ctx: Arc<Ctx>,
    pub cfg: AppConfig,
    pub repos: Vec<RepoState>,
    pub active: usize,
    pub pal: Pal,
    pub diff: Entity<DiffState>,
    pub side_by_side: bool,
    pub wrap: bool,
    pub hist_tab: HistTab,
    /// "auto" (follow the system), "zh-TW" or "en"
    pub lang: String,
    pub cur_hunk: usize,
    /// The lines picked in the diff. The diff view forgets its own selection when the pointer
    /// goes down anywhere else (such as on the Stage button), so the last pick is kept here.
    pub stored_sel: Option<gpui_kit::component::diff::DiffLineRange>,
    pub bar_press: Option<Instant>,
    pub subject: Entity<InputState>,
    pub desc: Entity<TextareaState>,
    pub amend: bool,
    pub commit_err: String,
    pub committing: bool,
    pub search: Entity<InputState>,
    /// The sidebar's filter over branches, remotes, tags, stashes and worktrees.
    pub side_filter: Entity<InputState>,
    pub nav_focus: FocusHandle,
    pub modal_focus: FocusHandle,
    pub menu: Option<MenuState>,
    pub modal: Option<Modal>,
    pub palette: Option<Palette>,
    pub toasts: Vec<Toast>,
    pub toast_id: u64,
    pub log: Vec<LogEntry>,
    pub log_since: u64,
    pub log_open: bool,
    pub log_expand: Option<u64>,
    pub git_version: String,
    pub seen: HashMap<String, String>,
    pub collapsed: HashSet<String>,
    pub folded: HashSet<String>,
    pub window_size: gpui_kit::Size<gpui_kit::Pixels>,
    /// Where the window is, to open it there again next time.
    window_place: Option<WindowPlace>,
    window_save: Option<Task<()>>,
    pub sizes: Sizes,
    pub sizes_dirty: bool,
    pub pending_ids: Vec<gitcore::config::Identity>,
    /// Identities found on this computer at the first launch, waiting for a window to be shown in.
    pub pending_first_run: Option<Vec<gitcore::config::Identity>>,
    pub tab_scroll: gpui_kit::ScrollHandle,
    pub side_scroll: gpui_kit::ScrollHandle,
    pub files_scroll: gpui_kit::ScrollHandle,
    pub detail_scroll: gpui_kit::ScrollHandle,
    pub commit_scroll: gpui_kit::UniformListScrollHandle,
    pub log_scroll: gpui_kit::ScrollHandle,
    commit_diffs: VecDeque<(String, FileDiff)>,
    inflight: HashSet<PathBuf>,
    again: HashSet<PathBuf>,
    after_refresh: HashMap<PathBuf, Vec<Box<dyn FnOnce(&mut GitTabs, &mut Context<GitTabs>)>>>,
    diff_seq: u64,
    gate: Arc<Gate>,
    gate_next: u64,
    watcher: Option<Debouncer<RecommendedWatcher>>,
    search_timer: Task<()>,
    ticks: u64,
    _subs: Vec<Subscription>,
}

impl GitTabs {
    pub fn new(ctx: Arc<Ctx>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        window.set_window_title("git-tabs");
        let cfg = config::load(&ctx.config_dir);
        let repos: Vec<RepoState> =
            cfg.repos.iter().filter(|r| Path::new(&r.path).is_dir()).map(|r| RepoState::new(PathBuf::from(&r.path))).collect();
        let active = cfg.active.min(repos.len().saturating_sub(1));
        let dark = resolve_dark(&cfg.theme, window.appearance());
        theme::apply(dark, Some(window), cx);
        let prefs = load_prefs(&ctx.config_dir);
        let window_place = load_window(&ctx.config_dir);
        crate::i18n::set_from_setting(&prefs.lang);
        let diff = cx.new(|cx| DiffState::new([], cx).with_mode(DiffMode::Unified));
        let subject = cx.new(|cx| InputState::new(window, cx).placeholder(crate::i18n::t("Commit 標題")));
        let desc = cx.new(|cx| TextareaState::new(window, cx).placeholder(crate::i18n::t("說明（可留空）")));
        let search = cx.new(|cx| InputState::new(window, cx).placeholder(crate::i18n::t("搜尋 commit 訊息，或貼上 SHA")));
        let side_filter = cx.new(|cx| InputState::new(window, cx).placeholder(crate::i18n::t("篩選")));
        let nav_focus = cx.focus_handle().tab_stop(true);
        let mut subs = Vec::new();
        subs.push(cx.subscribe(&side_filter, |_, _, ev: &InputEvent, cx| {
            if matches!(ev, InputEvent::Change) {
                cx.notify();
            }
        }));
        subs.push(cx.observe(&diff, |_, _, cx| cx.notify()));
        subs.push(cx.subscribe(&diff, |this, _, ev: &gpui_kit::component::diff::DiffEvent, cx| {
            use gpui_kit::component::diff::DiffEvent;
            match ev {
                DiffEvent::SelectionChanged(Some(r)) | DiffEvent::SelectionEnded(r) => this.stored_sel = Some(r.clone()),
                DiffEvent::SelectionChanged(None) => {
                    // Pressing the selection bar itself must not lose the pick.
                    if this.bar_press.is_none_or(|t| t.elapsed() > Duration::from_millis(800)) {
                        this.stored_sel = None;
                    }
                }
                _ => {}
            }
            cx.notify();
        }));
        subs.push(cx.subscribe(&subject, |this, _, ev: &InputEvent, cx| {
            if matches!(ev, InputEvent::Change) {
                this.commit_err.clear();
                cx.notify();
            }
        }));
        subs.push(cx.subscribe(&search, |this, s, ev: &InputEvent, cx| {
            if matches!(ev, InputEvent::Change) {
                let q = s.read(cx).value().to_string();
                this.on_search(q, cx);
            }
        }));
        subs.push(cx.observe_window_bounds(window, |this, window, cx| this.window_moved(window, cx)));
        subs.push(cx.observe_window_appearance(window, |this, window, cx| {
            if this.cfg.theme == "system" {
                this.apply_theme(window, cx);
            }
        }));
        subs.push(cx.observe_window_activation(window, |this, window, cx| {
            if window.is_window_active() {
                this.refresh_active(cx);
            }
        }));
        let seen = load_seen(&ctx.config_dir);
        let mut this = Self {
            ctx,
            cfg,
            repos,
            active,
            pal: Pal::new(dark),
            diff,
            side_by_side: prefs.side,
            wrap: prefs.wrap,
            hist_tab: match prefs.hist_tab.as_str() {
                "changes" => HistTab::Changes,
                "tree" => HistTab::Tree,
                _ => HistTab::Commit,
            },
            lang: prefs.lang.clone(),
            cur_hunk: 0,
            stored_sel: None,
            bar_press: None,
            subject,
            desc,
            amend: false,
            commit_err: String::new(),
            committing: false,
            search,
            side_filter,
            nav_focus,
            modal_focus: cx.focus_handle(),
            menu: None,
            modal: None,
            palette: None,
            toasts: Vec::new(),
            toast_id: 0,
            log: Vec::new(),
            log_since: 0,
            log_open: false,
            log_expand: None,
            git_version: String::new(),
            seen,
            collapsed: HashSet::new(),
            folded: HashSet::from(["Tags".to_string()]),
            window_size: window.viewport_size(),
            window_place,
            window_save: None,
            sizes: prefs.sizes,
            sizes_dirty: false,
            pending_ids: Vec::new(),
            pending_first_run: None,
            tab_scroll: Default::default(),
            side_scroll: Default::default(),
            files_scroll: Default::default(),
            detail_scroll: Default::default(),
            commit_scroll: Default::default(),
            log_scroll: Default::default(),
            commit_diffs: VecDeque::new(),
            inflight: HashSet::new(),
            again: HashSet::new(),
            after_refresh: HashMap::new(),
            diff_seq: 0,
            gate: Arc::new(Gate { turn: Mutex::new(0), cv: Condvar::new() }),
            gate_next: 1,
            watcher: None,
            search_timer: Task::ready(()),
            ticks: 0,
            _subs: subs,
        };
        this.apply_diff_view(cx);
        this.nav_focus.focus(window, cx);
        this.watch(cx);
        this.start_timers(cx);
        this.work(cx, || gitcore::runner::run(None, &["--version"], None, false), |this, o, cx| {
            this.git_version = if o.ok() { o.stdout.trim().to_string() } else { String::new() };
            if !o.ok() {
                this.toast(crate::i18n::t("找不到 git，請到設定填 git 路徑"), ToastKind::Err, Some(o.message()), cx);
            }
            cx.notify();
        });
        this.offer_detected_identities(cx);
        for i in 0..this.repos.len() {
            let p = this.repos[i].path.clone();
            if i == this.active {
                this.open_repo(&p, cx);
            } else {
                this.refresh_summary(&p, cx);
            }
        }
        this
    }

    /// First launch only: fill the identity settings from SSH keys and gitconfig already on this computer.
    fn offer_detected_identities(&mut self, cx: &mut Context<Self>) {
        let flag = self.ctx.config_dir.join("identities-offered");
        if !self.cfg.identities.is_empty() || flag.exists() {
            return;
        }
        let _ = std::fs::create_dir_all(&self.ctx.config_dir);
        let _ = std::fs::write(&flag, "1");
        let ctx = self.ctx.clone();
        self.work(cx, move || gitcore::dispatch(&ctx, "identities_detect", serde_json::json!({})), |this, res, cx| {
            if let Ok(found) = res {
                let ids = crate::identity::from_found(&found, &[]);
                if !ids.is_empty() {
                    this.pending_first_run = Some(ids);
                    cx.notify();
                }
            }
        });
    }

    // ----- plumbing -----

    /// Runs `job` off the UI thread, then `done` on it.
    pub fn work<T: Send + 'static>(
        &mut self,
        cx: &mut Context<Self>,
        job: impl FnOnce() -> T + Send + 'static,
        done: impl FnOnce(&mut Self, T, &mut Context<Self>) + 'static,
    ) {
        cx.spawn(async move |this, cx| {
            let out = cx.background_spawn(async move { job() }).await;
            let _ = this.update(cx, |this, cx| done(this, out, cx));
        })
        .detach();
    }

    /// A place in the serial queue of git commands that change a repo.
    pub fn take_ticket(&mut self) -> (Arc<Gate>, u64) {
        let t = self.gate_next;
        self.gate_next += 1;
        (self.gate.clone(), t)
    }

    pub fn repo(&self) -> Option<&RepoState> {
        self.repos.get(self.active)
    }

    pub fn repo_mut(&mut self) -> Option<&mut RepoState> {
        self.repos.get_mut(self.active)
    }

    pub fn repo_by_path(&mut self, path: &Path) -> Option<&mut RepoState> {
        self.repos.iter_mut().find(|r| r.path == path)
    }

    pub fn active_path(&self) -> Option<PathBuf> {
        self.repo().map(|r| r.path.clone())
    }

    pub fn toast(&mut self, text: impl Into<String>, kind: ToastKind, detail: Option<String>, cx: &mut Context<Self>) {
        self.toast_id += 1;
        let id = self.toast_id;
        let text: String = text.into();
        crate::debug::log("toast", || format!("{kind:?}: {text} {}", detail.as_deref().unwrap_or("")));
        self.toasts.push(Toast { id, text, kind, detail });
        let secs = if kind == ToastKind::Err { 9 } else { 3 };
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_secs(secs)).await;
            let _ = this.update(cx, |this, cx| {
                this.toasts.retain(|t| t.id != id);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub fn ok(&mut self, text: impl Into<String>, cx: &mut Context<Self>) {
        self.toast(text, ToastKind::Ok, None, cx);
    }

    /// Re-reads the theme setting and the system appearance (no window needed).
    pub fn reapply_theme(&mut self, cx: &mut Context<Self>) {
        let dark = resolve_dark(&self.cfg.theme, cx.window_appearance());
        self.pal = Pal::new(dark);
        theme::apply(dark, None, cx);
        cx.notify();
    }

    pub fn apply_theme(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let dark = resolve_dark(&self.cfg.theme, window.appearance());
        self.pal = Pal::new(dark);
        theme::apply(dark, Some(window), cx);
        cx.notify();
    }

    pub fn save_config(&mut self) {
        let old = std::mem::take(&mut self.cfg.repos);
        self.cfg.repos = self
            .repos
            .iter()
            .map(|r| RepoEntry { path: r.path.display().to_string(), pin: old.iter().find(|e| Path::new(&e.path) == r.path).and_then(|e| e.pin.clone()) })
            .collect();
        self.cfg.active = self.active;
        if let Err(e) = config::save(&self.ctx.config_dir, &self.cfg) {
            eprintln!("git-tabs: could not save the settings: {e}");
        }
    }

    /// Switches the interface language: "auto", "zh-TW" or "en".
    pub fn set_lang(&mut self, lang: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.lang = lang.to_string();
        crate::i18n::set_from_setting(lang);
        self.save_prefs();
        // Text boxes keep the placeholder they were made with.
        self.subject.update(cx, |s, cx| s.set_placeholder(crate::i18n::t("Commit 標題"), window, cx));
        self.desc.update(cx, |s, cx| s.set_placeholder(crate::i18n::t("說明（可留空）"), window, cx));
        self.search.update(cx, |s, cx| s.set_placeholder(crate::i18n::t("搜尋 commit 訊息，或貼上 SHA"), window, cx));
        self.side_filter.update(cx, |s, cx| s.set_placeholder(crate::i18n::t("篩選"), window, cx));
        cx.notify();
    }

    /// The window was moved, resized or maximized: remember it, and write it down once it rests.
    fn window_moved(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (b, maximized) = match window.window_bounds() {
            gpui_kit::WindowBounds::Windowed(b) => (b, false),
            gpui_kit::WindowBounds::Maximized(b) | gpui_kit::WindowBounds::Fullscreen(b) => (b, true),
        };
        let place = WindowPlace { rect: [b.origin.x.into(), b.origin.y.into(), b.size.width.into(), b.size.height.into()], maximized };
        if self.window_place == Some(place) {
            return;
        }
        self.window_place = Some(place);
        // A drag reports every step; replacing the task cancels the one before it.
        self.window_save = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_millis(600)).await;
            let _ = this.update(cx, |this, _| this.save_prefs());
        }));
    }

    pub fn save_prefs(&self) {
        let v = serde_json::json!({
            "window": self.window_place.map(|w| w.rect),
            "maximized": self.window_place.is_some_and(|w| w.maximized),
            "side": self.side_by_side,
            "wrap": self.wrap,
            "lang": self.lang,
            "hist_tab": match self.hist_tab {
                HistTab::Changes => "changes",
                HistTab::Tree => "tree",
                HistTab::Commit => "commit",
            },
            "sizes": { "side_bar": self.sizes.side_bar, "files": self.sizes.files, "hist_top": self.sizes.hist_top, "hist_files": self.sizes.hist_files },
        });
        let _ = std::fs::create_dir_all(&self.ctx.config_dir);
        let _ = std::fs::write(self.ctx.config_dir.join("prefs.json"), v.to_string());
    }

    pub fn apply_diff_view(&mut self, cx: &mut Context<Self>) {
        let mode = if self.side_by_side { DiffMode::Split } else { DiffMode::Unified };
        self.diff.update(cx, |d, cx| d.set_mode(mode, cx));
    }

    /// Pulls new entries of the command log gitcore keeps.
    pub fn pull_log(&mut self) {
        let items = gitcore::runner::log_since(self.log_since);
        if let Some(last) = items.last() {
            self.log_since = last.id;
            for l in &items {
                crate::debug::log("git", || format!("{} {} [{}ms] {} {}", if l.ok { "ok " } else { "ERR" }, l.cmd, l.ms, l.repo, if l.ok { String::new() } else { l.output.clone() }));
            }
            self.log.extend(items);
            let n = self.log.len();
            if n > 500 {
                self.log.drain(0..n - 500);
            }
        }
    }

    // ----- watching and timers -----

    /// Watches every repo and refreshes the one that changed on disk.
    pub fn watch(&mut self, cx: &mut Context<Self>) {
        let roots: Vec<PathBuf> = self.repos.iter().map(|r| r.path.clone()).collect();
        let (tx, rx) = mpsc::channel::<PathBuf>();
        let r2 = roots.clone();
        let Ok(mut deb) = new_debouncer(Duration::from_millis(400), move |res: DebounceEventResult| {
            let Ok(events) = res else { return };
            let mut hit: Vec<PathBuf> = Vec::new();
            for e in &events {
                let s = e.path.to_string_lossy().replace('\\', "/");
                // Object writes and lock files churn during git commands; build folders during builds.
                const NOISY: [&str; 8] = ["/.git/objects", "/node_modules/", "/target/", "/.venv/", "/__pycache__/", "/dist/", "/.next/", "/.mypy_cache/"];
                if s.ends_with(".lock") || NOISY.iter().any(|n| s.contains(n)) {
                    continue;
                }
                if let Some(root) = r2.iter().filter(|r| e.path.starts_with(r)).max_by_key(|r| r.as_os_str().len())
                    && !hit.contains(root)
                {
                    hit.push(root.clone());
                }
            }
            for h in hit {
                let _ = tx.send(h);
            }
        }) else {
            return;
        };
        for r in &roots {
            let _ = deb.watcher().watch(r, RecursiveMode::Recursive);
        }
        self.watcher = Some(deb);
        let rx = Arc::new(Mutex::new(rx));
        cx.spawn(async move |this, cx| {
            loop {
                let rx = rx.clone();
                let Some(path) = cx.background_spawn(async move { rx.lock().ok()?.recv().ok() }).await else { break };
                if this.update(cx, |this, cx| this.on_disk_change(&path, cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
    }

    fn on_disk_change(&mut self, path: &Path, cx: &mut Context<Self>) {
        let active = self.active_path();
        let Some(r) = self.repo_by_path(path) else { return };
        if r.busy.is_some() || r.queued > 0 || r.quiet_until.is_some_and(|t| Instant::now() < t) {
            return;
        }
        if active.as_deref() == Some(path) {
            self.refresh(path, cx);
        } else {
            self.refresh_summary(path, cx);
        }
    }

    /// Every 5 s: slow polling as a safety net, and the background fetch timer.
    fn start_timers(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(5)).await;
                if this.update(cx, |this, cx| this.tick(cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
    }

    fn tick(&mut self, cx: &mut Context<Self>) {
        self.ticks += 1;
        if self.ticks % 6 == 0 {
            if let Some(r) = self.repo()
                && r.busy.is_none()
                && r.queued == 0
            {
                let p = r.path.clone();
                self.refresh(&p, cx);
            }
            let others: Vec<PathBuf> = self.repos.iter().enumerate().filter(|(i, _)| *i != self.active).map(|(_, r)| r.path.clone()).collect();
            if self.ticks % 36 == 0 {
                for p in others {
                    self.refresh_summary(&p, cx);
                }
            }
        }
        let m = self.cfg.fetch_minutes as u64;
        if m > 0 && self.ticks % (m * 12) == 0 {
            self.fetch_all(true, cx);
        }
    }

    // ----- loading -----

    pub fn refresh_active(&mut self, cx: &mut Context<Self>) {
        if let Some(p) = self.active_path() {
            self.refresh(&p, cx);
        }
    }

    pub fn open_repo(&mut self, path: &Path, cx: &mut Context<Self>) {
        self.refresh_then(path, cx, |this, cx| {
            if let Some(r) = this.repo()
                && r.view == View::History
                && r.commits.is_empty()
            {
                let p = r.path.clone();
                this.load_log(&p, true, cx);
            }
        });
    }

    pub fn refresh_summary(&mut self, path: &Path, cx: &mut Context<Self>) {
        let p = path.to_path_buf();
        let p2 = p.clone();
        self.work(cx, move || ops::summary(&p2), move |this, res, cx| {
            if let Some(r) = this.repo_by_path(&p) {
                match res {
                    Ok(s) => {
                        r.summary = Some(s);
                        r.error = None;
                    }
                    Err(e) => r.error = Some(e),
                }
            }
            cx.notify();
        });
    }

    /// Reloads everything the current view of a repo shows. Calls made while one runs are merged.
    pub fn refresh(&mut self, path: &Path, cx: &mut Context<Self>) {
        if !self.inflight.insert(path.to_path_buf()) {
            self.again.insert(path.to_path_buf());
            return;
        }
        self.do_refresh(path.to_path_buf(), cx);
    }

    pub fn refresh_then(&mut self, path: &Path, cx: &mut Context<Self>, then: impl FnOnce(&mut GitTabs, &mut Context<GitTabs>) + 'static) {
        self.after_refresh.entry(path.to_path_buf()).or_default().push(Box::new(then));
        self.refresh(path, cx);
    }

    fn do_refresh(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.again.remove(&path);
        let p2 = path.clone();
        self.work(cx, move || ops::snapshot(&p2), move |this, res, cx| {
            let mut reload_log = false;
            let mut ok = false;
            if let Some(r) = this.repo_by_path(&path) {
                match res {
                    Ok(snap) => {
                        let sig = |r: &RepoState| -> String {
                            r.refs.as_ref().map(|x| x.refs.branches.iter().chain(&x.refs.remotes).map(|b| format!("{}{}", b.sha, b.name)).collect::<String>()).unwrap_or_default()
                        };
                        let (old_head, old_refs) = (r.summary.as_ref().and_then(|s| s.head.clone()), sig(r));
                        r.summary = Some(snap.summary);
                        r.status = snap.status;
                        r.refs = Some(snap.refs);
                        r.error = None;
                        reload_log = !r.commits.is_empty() && (old_head != r.summary.as_ref().and_then(|s| s.head.clone()) || old_refs != sig(r));
                        ok = true;
                    }
                    Err(e) => r.error = Some(e.trim_start_matches("Error: ").to_string()),
                }
            }
            if ok {
                this.prune_seen(&path);
                if reload_log {
                    this.load_log(&path, true, cx);
                }
                let view = this.repos.iter().find(|r| r.path == path).map(|r| r.view);
                match view {
                    Some(View::Reflog) => this.load_reflog(&path, cx),
                    Some(View::Changes) => this.ensure_sel(&path),
                    _ => {}
                }
                if this.active_path().as_deref() == Some(&path) {
                    this.load_diff(cx);
                }
            }
            let hooks = this.after_refresh.remove(&path).unwrap_or_default();
            for h in hooks {
                h(this, cx);
            }
            if this.again.contains(&path) {
                this.do_refresh(path, cx);
            } else {
                this.inflight.remove(&path);
            }
            cx.notify();
        });
    }

    pub fn load_log(&mut self, path: &Path, reset: bool, cx: &mut Context<Self>) {
        let Some(r) = self.repo_by_path(path) else { return };
        if r.log_loading && !reset {
            return;
        }
        r.log_loading = true;
        r.log_seq += 1;
        let seq = r.log_seq;
        let skip = if reset { 0 } else { r.commits.len() };
        let (query, lp) = (r.log_query.clone(), r.log_path.clone());
        let p = path.to_path_buf();
        let p2 = p.clone();
        self.work(cx, move || ops::log(&p2, skip, 400, lp.is_none(), &query, lp.as_deref()), move |this, res, cx| {
            let Some(r) = this.repo_by_path(&p) else { return };
            if r.log_seq != seq {
                return;
            }
            r.log_loading = false;
            match res {
                Ok(page) => {
                    r.log_done = page.len() < 400;
                    if reset {
                        r.commits = Arc::new(page);
                    } else {
                        Arc::make_mut(&mut r.commits).extend(page);
                    }
                    Self::relayout(r, reset);
                    let is_stash = |s: &String| r.refs.iter().flat_map(|x| x.stashes.iter()).any(|x| &x.sha == s);
                    // A stash being looked at is not in the log, and stays selected all the same.
                    if reset && r.sel_commit.as_ref().is_none_or(|s| !r.commits.iter().any(|c| &c.sha == s) && !is_stash(s)) {
                        r.sel_commit = r.commits.first().map(|c| c.sha.clone());
                    }
                    if reset {
                        this.load_detail(&p, cx);
                    }
                }
                Err(e) => this.toast(crate::i18n::t("讀不到歷史"), ToastKind::Err, Some(e), cx),
            }
            cx.notify();
        });
        cx.notify();
    }

    pub fn load_detail(&mut self, path: &Path, cx: &mut Context<Self>) {
        let Some(r) = self.repo_by_path(path) else { return };
        let Some(sha) = r.sel_commit.clone() else {
            r.detail = None;
            return;
        };
        if r.detail.as_ref().is_some_and(|d| d.sha == sha) {
            if self.active_path().as_deref() == Some(path) {
                self.load_diff(cx);
            }
            if self.hist_tab == HistTab::Tree {
                self.load_tree(path, cx);
            }
            return;
        }
        r.detail_seq += 1;
        let seq = r.detail_seq;
        let p = path.to_path_buf();
        let p2 = p.clone();
        let s2 = sha.clone();
        self.work(cx, move || ops::commit_detail(&p2, &s2), move |this, res, cx| {
            let Some(r) = this.repo_by_path(&p) else { return };
            // Holding the arrow key fires many loads; only the latest selection may land.
            if r.detail_seq != seq || r.sel_commit.as_deref() != Some(&sha) {
                return;
            }
            match res {
                Ok(d) => {
                    let want = r.log_path.as_ref().and_then(|lp| d.files.iter().find(|f| &f.path == lp)).map(|f| f.path.clone());
                    r.sel = d.files.first().map(|f| Sel { list: ListKind::C, path: want.unwrap_or_else(|| f.path.clone()) });
                    r.detail = Some(d);
                    if this.active_path().as_deref() == Some(&p) {
                        this.load_diff(cx);
                    }
                    if this.hist_tab == HistTab::Tree {
                        this.load_tree(&p, cx);
                    }
                }
                Err(e) => {
                    r.detail = None;
                    this.toast(crate::i18n::t("讀不到 commit"), ToastKind::Err, Some(e), cx);
                }
            }
            cx.notify();
        });
    }

    pub fn load_reflog(&mut self, path: &Path, cx: &mut Context<Self>) {
        let p = path.to_path_buf();
        let p2 = p.clone();
        self.work(cx, move || ops::reflog(&p2, 300).unwrap_or_default(), move |this, v, cx| {
            if let Some(r) = this.repo_by_path(&p) {
                r.reflog = v;
            }
            cx.notify();
        });
    }

    // ----- selection and the diff -----

    /// Keeps the selected file if it is still listed, follows it between lists, else takes the first.
    pub fn ensure_sel(&mut self, path: &Path) {
        let Some(r) = self.repo_by_path(path) else { return };
        let in_u = |r: &RepoState, p: &str| r.unstaged().iter().any(|f| f.path == p);
        let in_s = |r: &RepoState, p: &str| r.staged().iter().any(|f| f.path == p);
        if let Some(s) = &r.sel
            && ((s.list == ListKind::U && in_u(r, &s.path)) || (s.list == ListKind::S && in_s(r, &s.path)))
        {
            return;
        }
        let path_sel = r.sel.as_ref().map(|s| s.path.clone());
        r.sel = if let Some(p) = path_sel.filter(|p| in_u(r, p)) {
            Some(Sel { list: ListKind::U, path: p })
        } else if let Some(p) = r.sel.as_ref().map(|s| s.path.clone()).filter(|p| in_s(r, p)) {
            Some(Sel { list: ListKind::S, path: p })
        } else {
            Self::ordered_files(r).into_iter().next()
        };
    }

    pub fn ordered_files(r: &RepoState) -> Vec<Sel> {
        use crate::tree::tree_sort;
        match r.view {
            View::History => r.detail.iter().flat_map(|d| d.files.iter()).map(|f| Sel { list: ListKind::C, path: f.path.clone() }).collect(),
            _ => {
                let u: Vec<String> = r.unstaged().iter().map(|f| f.path.clone()).collect();
                let s: Vec<String> = r.staged().iter().map(|f| f.path.clone()).collect();
                tree_sort(&u, |x| x.as_str())
                    .into_iter()
                    .map(|p| Sel { list: ListKind::U, path: p })
                    .chain(tree_sort(&s, |x| x.as_str()).into_iter().map(|p| Sel { list: ListKind::S, path: p }))
                    .collect()
            }
        }
    }

    pub fn select(&mut self, list: ListKind, path: String, cx: &mut Context<Self>) {
        if let Some(r) = self.repo_mut() {
            r.sel = Some(Sel { list, path });
        }
        self.load_diff(cx);
        cx.notify();
    }

    pub fn move_sel(&mut self, d: isize, cx: &mut Context<Self>) {
        let Some(r) = self.repo() else { return };
        let files = Self::ordered_files(r);
        if files.is_empty() {
            return;
        }
        let i = r.sel.as_ref().and_then(|s| files.iter().position(|f| f == s)).map_or(0, |i| i as isize);
        let j = (i + d).clamp(0, files.len() as isize - 1) as usize;
        let f = files[j].clone();
        self.select(f.list, f.path, cx);
    }

    // ----- the File Tree tab -----

    /// Lists the files of the commit shown, unless they are the ones listed already.
    pub fn load_tree(&mut self, path: &Path, cx: &mut Context<Self>) {
        let Some(r) = self.repo_by_path(path) else { return };
        let Some(sha) = r.detail.as_ref().map(|d| d.sha.clone()) else { return };
        if r.tree_of == sha {
            return;
        }
        r.tree_of = sha.clone();
        r.tree_file = r.tree_sel.as_ref().map(|_| FileShown::Loading);
        let (p, p2, s2) = (path.to_path_buf(), path.to_path_buf(), sha.clone());
        self.work(cx, move || ops::tree(&p2, &s2), move |this, res, cx| {
            let Some(r) = this.repo_by_path(&p) else { return };
            // Stepping through commits fires many loads; only the commit still shown may land.
            if r.tree_of != sha {
                return;
            }
            match res {
                Ok(paths) => {
                    r.tree_rows = Arc::new(crate::tree::file_tree_rows(&paths, &r.tree_open));
                    // The file being read stays picked from commit to commit, while the commit has it.
                    if r.tree_sel.as_ref().is_some_and(|s| !paths.contains(s)) {
                        r.tree_sel = None;
                    }
                    r.tree_paths = Arc::new(paths);
                    this.load_tree_file(&p, cx);
                }
                Err(e) => {
                    r.tree_paths = Arc::default();
                    r.tree_rows = Arc::default();
                    r.tree_file = Some(FileShown::Note(e));
                }
            }
            cx.notify();
        });
    }

    pub fn toggle_tree_dir(&mut self, dir: String, cx: &mut Context<Self>) {
        let Some(r) = self.repo_mut() else { return };
        if !r.tree_open.remove(&dir) {
            r.tree_open.insert(dir);
        }
        r.tree_rows = Arc::new(crate::tree::file_tree_rows(&r.tree_paths, &r.tree_open));
        cx.notify();
    }

    pub fn pick_tree_file(&mut self, file: String, cx: &mut Context<Self>) {
        let Some(r) = self.repo_mut() else { return };
        r.tree_sel = Some(file);
        let p = r.path.clone();
        self.load_tree_file(&p, cx);
        cx.notify();
    }

    /// Reads the picked file as it was at the commit shown.
    fn load_tree_file(&mut self, path: &Path, cx: &mut Context<Self>) {
        let Some(r) = self.repo_by_path(path) else { return };
        let (Some(file), sha) = (r.tree_sel.clone(), r.tree_of.clone()) else {
            r.tree_file = None;
            return;
        };
        r.tree_file = Some(FileShown::Loading);
        let (p, p2, s2, f2) = (path.to_path_buf(), path.to_path_buf(), sha.clone(), file.clone());
        self.work(cx, move || ops::file_at(&p2, &s2, &f2), move |this, res, cx| {
            let Some(r) = this.repo_by_path(&p) else { return };
            if r.tree_of != sha || r.tree_sel.as_ref() != Some(&file) {
                return;
            }
            r.tree_file = Some(match res {
                Ok(ops::FileText::Text(t)) => FileShown::Lines(Arc::new(t.lines().map(|l| gpui_kit::SharedString::from(l.replace('\t', "    "))).collect())),
                Ok(ops::FileText::Binary) => FileShown::Note(crate::i18n::t("二進位檔案，沒有文字可以顯示").into()),
                Ok(ops::FileText::Large(n)) => FileShown::Note(crate::i18n::tr("檔案太大（{} MB），不在這裡顯示", &[], &[format!("{:.1}", n as f64 / 1e6)])),
                Err(e) => FileShown::Note(e),
            });
            cx.notify();
        });
    }

    /// Lays the graph out for the commits loaded so far. While no merge is folded only the new
    /// page is laid out, continuing below the rows there are; folding hides commits, and then
    /// the rows that are left are laid out afresh.
    pub fn relayout(r: &mut RepoState, reset: bool) {
        // Let go of the rows first, so that appending to them copies nothing.
        r.graph = None;
        r.shown = None;
        r.unpushed = Arc::default();
        if reset {
            r.layout = Default::default();
        }
        if !r.log_query.is_empty() || r.log_path.is_some() {
            r.layout = Default::default();
            return;
        }
        let pairs: Vec<(&str, &[String])> = r.commits.iter().map(|c| (c.sha.as_str(), c.parents.as_slice())).collect();
        let remote_tips: HashSet<&str> = r.commits.iter().filter(|c| c.refs.iter().any(|x| x.kind == "remote")).map(|c| c.sha.as_str()).collect();
        r.unpushed = Arc::new(crate::graph::unpushed(&pairs, &remote_tips));
        let head = r.commits.iter().find(|c| c.refs.iter().any(|x| matches!(x.kind, "head" | "detached"))).map(|c| c.sha.as_str());
        if r.collapsed.is_empty() {
            let done = r.layout.rows.len();
            r.layout.extend(&pairs[done..], head);
            r.graph = Some(r.layout.rows.clone());
        } else {
            let hide = crate::graph::hidden(&pairs, &r.collapsed);
            let rows = crate::graph::shown(&pairs, &r.collapsed, &hide);
            let left: Vec<(&str, &[String])> = rows.iter().map(|(i, ps)| (pairs[*i].0, ps.as_slice())).collect();
            let mut l = crate::graph::Layout::default();
            l.extend(&left, head);
            r.graph = Some(l.rows);
            r.shown = Some(Arc::new(rows.iter().map(|(i, _)| *i).collect()));
            // Not the rows of every commit any more: unfolding starts over from the top.
            r.layout = Default::default();
        }
    }

    /// Folds away what a merge brought in, or shows it again.
    pub fn toggle_merge(&mut self, sha: String, cx: &mut Context<Self>) {
        let Some(r) = self.repo_mut() else { return };
        if !r.collapsed.remove(&sha) {
            r.collapsed.insert(sha.clone());
        }
        Self::relayout(r, false);
        // A selected commit that just got folded away hands the selection to the merge.
        let gone = r.sel_commit.as_ref().is_some_and(|s| r.commits.iter().position(|c| &c.sha == s).is_some_and(|i| r.shown.as_ref().is_some_and(|v| !v.contains(&i))));
        if gone {
            r.sel_commit = Some(sha);
            let p = r.path.clone();
            self.load_detail(&p, cx);
        }
        cx.notify();
    }

    pub fn move_commit(&mut self, d: isize, cx: &mut Context<Self>) {
        let Some(r) = self.repo_mut() else { return };
        if r.commits.is_empty() {
            return;
        }
        // Up and down move over the rows on screen, which skip what is folded away.
        let rows: Vec<usize> = r.shown.as_ref().map_or_else(|| (0..r.commits.len()).collect(), |v| v.to_vec());
        let at = r.commits.iter().position(|c| Some(&c.sha) == r.sel_commit.as_ref());
        let i = at.and_then(|a| rows.iter().position(|x| *x == a)).map_or(0, |i| i as isize);
        let j = rows[(i + d).clamp(0, rows.len() as isize - 1) as usize];
        if r.sel_commit.as_ref() == Some(&r.commits[j].sha) {
            return;
        }
        r.sel_commit = Some(r.commits[j].sha.clone());
        let p = r.path.clone();
        self.load_detail(&p, cx);
        cx.notify();
    }

    /// Loads the selected file's diff into the shared diff view.
    pub fn load_diff(&mut self, cx: &mut Context<Self>) {
        self.diff_seq += 1;
        let seq = self.diff_seq;
        let Some(r) = self.repos.get_mut(self.active) else { return };
        let Some(sel) = r.sel.clone() else {
            r.diff = None;
            r.diff_for.clear();
            self.diff.update(cx, |d, cx| d.set_files([], cx));
            return;
        };
        let repo = r.path.clone();
        let key = format!("{}:{}", sel.list.tag(), sel.path);
        enum Job {
            Commit { sha: String, old: Option<String> },
            Work { staged: bool, untracked: bool, conflicted: bool },
        }
        let job = match sel.list {
            ListKind::C => {
                let Some(d) = &r.detail else { return };
                let old = d.files.iter().find(|f| f.path == sel.path).and_then(|f| f.old_path.clone());
                Job::Commit { sha: d.sha.clone(), old }
            }
            l => {
                let st = r.status.iter().find(|x| x.path == sel.path);
                Job::Work { staged: l == ListKind::S, untracked: st.is_some_and(|s| s.untracked) && l == ListKind::U, conflicted: st.is_some_and(|s| s.conflicted) && l == ListKind::U }
            }
        };
        // A commit's diff never changes, so revisiting a file is instant.
        let cache_key = if let Job::Commit { sha, .. } = &job { Some(format!("{}|{}|{}", repo.display(), sha, sel.path)) } else { None };
        if let Some(ck) = &cache_key
            && let Some(pos) = self.commit_diffs.iter().position(|(k, _)| k == ck)
        {
            let (k, d) = self.commit_diffs.remove(pos).unwrap();
            self.commit_diffs.push_back((k, d.clone()));
            self.install_diff(&repo, &sel, key, d, cx);
            return;
        }
        let path = sel.path.clone();
        self.work(
            cx,
            move || match job {
                Job::Commit { sha, old } => ops::commit_file_diff(&repo, &sha, &path, old.as_deref()).map(|d| (repo, d)),
                Job::Work { conflicted: true, .. } => Ok((repo.clone(), ops::conflict_view(&repo, &path))),
                Job::Work { staged, untracked, .. } => ops::diff_file(&repo, &path, staged, untracked).map(|d| (repo, d)),
            },
            move |this, res, cx| {
                if this.diff_seq != seq {
                    return;
                }
                match res {
                    Ok((repo, d)) => {
                        if let Some(ck) = cache_key {
                            this.commit_diffs.push_back((ck, d.clone()));
                            if this.commit_diffs.len() > 300 {
                                this.commit_diffs.pop_front();
                            }
                        }
                        if this.repos.get(this.active).and_then(|r| r.sel.as_ref()) == Some(&sel) {
                            this.install_diff(&repo, &sel, key, d, cx);
                        }
                    }
                    Err(e) => this.toast(crate::i18n::t("讀不到 diff"), ToastKind::Err, Some(e), cx),
                }
            },
        );
    }

    fn install_diff(&mut self, repo: &Path, sel: &Sel, key: String, d: FileDiff, cx: &mut Context<Self>) {
        let files = DiffFile::parse(&crate::diffmap::to_patch(&d)).unwrap_or_default();
        // A file marked as reviewed that changed since gets unmarked.
        let hash = diff_hash(&d);
        if let Some(r) = self.repos.iter().find(|r| r.path == repo)
            && let Some(k) = self.seen_key(r, sel.list, &sel.path)
            && self.seen.get(&k).is_some_and(|h| *h != hash)
        {
            self.seen.remove(&k);
            self.persist_seen();
        }
        if let Some(r) = self.repo_mut() {
            r.diff = Some(d);
            r.diff_for = key;
        }
        self.cur_hunk = 0;
        self.stored_sel = None;
        self.diff.update(cx, |s, cx| {
            s.set_files(files, cx);
            s.set_selected_lines(None, cx);
        });
        cx.notify();
    }

    // ----- reviewed ("已看") -----

    pub fn seen_key(&self, r: &RepoState, list: ListKind, path: &str) -> Option<String> {
        match list {
            ListKind::U | ListKind::S => Some(format!("{}|w|{}", r.path.display(), path)),
            ListKind::C => None,
        }
    }

    pub fn is_seen(&self, r: &RepoState, list: ListKind, path: &str) -> bool {
        self.seen_key(r, list, path).is_some_and(|k| self.seen.contains_key(&k))
    }

    /// Drops "reviewed" marks of files that are no longer changed, so the saved map doesn't grow forever.
    fn prune_seen(&mut self, path: &Path) {
        let Some(r) = self.repos.iter().find(|r| r.path == path) else { return };
        let prefix = format!("{}|w|", path.display());
        let live: HashSet<&str> = r.status.iter().map(|f| f.path.as_str()).collect();
        let before = self.seen.len();
        self.seen.retain(|k, _| k.strip_prefix(&prefix).is_none_or(|p| live.contains(p)));
        if self.seen.len() != before {
            self.persist_seen();
        }
    }

    pub fn persist_seen(&self) {
        save_seen(&self.ctx.config_dir, &self.seen);
    }

    pub fn toggle_seen(&mut self, advance: bool, cx: &mut Context<Self>) {
        let Some(r) = self.repo() else { return };
        let (Some(sel), Some(d)) = (r.sel.clone(), r.diff.as_ref()) else { return };
        let Some(k) = self.seen_key(r, sel.list, &sel.path) else { return };
        let hash = diff_hash(d);
        if self.seen.remove(&k).is_none() {
            self.seen.insert(k, hash);
            if advance {
                let next = self.repo().and_then(|r| Self::ordered_files(r).into_iter().find(|f| !self.is_seen(r, f.list, &f.path)));
                match next {
                    Some(n) => self.select(n.list, n.path, cx),
                    None => self.ok(crate::i18n::t("全部檔案都看完了"), cx),
                }
            }
        }
        self.persist_seen();
        cx.notify();
    }

    // ----- search box -----

    fn on_search(&mut self, q: String, cx: &mut Context<Self>) {
        let Some(path) = self.active_path() else { return };
        // Debounce: only the last keystroke of a burst reloads.
        self.search_timer = cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_millis(300)).await;
            let _ = this.update(cx, |this, cx| {
                if let Some(r) = this.repo_by_path(&path)
                    && r.log_query != q
                {
                    r.log_query = q;
                    this.load_log(&path, true, cx);
                }
            });
        });
    }

    // ----- identity -----

    pub fn pin_of(&self, r: &RepoState) -> Option<String> {
        self.cfg.repos.iter().find(|x| Path::new(&x.path) == r.path).and_then(|x| x.pin.clone())
    }

    /// The identity git will use for this repo: a pin, else the first matching rule, else none.
    pub fn identity_of(&self, r: &RepoState) -> IdentityInfo {
        let rule = crate::identity::match_identity(&self.cfg.identities, &r.path.display().to_string(), r.summary.as_ref().and_then(|s| s.remote_url.as_deref())).cloned();
        let pinned = self.pin_of(r).and_then(|pin| self.cfg.identities.iter().find(|i| i.id == pin).cloned());
        IdentityInfo { pinned: pinned.is_some(), id: pinned.or(rule) }
    }

    // ----- tabs and views -----

    pub fn switch_tab(&mut self, i: usize, cx: &mut Context<Self>) {
        if i >= self.repos.len() {
            return;
        }
        self.active = i;
        self.subject.update(cx, |_, _| {});
        self.commit_err.clear();
        self.amend = false;
        self.save_config();
        let p = self.repos[i].path.clone();
        self.search_value(&self.repos[i].log_query.clone(), cx);
        self.load_diff(cx);
        self.open_repo(&p, cx);
        cx.notify();
    }

    /// Shows a repo's own search text in the box without triggering a reload.
    fn search_value(&mut self, _q: &str, _cx: &mut Context<Self>) {}

    pub fn set_view(&mut self, v: View, cx: &mut Context<Self>) {
        let Some(r) = self.repo_mut() else { return };
        r.view = v;
        r.sel = None;
        r.diff = None;
        let p = r.path.clone();
        match v {
            View::History => {
                r.detail = None;
                if r.commits.is_empty() {
                    self.load_log(&p, true, cx);
                } else {
                    self.load_detail(&p, cx);
                }
            }
            View::Reflog => self.load_reflog(&p, cx),
            View::Changes => {
                self.ensure_sel(&p);
                self.load_diff(cx);
            }
        }
        self.diff.update(cx, |d, cx| d.set_files([], cx));
        cx.notify();
    }

    /// Runs a git command that changes the repo without blocking the UI. Stage/unstage calls
    /// queue behind each other so two quick presses never fight over .git/index.lock.
    pub fn quick(
        &mut self,
        path: &Path,
        label: &'static str,
        apply: impl FnOnce(&mut RepoState),
        job: impl FnOnce(&Path) -> Result<String, String> + Send + 'static,
        cx: &mut Context<Self>,
    ) {
        let Some(r) = self.repo_by_path(path) else { return };
        apply(r);
        r.queued += 1;
        let (gate, ticket) = self.take_ticket();
        let p = path.to_path_buf();
        let p2 = p.clone();
        self.work(
            cx,
            move || gated(&gate, ticket, || job(&p2)),
            move |this, res, cx| {
                this.pull_log();
                let last = {
                    let Some(r) = this.repo_by_path(&p) else { return };
                    r.queued = r.queued.saturating_sub(1);
                    r.queued == 0
                };
                if let Err(e) = res {
                    this.toast(crate::i18n::tr("{label}失敗", &[("label", (label).to_string())], &[]), ToastKind::Err, Some(e.trim_start_matches("Error: ").to_string()), cx);
                }
                if last {
                    this.refresh_status(&p, cx);
                }
                cx.notify();
            },
        );
        cx.notify();
    }

    /// Just the file list (one git call), for after stage/unstage where nothing else changed.
    fn refresh_status(&mut self, path: &Path, cx: &mut Context<Self>) {
        let p = path.to_path_buf();
        let p2 = p.clone();
        self.work(cx, move || ops::status(&p2), move |this, res, cx| {
            match res {
                Ok(st) => {
                    if let Some(r) = this.repo_by_path(&p) {
                        let conflicts = st.iter().filter(|f| f.conflicted).count();
                        if let Some(s) = r.summary.as_mut() {
                            s.changes = st.len();
                            s.conflicts = conflicts;
                        }
                        r.status = st;
                        r.quiet_until = Some(Instant::now() + Duration::from_millis(1500));
                    }
                    this.prune_seen(&p);
                    let is_changes = this.repo_by_path(&p).is_some_and(|r| r.view == View::Changes);
                    if is_changes {
                        this.ensure_sel(&p);
                        this.load_diff(cx);
                    }
                }
                Err(_) => this.refresh(&p, cx),
            }
            cx.notify();
        });
    }
}

/// Waits for this ticket's turn, runs `f`, then lets the next ticket go.
pub fn gated<T>(gate: &Gate, ticket: u64, f: impl FnOnce() -> T) -> T {
    let mut turn = gate.turn.lock().unwrap();
    while *turn + 1 != ticket {
        turn = gate.cv.wait(turn).unwrap();
    }
    drop(turn);
    let out = f();
    *gate.turn.lock().unwrap() = ticket;
    gate.cv.notify_all();
    out
}

pub struct IdentityInfo {
    pub id: Option<gitcore::config::Identity>,
    /// Picked for this repo by hand, rather than matched by a rule.
    pub pinned: bool,
}

pub fn resolve_dark(theme: &str, appearance: WindowAppearance) -> bool {
    match theme {
        "dark" => true,
        "light" => false,
        _ => matches!(appearance, WindowAppearance::Dark | WindowAppearance::VibrantDark),
    }
}

/// Short fingerprint of a diff, so "已看" can be cleared when the file changes again.
pub fn diff_hash(d: &FileDiff) -> String {
    let mut h: u32 = 2166136261;
    for hk in &d.hunks {
        for l in &hk.lines {
            for b in std::iter::once(l.t as u8).chain(l.text.bytes()).chain(std::iter::once(10)) {
                h = (h ^ b as u32).wrapping_mul(16777619);
            }
        }
    }
    format!("{h:x}")
}

#[derive(Clone, Copy)]
pub struct Sizes {
    pub side_bar: f32,
    pub files: f32,
    pub hist_top: f32,
    pub hist_files: f32,
}

pub struct Prefs {
    pub side: bool,
    pub wrap: bool,
    pub hist_tab: String,
    pub lang: String,
    pub sizes: Sizes,
}

/// The window's restored (not maximized) rectangle, `[x, y, width, height]`, and whether it is maximized.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct WindowPlace {
    pub rect: [f32; 4],
    pub maximized: bool,
}

pub fn load_window(dir: &Path) -> Option<WindowPlace> {
    let v: serde_json::Value = std::fs::read_to_string(dir.join("prefs.json")).ok().and_then(|s| serde_json::from_str(&s).ok())?;
    let r: Vec<f32> = v["window"].as_array()?.iter().filter_map(|x| x.as_f64()).map(|x| x as f32).collect();
    Some(WindowPlace { rect: r.try_into().ok()?, maximized: v["maximized"].as_bool().unwrap_or(false) })
}

/// The size to open with when nothing fits better: the usual one, but never larger than the screen.
pub fn default_window_size(screen: Option<(f32, f32)>) -> (f32, f32) {
    match screen {
        Some((w, h)) => ((w - 40.).clamp(640., 1400.), (h - 90.).clamp(480., 900.)),
        None => (1400., 900.),
    }
}

/// Whether a saved window can be opened as it was: big enough to use, and with the middle of its
/// title bar on one of the screens (a screen may have been unplugged since).
pub fn window_fits(place: &WindowPlace, screens: &[[f32; 4]]) -> bool {
    let [x, y, w, h] = place.rect;
    let (gx, gy) = (x + w / 2., y + 12.);
    w >= 480. && h >= 360. && screens.iter().any(|[sx, sy, sw, sh]| gx >= *sx && gx < sx + sw && gy >= *sy && gy < sy + sh)
}

/// The bounds to open the window with.
pub fn initial_window(saved: Option<WindowPlace>, cx: &gpui_kit::App) -> gpui_kit::WindowBounds {
    use gpui_kit::{Bounds, WindowBounds, point, px, size};
    let screens: Vec<[f32; 4]> = cx.displays().iter().map(|d| d.bounds()).map(|b| [b.origin.x.into(), b.origin.y.into(), b.size.width.into(), b.size.height.into()]).collect();
    if let Some(p) = saved.filter(|p| window_fits(p, &screens)) {
        let [x, y, w, h] = p.rect;
        let b = Bounds::new(point(px(x), px(y)), size(px(w), px(h)));
        return if p.maximized { WindowBounds::Maximized(b) } else { WindowBounds::Windowed(b) };
    }
    let screen = cx.primary_display().map(|d| d.bounds().size).map(|s| (f32::from(s.width), f32::from(s.height)));
    let (w, h) = default_window_size(screen);
    WindowBounds::centered(size(px(w), px(h)), cx)
}

fn load_prefs(dir: &Path) -> Prefs {
    let v: serde_json::Value = std::fs::read_to_string(dir.join("prefs.json")).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
    let z = |k: &str, d: f32| v["sizes"][k].as_f64().map(|x| x as f32).unwrap_or(d);
    Prefs {
        side: v["side"].as_bool().unwrap_or(false),
        wrap: v["wrap"].as_bool().unwrap_or(false),
        hist_tab: v["hist_tab"].as_str().unwrap_or("commit").to_string(),
        lang: v["lang"].as_str().unwrap_or("auto").to_string(),
        sizes: Sizes { side_bar: z("side_bar", 220.), files: z("files", 300.), hist_top: z("hist_top", 340.), hist_files: z("hist_files", 300.) },
    }
}

fn load_seen(dir: &Path) -> HashMap<String, String> {
    std::fs::read_to_string(dir.join("seen.json")).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

fn save_seen(dir: &Path, m: &HashMap<String, String>) {
    let _ = std::fs::create_dir_all(dir);
    if let Ok(s) = serde_json::to_string(m) {
        let _ = std::fs::write(dir.join("seen.json"), s);
    }
}

impl Render for GitTabs {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl gpui_kit::IntoElement {
        crate::views::root(self, window, cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_window_is_never_larger_than_the_screen() {
        assert_eq!(default_window_size(Some((2560., 1440.))), (1400., 900.));
        assert_eq!(default_window_size(Some((1024., 768.))), (984., 678.));
        assert_eq!(default_window_size(None), (1400., 900.));
    }

    #[test]
    fn a_saved_window_is_reused_only_while_its_screen_is_there() {
        let (one, two) = ([0., 0., 1920., 1080.], [1920., 0., 1920., 1080.]);
        let on_second = WindowPlace { rect: [2000., 100., 1200., 800.], maximized: false };
        assert!(window_fits(&on_second, &[one, two]));
        assert!(!window_fits(&on_second, &[one]), "the second screen was unplugged");
        assert!(!window_fits(&WindowPlace { rect: [10., 10., 200., 100.], maximized: false }, &[one]), "too small to use");
    }
}
