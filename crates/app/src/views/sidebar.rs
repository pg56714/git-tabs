//! The left navigation: views, branches, remotes, tags, stashes, worktrees (Sidebar.vue).

use gpui_kit::{
    AnyElement, ClickEvent, Context, InteractiveElement as _, IntoElement, MouseButton, ParentElement as _, SharedString,
    StatefulInteractiveElement as _, Styled as _, Window, div, prelude::FluentBuilder as _, px,
};

use gpui_kit::component::Sizable as _;
use gpui_kit::component::input::Input;

use crate::app::GitTabs;
use crate::model::*;
use crate::ui::*;

/// Where the icon of a top-level row starts, and how far each level of a tree moves it right.
const IND: f32 = 22.;
const STEP: f32 = 14.;
/// The room a fold arrow takes before an icon (its width and the gap after it).
const CHEV: f32 = 18.;

/// The fold arrow of a section, a folder or a remote.
fn chevron(p: &crate::theme::Pal, closed: bool) -> gpui_kit::Svg {
    icon(if closed { Ico::ChevronRight } else { Ico::ChevronDown }, 12., p.muted)
}

impl GitTabs {
    fn fold_toggle(&mut self, key: &str, cx: &mut Context<Self>) {
        if !self.folded.remove(key) {
            self.folded.insert(key.to_string());
        }
        cx.notify();
    }

    /// A folder in a branch tree (`feature/` holding `feature/a`, `feature/b`). `indent` is where
    /// its folder icon goes, in line with the icons of the branches beside it; the arrow sits before.
    fn dir_row(&self, key: String, name: String, indent: f32, closed: bool, cx: &mut Context<Self>) -> AnyElement {
        let p = self.pal;
        let k = key.clone();
        self.item_row(SharedString::from(format!("fd-{key}")), indent - CHEV, false, false)
            .on_click(cx.listener(move |this, _, _, cx| this.fold_toggle(&k, cx)))
            .child(chevron(&p, closed))
            .child(icon(if closed { Ico::Folder } else { Ico::FolderOpen }, 13., p.muted))
            .child(div().overflow_hidden().text_ellipsis().child(name))
            .into_any_element()
    }

    fn nav_item(&self, id: &'static str, ico: Ico, label: String, count: Option<usize>, on: bool, view: View, cx: &mut Context<Self>) -> AnyElement {
        let p = self.pal;
        let fg = if on { p.accent_tx } else { p.text };
        div()
            .id(id)
            .flex()
            .items_center()
            .gap(px(8.))
            .mx(px(8.))
            .my(px(1.))
            .px(px(8.))
            .py(px(5.))
            .rounded(px(6.))
            .cursor_pointer()
            .bg(if on { p.accent_bg } else { gpui_kit::Hsla::transparent_black() })
            .text_color(fg)
            .when(on, |d| d.font_weight(gpui_kit::FontWeight::MEDIUM))
            .hover(move |s| s.bg(if on { p.accent_bg } else { p.hover }))
            .on_click(cx.listener(move |this, _, w, cx| {
                this.set_view(view, cx);
                this.nav_focus.focus(w, cx);
            }))
            .child(icon(ico, 15., if on { p.accent_tx } else { p.muted }))
            .child(label)
            .when_some(count.filter(|c| *c > 0), |d, c| {
                d.child(div().ml_auto().px(px(6.)).rounded(px(8.)).bg(if on { p.panel } else { p.hover }).text_size(px(11.5)).text_color(p.muted).child(c.to_string()))
            })
            .into_any_element()
    }

    /// A section title. An empty section is greyed and has no arrow: there is nothing to unfold.
    fn section(&self, id: &'static str, title: &str, empty: bool, cx: &mut Context<Self>) -> gpui_kit::Stateful<gpui_kit::Div> {
        let p = self.pal;
        let closed = self.folded.contains(id);
        div()
            .id(SharedString::from(format!("sh-{id}")))
            .flex()
            .items_center()
            .gap(px(6.))
            .pt(px(12.))
            .pb(px(3.))
            .px(px(12.))
            .text_size(px(13.))
            .font_weight(gpui_kit::FontWeight::SEMIBOLD)
            .text_color(if empty { p.faint } else { p.text })
            .when(!empty, |d| d.cursor_pointer().on_click(cx.listener(move |this, _, _, cx| this.fold_toggle(id, cx))))
            .child(if empty { div().w(px(12.)).flex_none().into_any_element() } else { chevron(&p, closed).into_any_element() })
            .child(title.to_string())
    }

