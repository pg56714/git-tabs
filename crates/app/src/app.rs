//! The window: repo tabs, the changes view (file lists + diff + commit box) and the remotes bar.
//!
//! Git work always runs on the background executor through gitcore's typed `ops::*`; results
//! come back to the UI thread through [`GitTabs::work`].

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;

use gitcore::config::{self, AppConfig, RepoEntry};
use gitcore::ops::{self, LineAction, Summary};
use gitcore::parse::{FileDiff, StatusEntry};
use gitcore::Ctx;
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, IconName, Selectable as _, Sizable as _, StyledExt as _,
    button::{Button, ButtonVariants as _},
    diff::{Diff, DiffFile, DiffMode, DiffState},
    h_flex,
    input::{Textarea, TextareaState},
    list::ListItem,
    resizable::{h_resizable, resizable_panel},
    status_bar::StatusBar,
    v_flex,
};
use gpui_kit::{
    AnyElement, AnyWindowHandle, App, AppContext as _, Context, Entity, InteractiveElement as _,
    IntoElement, KeyBinding, ParentElement as _, PathPromptOptions, Render, SharedString,
    StatefulInteractiveElement as _, Styled as _, Window, div, prelude::FluentBuilder as _, px,
    rems,
};
use notify_debouncer_mini::{
    DebounceEventResult, Debouncer, new_debouncer, notify::RecommendedWatcher, notify::RecursiveMode,
};

mod actions {
    gpui_kit::actions!(git_tabs, [Refresh, StageSelection, Commit]);
}
use actions::*;

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("f5", Refresh, Some("GitTabs")),
        KeyBinding::new("ctrl-r", Refresh, Some("GitTabs")),
        KeyBinding::new("ctrl-enter", Commit, Some("GitTabs")),
        KeyBinding::new("s", StageSelection, Some("GitTabs")),
    ]);
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum List {
    Unstaged,
    Staged,
}

struct Repo {
    path: PathBuf,
    name: String,
    summary: Option<Summary>,
    status: Vec<StatusEntry>,
    selected: Option<(List, String)>,
    diff: Option<FileDiff>,
    error: Option<String>,
    rev: usize,
}

impl Repo {
    fn new(path: PathBuf) -> Self {
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.display().to_string());
        Self { path, name, summary: None, status: Vec::new(), selected: None, diff: None, error: None, rev: 0 }
    }

    fn unstaged(&self) -> Vec<&StatusEntry> {
        self.status.iter().filter(|f| f.untracked || f.conflicted || f.y != '.').collect()
    }

    fn staged(&self) -> Vec<&StatusEntry> {
        self.status.iter().filter(|f| !f.untracked && !f.conflicted && f.x != '.').collect()
    }

    /// Keeps the selection if the file is still in its list, otherwise picks the first file.
    fn fix_selection(&mut self) {
        let keep = match &self.selected {
            Some((List::Unstaged, p)) => self.unstaged().iter().any(|f| &f.path == p),
            Some((List::Staged, p)) => self.staged().iter().any(|f| &f.path == p),
            None => false,
        };
        if !keep {
            self.selected = self
                .unstaged()
                .first()
                .map(|f| (List::Unstaged, f.path.clone()))
                .or_else(|| self.staged().first().map(|f| (List::Staged, f.path.clone())));
        }
    }
}

pub struct GitTabs {
    ctx: Arc<Ctx>,
    cfg: AppConfig,
    repos: Vec<Repo>,
    active: usize,
    diff: Entity<DiffState>,
    message: Entity<TextareaState>,
    window: AnyWindowHandle,
    busy: bool,
    note: Option<(String, bool)>,
    diff_rev: usize,
    watcher: Option<Debouncer<RecommendedWatcher>>,
}

