//! The left navigation: views, branches, remotes, tags, stashes, worktrees (Sidebar.vue).

use gpui_kit::{
    AnyElement, ClickEvent, Context, InteractiveElement as _, IntoElement, MouseButton, ParentElement as _, SharedString,
    StatefulInteractiveElement as _, Styled as _, Window, div, prelude::FluentBuilder as _, px,
};

use crate::app::GitTabs;
use crate::model::*;
use crate::ui::*;

impl GitTabs {
    fn fold_toggle(&mut self, key: &'static str, cx: &mut Context<Self>) {
        if !self.folded.remove(key) {
            self.folded.insert(key);
        }
        cx.notify();
    }

    fn nav_item(&self, id: &'static str, label: String, count: Option<usize>, on: bool, view: View, cx: &mut Context<Self>) -> AnyElement {
        let p = self.pal;
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
            .text_color(if on { p.accent_tx } else { p.muted })
            .hover(move |s| s.bg(if on { p.accent_bg } else { p.hover }))
            .on_click(cx.listener(move |this, _, _, cx| this.set_view(view, cx)))
            .child(label)
            .when_some(count.filter(|c| *c > 0), |d, c| d.child(div().ml_auto().text_size(px(11.5)).child(c.to_string())))
            .into_any_element()
    }

    fn section(&self, id: &'static str, title: &str, count: Option<usize>, cx: &mut Context<Self>) -> AnyElement {
        let p = self.pal;
        let closed = self.folded.contains(id);
        div()
            .id(SharedString::from(format!("sh-{id}")))
            .flex()
            .items_center()
            .gap(px(4.))
            .pt(px(12.))
            .pb(px(3.))
            .px(px(14.))
            .text_size(px(12.))
            .font_weight(gpui_kit::FontWeight::SEMIBOLD)
            .text_color(p.muted)
            .cursor_pointer()
            .hover(move |s| s.text_color(p.text))
            .on_click(cx.listener(move |this, _, _, cx| this.fold_toggle(id, cx)))
            .child(format!("{} {}", if closed { "▸" } else { "▾" }, title))
            .when_some(count.filter(|c| *c > 0), |d, c| d.child(div().ml_1().font_weight(gpui_kit::FontWeight::NORMAL).text_color(p.faint).child(c.to_string())))
            .into_any_element()
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
            .rounded(px(6.))
            .whitespace_nowrap()
            .group("si")
            .when(cur, |d| d.font_weight(gpui_kit::FontWeight::SEMIBOLD))
            .when(!none, |d| d.cursor_pointer().hover(move |s| s.bg(p.hover)))
            .text_color(if none { p.faint } else { p.text })
    }