    fn item_row(&self, id: SharedString, indent: f32, cur: bool, none: bool) -> gpui_kit::Stateful<gpui_kit::Div> {
        let p = self.pal;
        div()
            .id(id)
            .flex()
            .items_center()
            .gap(px(6.))
            .mx(px(8.))
            .pl(px(indent))
            .pr(px(8.))
            .py(px(3.))
            .min_h(px(26.))
            .text_size(px(13.))
            .rounded(px(6.))
            .whitespace_nowrap()
            .group("si")
            .when(cur, |d| d.font_weight(gpui_kit::FontWeight::SEMIBOLD))
            .when(!none, |d| d.cursor_pointer().hover(move |s| s.bg(p.hover)))
            .text_color(if none { p.faint } else { p.text })
    }

    /// The `+` at the right of a section title. It stays lit when the section is empty: that is
    /// where its first entry gets made.
    fn add_btn(
        &self,
        id: &'static str,
        tip: &'static str,
        cx: &mut Context<Self>,
        on: impl Fn(&mut GitTabs, &mut Window, &mut Context<GitTabs>) + 'static,
    ) -> gpui_kit::Stateful<gpui_kit::Div> {
        let p = self.pal;
        div()
            .id(id)
            .flex()
            .items_center()
            .justify_center()
            .size(px(20.))
            .rounded(px(5.))
            .cursor_pointer()
            .hover(move |s| s.bg(p.hover))
            .tooltip_text(tip)
            .on_click(cx.listener(move |this, _, w, cx| {
                // Not also a click on the title, which would fold the section.
                cx.stop_propagation();
                on(this, w, cx);
            }))
            .child(icon(Ico::Plus, 14., p.muted))
    }

    /// Right-click menu of a remote (`origin`).
    fn remote_menu(&self, path: &std::path::Path, name: &str, url: &str) -> Vec<MenuItem> {
        let (p1, n1, n2, n3, u1, u2) = (path.to_path_buf(), name.to_string(), name.to_string(), name.to_string(), url.to_string(), url.to_string());
        vec![
            MenuItem::new("Fetch", move |t, _, cx| t.fetch_repo(&p1, cx)).icon(Ico::RefreshCw),
            MenuItem::new(crate::i18n::t("改網址…"), move |t, w, cx| t.edit_remote_url(n1.clone(), u1.clone(), w, cx)).icon(Ico::SquarePen),
            MenuItem::new(crate::i18n::t("複製網址"), move |t, _, cx| t.copy(u2.clone(), crate::i18n::t("網址"), cx)).icon(Ico::Copy).disabled(url.is_empty()),
            MenuItem::sep(),
            MenuItem::new(crate::i18n::tr("移除 {name}…", &[("name", n3.clone())], &[]), move |t, _, cx| t.remove_remote(n2.clone(), cx)).danger(),
        ]
    }