impl GitTabs {
    pub fn new(ctx: Arc<Ctx>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        window.set_window_title("git-tabs");
        let cfg = config::load(&ctx.config_dir);
        let repos: Vec<Repo> =
            cfg.repos.iter().filter(|r| Path::new(&r.path).is_dir()).map(|r| Repo::new(PathBuf::from(&r.path))).collect();
        let active = cfg.active.min(repos.len().saturating_sub(1));
        let diff = cx.new(|cx| DiffState::new([], cx).with_mode(DiffMode::Split));
        let message = cx.new(|cx| TextareaState::new(window, cx).placeholder("Commit 訊息（Ctrl+Enter 送出）"));
        let mut this = Self {
            ctx,
            cfg,
            repos,
            active,
            diff,
            message,
            window: window.window_handle(),
            busy: false,
            note: None,
            diff_rev: 0,
            watcher: None,
        };
        this.watch(cx);
        for i in 0..this.repos.len() {
            this.refresh_at(i, cx);
        }
        this
    }

    // ----- plumbing -----

    /// Runs `job` off the UI thread, then `done` on it.
    fn work<T: Send + 'static>(
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

    fn say(&mut self, text: impl Into<String>, ok: bool, cx: &mut Context<Self>) {
        self.note = Some((text.into(), ok));
        cx.notify();
    }

    fn save_config(&mut self) {
        self.cfg.repos = self.repos.iter().map(|r| RepoEntry { path: r.path.display().to_string(), pin: self.cfg.repos.iter().find(|e| Path::new(&e.path) == r.path).and_then(|e| e.pin.clone()) }).collect();
        self.cfg.active = self.active;
        let _ = config::save(&self.ctx.config_dir, &self.cfg);
    }

    /// Watches every repo and refreshes the one that changed on disk.
    fn watch(&mut self, cx: &mut Context<Self>) {
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
        // Our own commands change files too; the busy op refreshes when it ends.
        if self.busy {
            return;
        }
        if let Some(i) = self.repos.iter().position(|r| r.path == path) {
            self.refresh_at(i, cx);
        }
    }

    // ----- loading -----

    fn refresh_at(&mut self, i: usize, cx: &mut Context<Self>) {
        let Some(r) = self.repos.get_mut(i) else { return };
        r.rev += 1;
        let (rev, path) = (r.rev, r.path.clone());
        self.work(
            cx,
            move || ops::summary(&path).and_then(|s| ops::status(&path).map(|st| (s, st))),
            move |this, res, cx| {
                let Some(r) = this.repos.get_mut(i) else { return };
                if r.rev != rev {
                    return;
                }
                match res {
                    Ok((s, st)) => {
                        r.summary = Some(s);
                        r.status = st;
                        r.error = None;
                        r.fix_selection();
                    }
                    Err(e) => r.error = Some(e),
                }
                if i == this.active {
                    this.reload_diff(cx);
                }
                cx.notify();
            },
        );
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        self.refresh_at(self.active, cx);
    }

    fn reload_diff(&mut self, cx: &mut Context<Self>) {
        self.diff_rev += 1;
        let rev = self.diff_rev;
        let Some(r) = self.repos.get_mut(self.active) else { return };
        let Some((list, file)) = r.selected.clone() else {
            r.diff = None;
            self.diff.update(cx, |d, cx| d.set_files([], cx));
            return;
        };
        let untracked = r.status.iter().any(|f| f.untracked && f.path == file);
        let path = r.path.clone();
        self.work(
            cx,
            move || ops::diff_file(&path, &file, list == List::Staged, untracked),
            move |this, res, cx| {
                if this.diff_rev != rev {
                    return;
                }
                let files = match &res {
                    Ok(fd) => DiffFile::parse(&crate::diffmap::to_patch(fd)).unwrap_or_default(),
                    Err(_) => Vec::new(),
                };
                if let Some(r) = this.repos.get_mut(this.active) {
                    match res {
                        Ok(fd) => r.diff = Some(fd),
                        Err(e) => r.error = Some(e),
                    }
                }
                this.diff.update(cx, |d, cx| d.set_files(files, cx));
                cx.notify();
            },
        );
    }

    fn select(&mut self, list: List, path: String, cx: &mut Context<Self>) {
        if let Some(r) = self.repos.get_mut(self.active) {
            r.selected = Some((list, path));
        }
        self.reload_diff(cx);
        cx.notify();
    }