    pub fn render_sidebar(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let p = self.pal;
        let Some(r) = self.repo() else { return div().into_any_element() };
        let path = r.path.clone();
        let view = r.view;
        let default = r.refs.as_ref().and_then(|x| x.default_branch.clone()).unwrap_or_else(|| "main".into());
        let changes = r.summary.as_ref().map_or(0, |s| s.changes);
        let refs = r.refs.clone();
        let mut rows: Vec<AnyElement> = vec![
            self.nav_item("nav-changes", crate::i18n::t("✎ 未提交的修改").into(), Some(changes), view == View::Changes, View::Changes, cx),
            self.nav_item("nav-history", crate::i18n::t("◷ 歷史").into(), None, view == View::History, View::History, cx),
            self.nav_item("nav-range", crate::i18n::tr("⇄ 分支 vs {default}", &[("default", (default).to_string())], &[]), None, view == View::Range, View::Range, cx),
        ];

        // ---- branches
        rows.push(self.section("b", crate::i18n::t("分支"), refs.as_ref().map(|x| x.refs.branches.len()), cx));
        if !self.folded.contains("b") {
            for b in refs.iter().flat_map(|x| x.refs.branches.iter()) {
                let (name, sha, cur) = (b.name.clone(), b.sha.clone(), b.current);
                let (n_click, n_menu, n_dbl) = (name.clone(), name.clone(), name.clone());
                let (p1, p2) = (path.clone(), path.clone());
                let tail = format!(
                    "{}{}{}",
                    if b.ahead > 0 { format!("↑{}", b.ahead) } else { String::new() },
                    if b.behind > 0 { format!(" ↓{}", b.behind) } else { String::new() },
                    if b.gone { crate::i18n::t(" 已刪") } else { "" }
                );
                rows.push(
                    self.item_row(SharedString::from(format!("br-{name}")), 22., cur, false)
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
                        .child(div().w(px(10.)).flex_none().text_color(p.ok).child(if cur { "✓" } else { "" }))
                        .child(div().overflow_hidden().text_ellipsis().child(n_click))
                        .child(div().ml_auto().flex_none().text_size(px(11.5)).text_color(p.accent).child(tail))
                        .into_any_element(),
                );
            }
        }

        // ---- remotes
        rows.push(self.section("r", crate::i18n::t("遠端"), None, cx));
        if !self.folded.contains("r") {
            let mut groups: Vec<(String, Vec<(String, String)>)> = Vec::new();
            for b in refs.iter().flat_map(|x| x.refs.remotes.iter()) {
                let (remote, rest) = b.name.split_once('/').unwrap_or((&b.name, ""));
                match groups.iter_mut().find(|g| g.0 == remote) {
                    Some(g) => g.1.push((rest.to_string(), b.sha.clone())),
                    None => groups.push((remote.to_string(), vec![(rest.to_string(), b.sha.clone())])),
                }
            }
            for (remote, names) in &groups {
                rows.push(self.item_row(SharedString::from(format!("rm-{remote}")), 22., false, true).child(div().text_color(p.muted).child(remote.clone())).into_any_element());
                for (n, sha) in names {
                    let full = format!("{remote}/{n}");
                    let (f1, f2, sha, p1, p2) = (full.clone(), full.clone(), sha.clone(), path.clone(), path.clone());
                    rows.push(
                        self.item_row(SharedString::from(format!("rb-{full}")), 38., false, false)
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
                                    let (a, b, c) = (f2.clone(), f2.clone(), f2.clone());
                                    let (pa, pb) = (p2.clone(), p2.clone());
                                    let items = vec![
                                        MenuItem::new(crate::i18n::t("切換（建立本地分支）"), move |t, _, cx| t.switch_branch(&pa, &a, cx)),
                                        MenuItem::new(crate::i18n::t("Merge 進目前的分支"), move |t, _, cx| t.merge_into(b.clone(), cx)),
                                        MenuItem::new(crate::i18n::t("從這裡開新分支…"), move |t, w, cx| t.new_branch(Some(c.clone()), w, cx)),
                                    ];
                                    let _ = &pb;
                                    this.open_menu(ev.position, items, false, cx);
                                }),
                            )
                            .child(div().overflow_hidden().text_ellipsis().child(n.clone()))
                            .into_any_element(),
                    );
                }
            }
            if groups.is_empty() {
                rows.push(self.item_row("rm-none".into(), 22., false, true).text_size(px(12.)).child(crate::i18n::t("沒有遠端分支")).into_any_element());
            }
        }

        // ---- tags
        rows.push(self.section("Tags", "Tags", refs.as_ref().map(|x| x.refs.tags.len()), cx));
        if !self.folded.contains("Tags") {
            for t in refs.iter().flat_map(|x| x.refs.tags.iter()) {
                let (name, sha) = (t.name.clone(), t.sha.clone());
                let (n1, n2) = (name.clone(), name.clone());
                rows.push(
                    self.item_row(SharedString::from(format!("tg-{name}")), 22., false, false)
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
                        .child(div().flex_none().child("🏷"))
                        .child(div().overflow_hidden().text_ellipsis().child(n1))
                        .into_any_element(),
                );
            }
            if refs.as_ref().is_none_or(|x| x.refs.tags.is_empty()) {
                rows.push(self.item_row("tg-none".into(), 22., false, true).text_size(px(12.)).child(crate::i18n::t("沒有 tag")).into_any_element());
            }
        }

        // ---- stashes
        rows.push(self.section("s", "Stashes", refs.as_ref().map(|x| x.stashes.len()), cx));
        if !self.folded.contains("s") {
            for s in refs.iter().flat_map(|x| x.stashes.iter()) {
                let (idx, msg) = (s.index as usize, s.message.clone());
                let (a, b, c) = (path.clone(), path.clone(), idx);
                let small = |id: String, label: &'static str| btn(&p, SharedString::from(id), Bk::Normal, true, label);
                rows.push(
                    self.item_row(SharedString::from(format!("st-{idx}")), 22., false, false)
                        .child(div().overflow_hidden().text_ellipsis().child(msg))
                        .child(
                            div()
                                .hidden()
                                .group_hover("si", |d| d.flex())
                                .ml_auto()
                                .gap(px(2.))
                                .child(small(format!("sa-{idx}"), crate::i18n::t("套用")).on_click(cx.listener(move |this, _, _, cx| this.simple_op(&a, "stash-apply", idx.to_string(), crate::i18n::t("套用 stash"), Some(crate::i18n::t("已套用 stash").into()), cx))))
                                .child(small(format!("sp-{idx}"), "Pop").on_click(cx.listener(move |this, _, _, cx| this.simple_op(&b, "stash-pop", idx.to_string(), "Pop stash", Some(crate::i18n::t("已套用並刪除 stash").into()), cx))))
                                .child(small(format!("sd-{idx}"), "×").on_click(cx.listener(move |this, _, _, cx| this.stash_drop(c, cx)))),
                        )
                        .into_any_element(),
                );
            }
            if refs.as_ref().is_none_or(|x| x.stashes.is_empty()) {
                rows.push(self.item_row("st-none".into(), 22., false, true).text_size(px(12.)).child(crate::i18n::t("沒有 stash")).into_any_element());
            }
        }

        // ---- worktrees
        let others: Vec<_> = refs.iter().flat_map(|x| x.worktrees.iter().skip(1)).cloned().collect();
        rows.push(self.section("w", "Worktrees", Some(others.len()), cx));
        if !self.folded.contains("w") {
            for w in &others {
                let (wp, label) = (w.path.clone(), w.branch.clone().unwrap_or_else(|| w.head.chars().take(7).collect()));
                let (w1, w2) = (wp.clone(), wp.clone());
                rows.push(
                    self.item_row(SharedString::from(format!("wt-{wp}")), 22., false, false)
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
                        .child("🌿")
                        .child(div().overflow_hidden().text_ellipsis().child(label))
                        .into_any_element(),
                );
            }
            if others.is_empty() {
                rows.push(self.item_row("wt-none".into(), 22., false, true).text_size(px(12.)).child(crate::i18n::t("沒有其他 worktree")).into_any_element());
            }
        }
        rows.push(div().mt(px(8.)).child(self.nav_item("nav-reflog", "↺ Reflog".into(), None, view == View::Reflog, View::Reflog, cx)).into_any_element());

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

    pub fn branch_item_menu(&self, path: &std::path::Path, name: &str, current: bool) -> Vec<MenuItem> {
        let (p, n) = (path.to_path_buf(), name.to_string());
        let mk = |n: &str| n.to_string();
        let (a, b, c, d, e, f, g) = (mk(&n), mk(&n), mk(&n), mk(&n), mk(&n), mk(&n), mk(&n));
        vec![
            MenuItem::new(crate::i18n::t("切換到這個分支"), move |t, _, cx| t.switch_branch(&p, &a, cx)).disabled(current),
            MenuItem::new(crate::i18n::t("Merge 進目前的分支"), move |t, _, cx| t.merge_into(b.clone(), cx)).disabled(current),
            MenuItem::new(crate::i18n::t("把目前的分支 rebase 到這裡"), move |t, _, cx| t.rebase_onto(c.clone(), cx)).disabled(current),
            MenuItem::new(crate::i18n::t("從這裡開新分支…"), move |t, w, cx| t.new_branch(Some(d.clone()), w, cx)),
            MenuItem::new(crate::i18n::t("跟 main 比較"), move |t, _, cx| {
                if let Some(r) = t.repo_mut() {
                    r.range_head = Some(e.clone());
                }
                t.set_view(View::Range, cx);
            }),
            MenuItem::new(crate::i18n::t("複製名稱"), move |t, _, cx| t.copy(f.clone(), crate::i18n::t("分支名稱"), cx)),
            MenuItem::sep(),
            MenuItem::new(crate::i18n::t("刪除分支…"), move |t, _, cx| t.delete_branch(g.clone(), cx)).danger().disabled(current),
        ]
    }
}