    pub fn render_sidebar(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let p = self.pal;
        let Some(r) = self.repo() else { return div().into_any_element() };
        let path = r.path.clone();
        let view = r.view;
        let changes = r.summary.as_ref().map_or(0, |s| s.changes);
        let refs = r.refs.clone();
        // What the filter box holds. While it holds something, every match is shown, folded or not.
        let q = self.side_filter.read(cx).value().trim().to_lowercase();
        let filtering = !q.is_empty();
        let hit = |name: &str| !filtering || name.to_lowercase().contains(&q);
        let no_folds = std::collections::HashSet::new();
        let folds = if filtering { &no_folds } else { &self.folded };
        let open = |key: &str| filtering || !self.folded.contains(key);
        let mut rows: Vec<AnyElement> = vec![
            self.nav_item("nav-changes", Ico::FilePenLine, crate::i18n::t("未提交的修改").into(), Some(changes), view == View::Changes, View::Changes, cx),
            self.nav_item("nav-history", Ico::GitCommitVertical, crate::i18n::t("歷史").into(), None, view == View::History, View::History, cx),
            div()
                .px(px(10.))
                .pt(px(10.))
                .child(Input::new(&self.side_filter).small().cleanable(true).prefix(icon(Ico::ListFilter, 14., p.muted)))
                .into_any_element(),
        ];

        // ---- branches
        let branches: Vec<_> = refs.iter().flat_map(|x| x.refs.branches.iter()).filter(|b| hit(&b.name)).cloned().collect();
        rows.push(self.section("b", crate::i18n::t("分支"), branches.is_empty(), cx).into_any_element());
        if open("b") {
            let items: Vec<(String, usize)> = branches.iter().enumerate().map(|(i, b)| (b.name.clone(), i)).collect();
            for row in tree_rows(&items, "bt", folds) {
                let (label, idx, depth) = match row {
                    TreeRow::Dir { key, name, depth, closed } => {
                        rows.push(self.dir_row(key, name, IND + STEP * depth as f32, closed, cx));
                        continue;
                    }
                    TreeRow::Leaf { label, idx, depth } => (label, idx, depth),
                };
                let b = &branches[idx];
                let (name, sha, cur) = (b.name.clone(), b.sha.clone(), b.current);
                let (n_menu, n_dbl) = (name.clone(), name.clone());
                let (p1, p2) = (path.clone(), path.clone());
                // Commits to push and to pull, the way Fork writes them: `2↑ 1↓`.
                let tail = format!(
                    "{}{}{}",
                    if b.ahead > 0 { format!("{}↑", b.ahead) } else { String::new() },
                    if b.behind > 0 { format!(" {}↓", b.behind) } else { String::new() },
                    if b.gone { crate::i18n::t(" 已刪") } else { "" }
                );
                rows.push(
                    self.item_row(SharedString::from(format!("br-{name}")), IND + STEP * depth as f32, cur, false)
                        .on_click(cx.listener(move |this, ev: &ClickEvent, _, cx| {
                            if ev.click_count() >= 2 {
                                if !cur {
                                    this.switch_branch(&p1, &n_dbl, cx);
                                }
                            } else {
                                this.show_commit(sha.clone(), cx);
                            }
                        }))
                        .on_mouse_down(
                            MouseButton::Right,
                            cx.listener(move |this, ev: &gpui_kit::MouseDownEvent, _, cx| {
                                let n = n_menu.clone();
                                let items = this.branch_item_menu(&p2, &n, cur);
                                this.open_menu(ev.position, items, false, cx);
                            }),
                        )
                        .child(if cur {
                            div().w(px(13.)).flex_none().text_color(p.ok).font_weight(gpui_kit::FontWeight::BOLD).child("\u{2713}").into_any_element()
                        } else {
                            icon(Ico::GitBranch, 13., p.muted).into_any_element()
                        })
                        .child(div().overflow_hidden().text_ellipsis().child(label))
                        .child(div().ml_auto().flex_none().text_size(px(12.)).font_weight(gpui_kit::FontWeight::NORMAL).text_color(p.muted).child(tail))
                        .into_any_element(),
                );
            }
        }

        // ---- remotes
        // Every remote with its branches; a remote just added has none yet and is listed all the same.
        let mut groups: Vec<(String, String, Vec<(String, String)>)> =
            refs.iter().flat_map(|x| x.remote_list.iter()).map(|r| (r.name.clone(), r.url.clone(), Vec::new())).collect();
        for b in refs.iter().flat_map(|x| x.refs.remotes.iter()).filter(|b| hit(&b.name)) {
            // A remote's own name may hold a slash, so the known names are tried before the first slash.
            let known = groups.iter().position(|g| b.name.strip_prefix(g.0.as_str()).is_some_and(|rest| rest.starts_with('/')));
            match known {
                Some(i) => {
                    let rest = b.name[groups[i].0.len() + 1..].to_string();
                    groups[i].2.push((rest, b.sha.clone()));
                }
                None => {
                    let (remote, rest) = b.name.split_once('/').unwrap_or((&b.name, ""));
                    groups.push((remote.to_string(), String::new(), vec![(rest.to_string(), b.sha.clone())]));
                }
            }
        }
        if filtering {
            groups.retain(|g| !g.2.is_empty());
        }
        rows.push(
            self.section("r", crate::i18n::t("遠端"), groups.is_empty(), cx)
                .child(div().flex_1())
                .child(self.add_btn("rm-add", crate::i18n::t("新增 remote"), cx, |this, w, cx| this.add_remote(w, cx)))
                .into_any_element(),
        );
        if open("r") {
            for (remote, url, names) in &groups {
                let gkey = format!("rg:{remote}");
                let gclosed = !open(&gkey);
                let (m_name, m_url, m_path) = (remote.clone(), url.clone(), path.clone());
                rows.push(
                    self.item_row(SharedString::from(format!("rm-{remote}")), IND - CHEV, false, false)
                        .on_click(cx.listener({
                            let k = gkey.clone();
                            move |this, _, _, cx| this.fold_toggle(&k, cx)
                        }))
                        .on_mouse_down(
                            MouseButton::Right,
                            cx.listener(move |this, ev: &gpui_kit::MouseDownEvent, _, cx| {
                                let items = this.remote_menu(&m_path, &m_name, &m_url);
                                this.open_menu(ev.position, items, false, cx);
                            }),
                        )
                        .child(chevron(&p, gclosed))
                        .child(icon(remote_icon(url.contains("github.com")), 14., p.text))
                        .child(div().child(remote.clone()))
                        .into_any_element(),
                );
                if gclosed {
                    continue;
                }
                let items: Vec<(String, usize)> = names.iter().enumerate().map(|(i, (n, _))| (n.clone(), i)).collect();
                for row in tree_rows(&items, &format!("rt:{remote}"), folds) {
                    let (label, idx, depth) = match row {
                        TreeRow::Dir { key, name, depth, closed } => {
                            rows.push(self.dir_row(key, name, IND + STEP * (depth + 1) as f32, closed, cx));
                            continue;
                        }
                        TreeRow::Leaf { label, idx, depth } => (label, idx, depth),
                    };
                    let (n, sha) = &names[idx];
                    let full = format!("{remote}/{n}");
                    let (f1, f2, sha, p1, p2) = (full.clone(), full.clone(), sha.clone(), path.clone(), path.clone());
                    rows.push(
                        self.item_row(SharedString::from(format!("rb-{full}")), IND + STEP * (depth + 1) as f32, false, false)
                            .on_click(cx.listener(move |this, ev: &ClickEvent, _, cx| {
                                if ev.click_count() >= 2 {
                                    this.switch_branch(&p1, &f1, cx);
                                } else {
                                    this.show_commit(sha.clone(), cx);
                                }
                            }))
                            .on_mouse_down(
                                MouseButton::Right,
                                cx.listener(move |this, ev: &gpui_kit::MouseDownEvent, _, cx| {
                                    let items = this.remote_item_menu(&p2, &f2);
                                    this.open_menu(ev.position, items, false, cx);
                                }),
                            )
                            .child(icon(Ico::GitBranch, 13., p.muted))
                            .child(div().overflow_hidden().text_ellipsis().child(label))
                            .into_any_element(),
                    );
                }
            }
        }

        // ---- tags
        let tags: Vec<_> = refs.iter().flat_map(|x| x.refs.tags.iter()).filter(|t| hit(&t.name)).cloned().collect();
        rows.push(self.section("Tags", "Tags", tags.is_empty(), cx).into_any_element());
        if open("Tags") {
            for t in &tags {
                let (name, sha) = (t.name.clone(), t.sha.clone());
                let (n1, n2) = (name.clone(), name.clone());
                rows.push(
                    self.item_row(SharedString::from(format!("tg-{name}")), IND, false, false)
                        .on_click(cx.listener(move |this, _, _, cx| this.show_commit(sha.clone(), cx)))
                        .on_mouse_down(
                            MouseButton::Right,
                            cx.listener(move |this, ev: &gpui_kit::MouseDownEvent, _, cx| {
                                let (a, b, c, d) = (n2.clone(), n2.clone(), n2.clone(), n2.clone());
                                let items = vec![
                                    MenuItem::new(crate::i18n::t("Push 這個 tag"), move |t, _, cx| {
                                        if let Some(pth) = t.active_path() {
                                            t.simple_op(&pth, "push-tag", a.clone(), "Push tag", Some(crate::i18n::tr("已 push {a}", &[("a", (a).to_string())], &[])), cx)
                                        }
                                    }),
                                    MenuItem::new(crate::i18n::t("從這裡開新分支…"), move |t, w, cx| t.new_branch(Some(b.clone()), w, cx)),
                                    MenuItem::new(crate::i18n::t("複製名稱"), move |t, _, cx| t.copy(c.clone(), "tag", cx)),
                                    MenuItem::sep(),
                                    MenuItem::new(crate::i18n::t("刪除 tag"), move |t, _, cx| {
                                        if let Some(pth) = t.active_path() {
                                            t.simple_op(&pth, "tag-delete", d.clone(), crate::i18n::t("刪除 tag"), Some(crate::i18n::tr("已刪除 {d}", &[("d", (d).to_string())], &[])), cx)
                                        }
                                    })
                                    .danger(),
                                ];
                                this.open_menu(ev.position, items, false, cx);
                            }),
                        )
                        .child(icon(Ico::Tag, 13., p.muted))
                        .child(div().overflow_hidden().text_ellipsis().child(n1))
                        .into_any_element(),
                );
            }
        }

        // ---- stashes
        let stashes: Vec<_> = refs.iter().flat_map(|x| x.stashes.iter()).filter(|s| hit(&s.message)).cloned().collect();
        rows.push(self.section("s", "Stashes", stashes.is_empty(), cx).into_any_element());
        if open("s") {
            for s in &stashes {
                let (idx, msg, sha) = (s.index as usize, s.message.clone(), s.sha.clone());
                let m_path = path.clone();
                rows.push(
                    self.item_row(SharedString::from(format!("st-{idx}")), IND, false, false)
                        // A click shows what the stash holds; what to do with it is in the menu and beside the preview.
                        .on_click(cx.listener(move |this, _, _, cx| this.show_stash(sha.clone(), cx)))
                        .on_mouse_down(
                            MouseButton::Right,
                            cx.listener(move |this, ev: &gpui_kit::MouseDownEvent, _, cx| {
                                let (a, b) = (m_path.clone(), m_path.clone());
                                let items = vec![
                                    MenuItem::new(crate::i18n::t("套用"), move |t, _, cx| {
                                        t.simple_op(&a, "stash-apply", idx.to_string(), crate::i18n::t("套用 stash"), Some(crate::i18n::t("已套用 stash").into()), cx)
                                    }),
                                    MenuItem::new(crate::i18n::t("Pop（套用後刪除）"), move |t, _, cx| {
                                        t.simple_op(&b, "stash-pop", idx.to_string(), "Pop stash", Some(crate::i18n::t("已套用並刪除 stash").into()), cx)
                                    }),
                                    MenuItem::sep(),
                                    MenuItem::new(crate::i18n::t("刪除 stash…"), move |t, _, cx| t.stash_drop(idx, cx)).danger(),
                                ];
                                this.open_menu(ev.position, items, false, cx);
                            }),
                        )
                        .child(icon(Ico::Archive, 13., p.muted))
                        .child(div().overflow_hidden().text_ellipsis().child(msg))
                        .into_any_element(),
                );
            }
        }

        // ---- worktrees
        // Every worktree but the one this tab shows (which need not be the main one).
        let others: Vec<_> = refs
            .iter()
            .flat_map(|x| x.worktrees.iter())
            .filter(|w| !crate::format::same_path(&w.path, &path.display().to_string()))
            .filter(|w| hit(w.branch.as_deref().unwrap_or(&w.path)))
            .cloned()
            .collect();
        rows.push(
            self.section("w", "Worktrees", others.is_empty(), cx)
                .child(div().flex_1())
                .child(self.add_btn("wt-add", crate::i18n::t("新增 worktree"), cx, |this, w, cx| this.new_worktree(w, cx)))
                .into_any_element(),
        );
        if open("w") {
            for w in &others {
                let (wp, label) = (w.path.clone(), w.branch.clone().unwrap_or_else(|| w.head.chars().take(7).collect()));
                let (w1, w2) = (wp.clone(), wp.clone());
                rows.push(
                    self.item_row(SharedString::from(format!("wt-{wp}")), IND, false, false)
                        .on_click(cx.listener(move |this, _, _, cx| this.open_worktree(w1.clone(), cx)))
                        .on_mouse_down(
                            MouseButton::Right,
                            cx.listener(move |this, ev: &gpui_kit::MouseDownEvent, _, cx| {
                                let (a, b, c) = (w2.clone(), w2.clone(), w2.clone());
                                let items = vec![
                                    MenuItem::new(crate::i18n::t("開成新分頁"), move |t, _, cx| t.open_worktree(a.clone(), cx)),
                                    MenuItem::new(crate::i18n::t("複製路徑"), move |t, _, cx| t.copy(b.clone(), crate::i18n::t("路徑"), cx)),
                                    MenuItem::sep(),
                                    MenuItem::new(crate::i18n::t("移除 worktree…"), move |t, _, cx| t.remove_worktree(c.clone(), cx)).danger(),
                                ];
                                this.open_menu(ev.position, items, false, cx);
                            }),
                        )
                        .child(icon(Ico::FolderGit2, 13., p.muted))
                        .child(div().overflow_hidden().text_ellipsis().child(label))
                        .into_any_element(),
                );
            }
        }
        // ---- submodules, in a repo that has any
        if refs.as_ref().is_some_and(|x| !x.submodules.is_empty()) {
            let subs: Vec<_> = refs.iter().flat_map(|x| x.submodules.iter()).filter(|m| hit(&m.path)).cloned().collect();
            rows.push(self.section("m", "Submodules", subs.is_empty(), cx).into_any_element());
            if open("m") {
                for m in &subs {
                    let full = path.join(&m.path).display().to_string();
                    // Until it has been fetched there is nothing to open: a click fetches it instead.
                    let ready = m.state != "uninit";
                    let (f1, f2, f3, r1, r2) = (full.clone(), full.clone(), full, m.path.clone(), m.path.clone());
                    let note = match m.state {
                        "uninit" => crate::i18n::t("還沒取回"),
                        "changed" => crate::i18n::t("不是記錄的 commit"),
                        "conflict" => crate::i18n::t("有衝突"),
                        _ => "",
                    };
                    rows.push(
                        self.item_row(SharedString::from(format!("sm-{}", m.path)), IND, false, false)
                            .on_click(cx.listener(move |this, _, _, cx| if ready { this.open_worktree(f1.clone(), cx) } else { this.update_submodule(r1.clone(), cx) }))
                            .on_mouse_down(
                                MouseButton::Right,
                                cx.listener(move |this, ev: &gpui_kit::MouseDownEvent, _, cx| {
                                    let (a, b, c) = (f2.clone(), f3.clone(), r2.clone());
                                    let items = vec![
                                        MenuItem::new(crate::i18n::t("開成新分頁"), move |t, _, cx| t.open_worktree(a.clone(), cx)).disabled(!ready),
                                        MenuItem::new(crate::i18n::t("Update（取回，並切到記錄的 commit）"), move |t, _, cx| t.update_submodule(c.clone(), cx)).icon(Ico::RefreshCw),
                                        MenuItem::new(crate::i18n::t("複製路徑"), move |t, _, cx| t.copy(b.clone(), crate::i18n::t("路徑"), cx)).icon(Ico::Copy),
                                    ];
                                    this.open_menu(ev.position, items, false, cx);
                                }),
                            )
                            .child(icon(Ico::Package, 13., p.muted))
                            .child(div().overflow_hidden().text_ellipsis().child(m.path.clone()))
                            .when(!note.is_empty(), |d| d.child(div().ml_auto().flex_none().text_size(px(11.5)).text_color(p.warn).child(note)))
                            .into_any_element(),
                    );
                }
            }
        }
        rows.push(div().mt(px(10.)).child(self.nav_item("nav-reflog", Ico::RotateCcw, "Reflog".into(), None, view == View::Reflog, View::Reflog, cx)).into_any_element());

        div()
            .id("sidebar")
            .size_full()
            .py(px(8.))
            .bg(p.panel2)
            .overflow_y_scroll()
            .track_scroll(&self.side_scroll)
            .children(rows)
            .into_any_element()
    }