    // ----- tabs -----

    fn switch_tab(&mut self, i: usize, cx: &mut Context<Self>) {
        if i < self.repos.len() && i != self.active {
            self.active = i;
            self.save_config();
            self.reload_diff(cx);
            self.refresh(cx);
            cx.notify();
        }
    }

    fn close_tab(&mut self, i: usize, cx: &mut Context<Self>) {
        if i >= self.repos.len() {
            return;
        }
        self.repos.remove(i);
        self.active = self.active.min(self.repos.len().saturating_sub(1));
        self.save_config();
        self.watch(cx);
        self.reload_diff(cx);
        cx.notify();
    }

    fn add_repo(&mut self, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions { files: false, directories: true, multiple: true, prompt: Some("選擇 git 資料夾".into()) });
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = rx.await else { return };
            let _ = this.update(cx, |this, cx| this.add_paths(paths, cx));
        })
        .detach();
    }

    fn add_paths(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        let mut added = 0;
        for p in paths {
            match ops::toplevel(&p) {
                Ok(top) => {
                    if let Some(i) = self.repos.iter().position(|r| r.path == top) {
                        self.active = i;
                    } else {
                        self.repos.push(Repo::new(top));
                        self.active = self.repos.len() - 1;
                        added += 1;
                    }
                }
                Err(e) => {
                    self.say(format!("不是 git 資料夾：{e}"), false, cx);
                }
            }
        }
        self.save_config();
        self.watch(cx);
        self.refresh(cx);
        if added > 0 {
            self.say(format!("已加入 {added} 個 repo"), true, cx);
        }
        cx.notify();
    }

    // ----- git operations -----

    /// Runs one mutating git command, then refreshes. One at a time, so commands never overlap.
    fn mutate(
        &mut self,
        label: &'static str,
        cx: &mut Context<Self>,
        job: impl FnOnce(&Path) -> Result<String, String> + Send + 'static,
    ) {
        if self.busy {
            return;
        }
        let Some(r) = self.repos.get(self.active) else { return };
        let path = r.path.clone();
        self.busy = true;
        self.say(format!("{label}…"), true, cx);
        self.work(cx, move || job(&path), move |this, res, cx| {
            this.busy = false;
            match res {
                Ok(out) => {
                    let first = out.lines().next().unwrap_or("").to_string();
                    this.say(if first.is_empty() { format!("{label} 完成") } else { format!("{label}：{first}") }, true, cx);
                }
                Err(e) => this.say(format!("{label} 失敗：{e}"), false, cx),
            }
            this.refresh(cx);
        });
    }

    fn stage_file(&mut self, list: List, path: String, cx: &mut Context<Self>) {
        match list {
            List::Unstaged => self.mutate("暫存", cx, move |repo| ops::stage(repo, &[path]).map(|_| String::new())),
            List::Staged => self.mutate("取消暫存", cx, move |repo| ops::unstage(repo, &[path]).map(|_| String::new())),
        }
    }

    fn stage_all(&mut self, cx: &mut Context<Self>) {
        let Some(r) = self.repos.get(self.active) else { return };
        let paths: Vec<String> = r.unstaged().iter().map(|f| f.path.clone()).collect();
        if !paths.is_empty() {
            self.mutate("全部暫存", cx, move |repo| ops::stage(repo, &paths).map(|_| String::new()));
        }
    }

    /// Stages (or unstages, when viewing the staged side) just the lines selected in the diff.
    fn stage_selection(&mut self, cx: &mut Context<Self>) {
        let Some(r) = self.repos.get(self.active) else { return };
        let (Some((list, file)), Some(fd)) = (r.selected.clone(), r.diff.clone()) else { return };
        let Some(range) = self.diff.read(cx).selected_lines() else {
            self.say("先在差異中點選或拖曳要處理的行", false, cx);
            return;
        };
        let pairs = crate::diffmap::selected_pairs(&fd, (range.side(), range.start()), (range.end_side(), range.end()));
        if pairs.is_empty() {
            self.say("選取的範圍沒有異動的行", false, cx);
            return;
        }
        let (label, action) = match list {
            List::Unstaged => ("暫存選取行", LineAction::Stage),
            List::Staged => ("取消暫存選取行", LineAction::Unstage),
        };
        self.mutate(label, cx, move |repo| ops::apply_lines(repo, &file, action, &pairs).map(|_| String::new()));
    }

    fn commit(&mut self, cx: &mut Context<Self>) {
        let msg = self.message.read(cx).value().to_string();
        if msg.trim().is_empty() {
            self.say("請先輸入 commit 訊息", false, cx);
            return;
        }
        if self.repos.get(self.active).is_none_or(|r| r.staged().is_empty()) {
            self.say("沒有已暫存的檔案", false, cx);
            return;
        }
        let window = self.window;
        let message = self.message.clone();
        self.mutate("Commit", cx, move |repo| ops::commit(repo, &msg, false).map(|sha| sha.chars().take(8).collect()));
        // Clear the box once the commit went through (the busy flag flips back when it ends).
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_millis(150)).await;
                let Ok(done) = this.update(cx, |this, _| !this.busy) else { return };
                if done {
                    let ok = this.update(cx, |this, _| this.note.as_ref().is_some_and(|(t, ok)| *ok && t.starts_with("Commit："))).unwrap_or(false);
                    if ok {
                        let _ = cx.update_window(window, |_, window, cx| message.update(cx, |m, cx| m.set_value("", window, cx)));
                    }
                    return;
                }
            }
        })
        .detach();
    }

    fn remote(&mut self, which: &'static str, cx: &mut Context<Self>) {
        match which {
            "fetch" => self.mutate("Fetch", cx, |repo| ops::fetch(repo)),
            "pull" => self.mutate("Pull", cx, |repo| ops::pull(repo, "ff")),
            _ => self.mutate("Push", cx, |repo| ops::push(repo, false)),
        }
    }

    // ----- rendering -----

    fn render_tabs(&self, cx: &mut Context<Self>) -> AnyElement {
        h_flex()
            .flex_none()
            .h_10()
            .px_2()
            .gap_1()
            .items_center()
            .border_b_1()
            .border_color(cx.theme().border)
            .children(self.repos.iter().enumerate().map(|(i, r)| {
                let dirty = r.summary.as_ref().is_some_and(|s| s.changes > 0);
                let label = match r.summary.as_ref().and_then(|s| s.branch.clone()) {
                    Some(b) => format!("{}  ·  {}{}", r.name, b, if dirty { " ●" } else { "" }),
                    None => r.name.clone(),
                };
                h_flex()
                    .gap_0p5()
                    .child(
                        Button::new(SharedString::from(format!("tab-{i}")))
                            .small()
                            .ghost()
                            .selected(i == self.active)
                            .label(label)
                            .on_click(cx.listener(move |this, _, _, cx| this.switch_tab(i, cx))),
                    )
                    .child(
                        Button::new(SharedString::from(format!("close-{i}")))
                            .xsmall()
                            .ghost()
                            .icon(IconName::Close)
                            .on_click(cx.listener(move |this, _, _, cx| this.close_tab(i, cx))),
                    )
            }))
            .child(Button::new("add-repo").small().ghost().icon(IconName::Plus).tooltip("加入 repo").on_click(cx.listener(|this, _, _, cx| this.add_repo(cx))))
            .into_any_element()
    }

    fn render_toolbar(&self, cx: &mut Context<Self>) -> AnyElement {
        let s = self.repos.get(self.active).and_then(|r| r.summary.clone());
        let busy = self.busy;
        h_flex()
            .flex_none()
            .h_10()
            .px_3()
            .gap_2()
            .items_center()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(div().font_medium().child(s.as_ref().and_then(|s| s.branch.clone()).unwrap_or_else(|| "（無分支）".into())))
            .when_some(s.as_ref().filter(|s| s.ahead > 0 || s.behind > 0), |this, s| {
                this.child(div().text_xs().text_color(cx.theme().muted_foreground).child(format!("↑{} ↓{}", s.ahead, s.behind)))
            })
            .when_some(s.as_ref().and_then(|s| s.state), |this, st| this.child(div().text_xs().text_color(cx.theme().warning).child(st)))
            .child(div().flex_1())
            .child(Button::new("refresh").small().ghost().label("重新整理").on_click(cx.listener(|this, _, _, cx| this.refresh(cx))))
            .child(Button::new("fetch").small().outline().label("Fetch").disabled(busy).on_click(cx.listener(|this, _, _, cx| this.remote("fetch", cx))))
            .child(Button::new("pull").small().outline().label("Pull").disabled(busy).on_click(cx.listener(|this, _, _, cx| this.remote("pull", cx))))
            .child(Button::new("push").small().outline().label("Push").disabled(busy).on_click(cx.listener(|this, _, _, cx| this.remote("push", cx))))
            .into_any_element()
    }

    fn file_row(&self, list: List, f: &StatusEntry, cx: &mut Context<Self>) -> AnyElement {
        let selected = self.repos.get(self.active).and_then(|r| r.selected.as_ref()) == Some(&(list, f.path.clone()));
        let code = if f.untracked {
            '?'
        } else if f.conflicted {
            '!'
        } else if list == List::Staged {
            f.x
        } else {
            f.y
        };
        let (path, path2) = (f.path.clone(), f.path.clone());
        let (name, dir) = match f.path.rsplit_once('/') {
            Some((d, n)) => (n.to_string(), d.to_string()),
            None => (f.path.clone(), String::new()),
        };
        let id = SharedString::from(format!("{}:{}", if list == List::Staged { "s" } else { "u" }, f.path));
        ListItem::new(id.clone())
            .selected(selected)
            .px_2()
            .w_full()
            .on_click(cx.listener(move |this, _, _, cx| this.select(list, path.clone(), cx)))
            .child(
                h_flex()
                    .w_full()
                    .gap_2()
                    .items_center()
                    .child(div().w(px(14.)).text_color(cx.theme().muted_foreground).font_family(cx.theme().mono_font_family.clone()).child(code.to_string()))
                    .child(div().flex_1().min_w_0().truncate().child(name).when(!dir.is_empty(), |t| t.child(div().ml_2().text_xs().text_color(cx.theme().muted_foreground).child(dir))))
                    .child(
                        Button::new(SharedString::from(format!("act-{id}")))
                            .xsmall()
                            .ghost()
                            .icon(if list == List::Staged { IconName::Minus } else { IconName::Plus })
                            .tooltip(if list == List::Staged { "取消暫存" } else { "暫存" })
                            .on_click(cx.listener(move |this, _, _, cx| this.stage_file(list, path2.clone(), cx))),
                    ),
            )
            .into_any_element()
    }

    fn render_files(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(r) = self.repos.get(self.active) else { return div().into_any_element() };
        let (unstaged, staged) = (r.unstaged(), r.staged());
        let head = |title: String, extra: Option<AnyElement>, cx: &mut Context<Self>| {
            h_flex()
                .h_8()
                .px_2()
                .gap_2()
                .items_center()
                .flex_none()
                .bg(cx.theme().muted)
                .child(div().text_xs().font_medium().child(title))
                .child(div().flex_1())
                .when_some(extra, |t, e| t.child(e))
        };
        v_flex()
            .size_full()
            .min_h_0()
            .child(head(format!("未暫存 ({})", unstaged.len()), Some(Button::new("stage-all").xsmall().ghost().label("全部暫存").on_click(cx.listener(|this, _, _, cx| this.stage_all(cx))).into_any_element()), cx))
            .child(div().id("unstaged").flex_1().min_h_0().overflow_y_scroll().children(unstaged.iter().map(|f| self.file_row(List::Unstaged, f, cx))))
            .child(head(format!("已暫存 ({})", staged.len()), None, cx))
            .child(div().id("staged").flex_1().min_h_0().overflow_y_scroll().children(staged.iter().map(|f| self.file_row(List::Staged, f, cx))))
            .child(
                v_flex()
                    .flex_none()
                    .p_2()
                    .gap_2()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .child(Textarea::new(&self.message).h(px(84.)))
                    .child(
                        Button::new("commit")
                            .primary()
                            .small()
                            .label(format!("Commit（{}）", staged.len()))
                            .disabled(self.busy || staged.is_empty())
                            .on_click(cx.listener(|this, _, _, cx| this.commit(cx))),
                    ),
            )
            .into_any_element()
    }

    fn render_diff(&self, cx: &mut Context<Self>) -> AnyElement {
        let selected = self.repos.get(self.active).and_then(|r| r.selected.clone());
        let has_sel = self.diff.read(cx).selected_lines().is_some();
        v_flex()
            .size_full()
            .min_w_0()
            .min_h_0()
            .child(
                h_flex()
                    .flex_none()
                    .h_9()
                    .px_3()
                    .gap_2()
                    .items_center()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(div().flex_1().min_w_0().truncate().text_sm().child(selected.as_ref().map(|(_, p)| p.clone()).unwrap_or_else(|| "（沒有選取檔案）".into())))
                    .when_some(selected.as_ref(), |t, (list, _)| {
                        t.child(
                            Button::new("stage-lines")
                                .xsmall()
                                .outline()
                                .label(if *list == List::Unstaged { "暫存選取行 (S)" } else { "取消暫存選取行 (S)" })
                                .disabled(self.busy || !has_sel)
                                .on_click(cx.listener(|this, _, _, cx| this.stage_selection(cx))),
                        )
                    })
                    .child(Button::new("mode-split").xsmall().ghost().label("並排").on_click(cx.listener(|this, _, _, cx| this.diff.update(cx, |d, cx| d.set_mode(DiffMode::Split, cx)))))
                    .child(Button::new("mode-unified").xsmall().ghost().label("合併").on_click(cx.listener(|this, _, _, cx| this.diff.update(cx, |d, cx| d.set_mode(DiffMode::Unified, cx))))),
            )
            .child(div().flex_1().min_h_0().child(Diff::new(&self.diff).line_number(true).soft_wrap(false)))
            .into_any_element()
    }
}

