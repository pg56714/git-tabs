//! Menus, dialogs and the command palette: the state side (rendering is in views/overlay.rs).

use std::collections::HashMap;
use std::rc::Rc;

use gpui_kit::component::input::{InputEvent, InputState, TextareaState};
use gpui_kit::{AppContext as _, Context, Pixels, Point, Window};

use crate::app::GitTabs;
use crate::format::{fuzzy, short};
use crate::identity::tag_of;
use crate::model::*;

impl GitTabs {
    // ----- menus -----

    pub fn open_menu(&mut self, pos: Point<Pixels>, items: Vec<MenuItem>, right: bool, cx: &mut Context<Self>) {
        self.menu = Some(MenuState { pos, items, right });
        cx.notify();
    }

    pub fn close_menu(&mut self, cx: &mut Context<Self>) {
        if self.menu.take().is_some() {
            cx.notify();
        }
    }

    // ----- dialogs -----

    pub fn close_modal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.modal = None;
        self.nav_focus.focus(window, cx);
        cx.notify();
    }

    pub fn confirm(
        &mut self,
        title: &str,
        body: String,
        ok: &str,
        danger: bool,
        cx: &mut Context<Self>,
        on_ok: impl Fn(&mut GitTabs, &mut Window, &mut Context<GitTabs>) + 'static,
    ) {
        self.menu = None;
        self.modal = Some(Modal::Confirm(ConfirmModal { title: title.into(), body, ok: ok.into(), danger, on_ok: Rc::new(on_ok) }));
        cx.notify();
    }

    pub fn choose(
        &mut self,
        title: &str,
        body: &str,
        choices: Vec<Choice>,
        cx: &mut Context<Self>,
        on_pick: impl Fn(&mut GitTabs, String, &mut Window, &mut Context<GitTabs>) + 'static,
    ) {
        self.menu = None;
        self.modal = Some(Modal::Choice(ChoiceModal { title: title.into(), body: body.into(), choices, on_pick: Rc::new(on_pick) }));
        cx.notify();
    }

    pub fn prompt(
        &mut self,
        title: &str,
        fields: Vec<FieldSpec>,
        ok: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
        on_ok: impl Fn(&mut GitTabs, HashMap<String, String>, &mut Window, &mut Context<GitTabs>) + 'static,
    ) {
        self.menu = None;
        let mut subs = Vec::new();
        let mut out = Vec::new();
        for spec in fields {
            let (mut input, mut textarea) = (None, None);
            match spec.kind {
                FieldKind::Text => {
                    let (ph, val) = (spec.placeholder.clone(), spec.value.clone());
                    let st = cx.new(|cx| {
                        let mut s = InputState::new(window, cx).placeholder(ph);
                        s.set_value(val, window, cx);
                        s
                    });
                    subs.push(cx.subscribe_in(&st, window, |this, _, ev: &InputEvent, window, cx| {
                        if matches!(ev, InputEvent::PressEnter { .. }) {
                            this.submit_prompt(window, cx);
                        }
                    }));
                    input = Some(st);
                }
                FieldKind::Textarea => {
                    let (ph, val) = (spec.placeholder.clone(), spec.value.clone());
                    textarea = Some(cx.new(|cx| {
                        let mut s = TextareaState::new(window, cx).placeholder(ph);
                        s.set_value(val, window, cx);
                        s
                    }));
                }
                FieldKind::Checkbox => {}
            }
            let checked = spec.kind == FieldKind::Checkbox && spec.value == "1";
            out.push(PromptField { spec, input, textarea, checked, err: None });
        }
        if let Some(first) = out.iter().find_map(|f| f.input.clone()) {
            first.update(cx, |s, cx| s.focus(window, cx));
        }
        self.modal = Some(Modal::Prompt(PromptModal { title: title.into(), ok: ok.into(), fields: out, on_ok: Rc::new(on_ok), _subs: subs }));
        cx.notify();
    }

    pub fn submit_prompt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(Modal::Prompt(m)) = &mut self.modal else { return };
        let mut vals = HashMap::new();
        let mut bad = false;
        for f in m.fields.iter_mut() {
            let v = match f.spec.kind {
                FieldKind::Text => f.input.as_ref().map(|i| i.read(cx).value().to_string()).unwrap_or_default(),
                FieldKind::Textarea => f.textarea.as_ref().map(|i| i.read(cx).value().to_string()).unwrap_or_default(),
                FieldKind::Checkbox => if f.checked { "1".into() } else { String::new() },
            };
            f.err = f.spec.check.and_then(|c| c(&v));
            bad |= f.err.is_some();
            vals.insert(f.spec.key.to_string(), v);
        }
        if bad {
            cx.notify();
            return;
        }
        let on_ok = m.on_ok.clone();
        self.close_modal(window, cx);
        on_ok(self, vals, window, cx);
    }

    // ----- command palette -----

    pub fn open_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.menu = None;
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("輸入指令、repo、分支或 commit…（例如：push my-repo）"));
        input.update(cx, |s, cx| s.focus(window, cx));
        let sub = cx.subscribe_in(&input, window, |this, s, ev: &InputEvent, window, cx| match ev {
            InputEvent::Change => {
                let q = s.read(cx).value().to_string();
                if let Some(p) = &mut this.palette {
                    p.query = q;
                    p.sel = 0;
                }
                cx.notify();
            }
            InputEvent::PressEnter { .. } => this.run_palette(window, cx),
            _ => {}
        });
        self.palette = Some(Palette { input, sel: 0, query: String::new(), _sub: sub });
        cx.notify();
    }

    pub fn close_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.palette.take().is_some() {
            self.nav_focus.focus(window, cx);
            cx.notify();
        }
    }

    pub fn palette_move(&mut self, d: isize, cx: &mut Context<Self>) {
        let n = self.palette_shown().len() as isize;
        if let Some(p) = &mut self.palette {
            p.sel = (p.sel as isize + d).clamp(0, (n - 1).max(0)) as usize;
        }
        cx.notify();
    }

    pub fn run_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(sel) = self.palette.as_ref().map(|p| p.sel) else { return };
        let item = self.palette_shown().into_iter().nth(sel);
        self.close_palette(window, cx);
        if let Some(it) = item {
            (it.run)(self, window, cx);
        }
    }

    pub fn palette_shown(&self) -> Vec<PaletteItem> {
        let q = self.palette.as_ref().map(|p| p.query.clone()).unwrap_or_default();
        let items = self.palette_items();
        if q.trim().is_empty() {
            items.into_iter().take(60).collect()
        } else {
            items.into_iter().filter(|x| fuzzy(&q, &format!("{} {}", x.title, x.group))).take(60).collect()
        }
    }

    fn palette_items(&self) -> Vec<PaletteItem> {
        fn item(title: impl Into<String>, group: impl Into<String>, run: impl Fn(&mut GitTabs, &mut Window, &mut Context<GitTabs>) + 'static) -> PaletteItem {
            PaletteItem { title: title.into(), group: group.into(), run: Rc::new(run) }
        }
        let mut v: Vec<PaletteItem> = Vec::new();
        for (i, r) in self.repos.iter().enumerate() {
            v.push(item(format!("切換到 {}", r.name), "repo", move |t, _, cx| t.switch_tab(i, cx)));
        }
        if let Some(r) = self.repo() {
            let (p, n) = (r.path.clone(), r.name.clone());
            macro_rules! each {
                ($title:expr, $group:expr, |$t:ident, $path:ident, $cx:ident| $body:expr) => {{
                    let path = p.clone();
                    v.push(item($title, $group, move |$t, _, $cx| {
                        let $path = path.clone();
                        $body
                    }));
                }};
            }
            each!("Sync", n.clone(), |t, path, cx| t.sync_repo(&path, cx));
            each!("Fetch", n.clone(), |t, path, cx| t.fetch_repo(&path, cx));
            each!("Pull", n.clone(), |t, path, cx| t.pull_repo(&path, "ff", cx));
            each!("Pull --rebase", n.clone(), |t, path, cx| t.pull_repo(&path, "rebase", cx));
            each!("Push", n.clone(), |t, path, cx| t.push_repo(&path, false, cx));
            v.push(item("Stash", n.clone(), |t, w, cx| t.stash_push(w, cx)));
            v.push(item("新增分支", n.clone(), |t, w, cx| t.new_branch(None, w, cx)));
            v.push(item("看未提交的修改", "檢視", |t, _, cx| t.set_view(View::Changes, cx)));
            v.push(item("看歷史", "檢視", |t, _, cx| t.set_view(View::History, cx)));
            v.push(item("看分支 vs main", "檢視", |t, _, cx| t.set_view(View::Range, cx)));
            v.push(item("看 Reflog", "檢視", |t, _, cx| t.set_view(View::Reflog, cx)));
            let ps = p.display().to_string();
            let ps2 = ps.clone();
            v.push(item("在檔案總管開啟", n.clone(), move |t, _, cx| t.open_folder(ps.clone(), cx)));
            v.push(item("開終端機", n.clone(), move |t, _, cx| t.open_terminal(ps2.clone(), cx)));
            v.push(item("重新整理", n.clone(), |t, _, cx| t.refresh_active(cx)));
            for b in r.refs.iter().flat_map(|x| x.refs.branches.iter()).filter(|b| !b.current) {
                let (path, name) = (p.clone(), b.name.clone());
                v.push(item(format!("切換分支：{}", b.name), n.clone(), move |t, _, cx| t.switch_branch(&path, &name, cx)));
            }
            if !self.cfg.identities.is_empty() {
                v.push(item("身分：自動（依規則）", n.clone(), |t, _, cx| t.set_pin(None, cx)));
                for id in &self.cfg.identities {
                    let key = id.id.clone();
                    v.push(item(format!("身分：{} {} {}", id.name, tag_of(id), id.account), n.clone(), move |t, _, cx| t.set_pin(Some(key.clone()), cx)));
                }
            }
            for (i, x) in self.repos.iter().enumerate() {
                let up = x.summary.as_ref().filter(|s| s.ahead > 0).map(|s| format!(" (↑{})", s.ahead)).unwrap_or_default();
                let path = x.path.clone();
                v.push(item(format!("Push {}{up}", x.name), "遠端", move |t, _, cx| {
                    t.switch_tab(i, cx);
                    t.push_repo(&path, false, cx);
                }));
            }
            // Commits are searchable too: "fix depth" jumps to that commit.
            for c in r.commits.iter().take(400) {
                let sha = c.sha.clone();
                v.push(item(c.subject.clone(), short(&c.sha), move |t, _, cx| t.show_commit(sha.clone(), cx)));
            }
        }
        v.push(item("全部 repo 一起 Fetch", "遠端", |t, _, cx| t.fetch_all(false, cx)));
        v.push(item("加入 repo", "repo", |t, w, cx| t.open_add_repo(w, cx)));
        v.push(item("開啟設定", "設定", |t, w, cx| t.open_settings(crate::model::SettingsTab::General, None, w, cx)));
        v.push(item("身分與 SSH key", "設定", |t, w, cx| t.open_settings(crate::model::SettingsTab::Ids, None, w, cx)));
        v.push(item("切換深淺色", "外觀", |t, w, cx| t.cycle_theme(w, cx)));
        v
    }

    pub fn cycle_theme(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.cfg.theme = match self.cfg.theme.as_str() {
            "system" => "light",
            "light" => "dark",
            _ => "system",
        }
        .to_string();
        self.save_config();
        let _ = window;
        self.reapply_theme(cx);
    }
}