    /// Right-click menu of a remote branch (`origin/feature/x`).
    pub fn remote_item_menu(&self, path: &std::path::Path, full: &str) -> Vec<MenuItem> {
        let (pa, a, b, c, d, e) = (path.to_path_buf(), full.to_string(), full.to_string(), full.to_string(), full.to_string(), full.to_string());
        vec![
            MenuItem::new(crate::i18n::t("切換（建立本地分支）"), move |t, _, cx| t.switch_branch(&pa, &a, cx)),
            MenuItem::new(crate::i18n::t("Merge 進目前的分支"), move |t, _, cx| t.merge_into(b.clone(), cx)),
            MenuItem::new(crate::i18n::t("從這裡開新分支…"), move |t, w, cx| t.new_branch(Some(c.clone()), w, cx)),
            MenuItem::new(crate::i18n::t("複製名稱"), move |t, _, cx| t.copy(d.clone(), crate::i18n::t("分支名稱"), cx)),
            MenuItem::sep(),
            MenuItem::new(crate::i18n::t("刪除遠端分支…"), move |t, _, cx| t.delete_remote_branch(e.clone(), cx)).danger(),
        ]
    }

    pub fn branch_item_menu(&self, path: &std::path::Path, name: &str, current: bool) -> Vec<MenuItem> {
        let (p, n) = (path.to_path_buf(), name.to_string());
        let mk = |n: &str| n.to_string();
        let (a, b, c, d, f, g, h) = (mk(&n), mk(&n), mk(&n), mk(&n), mk(&n), mk(&n), mk(&n));
        vec![
            MenuItem::new(crate::i18n::t("切換到這個分支"), move |t, _, cx| t.switch_branch(&p, &a, cx)).disabled(current),
            MenuItem::new(crate::i18n::t("Merge 進目前的分支"), move |t, _, cx| t.merge_into(b.clone(), cx)).disabled(current),
            MenuItem::new(crate::i18n::t("把目前的分支 rebase 到這裡"), move |t, _, cx| t.rebase_onto(c.clone(), cx)).disabled(current),
            MenuItem::new(crate::i18n::t("從這裡開新分支…"), move |t, w, cx| t.new_branch(Some(d.clone()), w, cx)),
            MenuItem::new(crate::i18n::t("重新命名…"), move |t, w, cx| t.rename_branch(h.clone(), w, cx)),
            MenuItem::new(crate::i18n::t("複製名稱"), move |t, _, cx| t.copy(f.clone(), crate::i18n::t("分支名稱"), cx)),
            MenuItem::sep(),
            MenuItem::new(crate::i18n::t("刪除分支…"), move |t, _, cx| t.delete_branch(g.clone(), cx)).danger().disabled(current),
        ]
    }
}