impl Render for GitTabs {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let body: AnyElement = if self.repos.is_empty() {
            v_flex()
                .size_full()
                .items_center()
                .justify_center()
                .gap_3()
                .child(div().text_lg().child("git-tabs"))
                .child(div().text_color(cx.theme().muted_foreground).child("加入一個 repo 開始"))
                .child(Button::new("welcome-add").primary().label("加入 repo").on_click(cx.listener(|this, _, _, cx| this.add_repo(cx))))
                .into_any_element()
        } else {
            div()
                .flex_1()
                .min_h_0()
                .child(
                    h_resizable("panes")
                        .child(resizable_panel().size(px(360.)).size_range(px(260.)..px(640.)).child(self.render_files(cx)))
                        .child(resizable_panel().child(self.render_diff(cx))),
                )
                .into_any_element()
        };
        let err = self.repos.get(self.active).and_then(|r| r.error.clone());
        v_flex()
            .id("git-tabs")
            .key_context("GitTabs")
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .on_action(cx.listener(|this, _: &Refresh, _, cx| this.refresh(cx)))
            .on_action(cx.listener(|this, _: &Commit, _, cx| this.commit(cx)))
            .on_action(cx.listener(|this, _: &StageSelection, _, cx| this.stage_selection(cx)))
            .child(self.render_tabs(cx))
            .when(!self.repos.is_empty(), |t| t.child(self.render_toolbar(cx)))
            .child(body)
            .child(StatusBar::new().left(div().text_xs().text_color(match (&err, &self.note) {
                (Some(_), _) | (_, Some((_, false))) => cx.theme().danger,
                _ => cx.theme().muted_foreground,
            }).child(err.or_else(|| self.note.as_ref().map(|(t, _)| t.clone())).unwrap_or_default())))
    }
}