/// One visible line of a branch tree.
enum TreeRow {
    Dir { key: String, name: String, depth: usize, closed: bool },
    Leaf { label: String, idx: usize, depth: usize },
}

#[derive(Default)]
struct TreeNode {
    dirs: std::collections::BTreeMap<String, TreeNode>,
    leaves: Vec<(String, usize)>,
}

/// Splits names on `/` into folders (folders first, like Fork) and returns the rows that are visible
/// given the folded set. `prefix` keeps the folder keys of different lists apart.
fn tree_rows(items: &[(String, usize)], prefix: &str, folded: &std::collections::HashSet<String>) -> Vec<TreeRow> {
    let mut root = TreeNode::default();
    for (name, idx) in items {
        let mut parts: Vec<&str> = name.split('/').collect();
        let leaf = parts.pop().unwrap_or(name);
        let mut node = &mut root;
        for d in parts {
            node = node.dirs.entry(d.to_string()).or_default();
        }
        node.leaves.push((leaf.to_string(), *idx));
    }
    fn emit(node: &TreeNode, depth: usize, key: &str, folded: &std::collections::HashSet<String>, out: &mut Vec<TreeRow>) {
        for (name, child) in &node.dirs {
            let k = format!("{key}/{name}");
            let closed = folded.contains(&k);
            out.push(TreeRow::Dir { key: k.clone(), name: name.clone(), depth, closed });
            if !closed {
                emit(child, depth + 1, &k, folded, out);
            }
        }
        let mut leaves = node.leaves.clone();
        leaves.sort();
        for (label, idx) in leaves {
            out.push(TreeRow::Leaf { label, idx, depth });
        }
    }
    let mut out = Vec::new();
    emit(&root, 0, prefix, folded, &mut out);
    out
}
