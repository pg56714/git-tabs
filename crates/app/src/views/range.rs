//! "分支 vs main" (RangeView.vue) and the Reflog list (ReflogView.vue).

use gpui_kit::{
    AnyElement, ClickEvent, Context, InteractiveElement as _, IntoElement, ParentElement as _, SharedString, StatefulInteractiveElement as _,
    Styled as _, Window, div, prelude::FluentBuilder as _, px,
};

use crate::app::*;
use crate::format::{ago, short};
use crate::model::*;
use crate::tree::TreeFile;
use crate::ui::*;
use crate::views::{SplitKind, diffpane::PaneMode, on_split, splitter};

impl GitTabs {
    fn pick_range_branch(&mut self, name: String, cx: &mut Context<Self>) {
        let Some(r) = self.repo_mut() else { return };
        r.range_head = Some(name);
        r.sel = None;
        let p = r.path.clone();
        self.load_range(&p, cx);
    }

    pub fn render_range(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let p = self.pal;
        let Some(r) = self.repo() else { return div().into_any_element() };
        let base = Self::range_base(r);
        let cands = Self::range_candidates(r);
        let Some(base) = base else { return empty(&p, "⇄", crate::i18n::t("找不到 main / master 分支可以比較"), None) };
        if cands.is_empty() {
            return empty(&p, "⇄", &crate::i18n::tr("{} 只有 {base}，沒有其他分支可以比較", &[("base", (base).to_string())], &[(r.name).to_string()]), None);
        }
        let head = r.range_head.clone().unwrap_or_default();
        let files: Vec<TreeFile> = r.range_files.iter().map(|f| TreeFile { path: f.path.clone(), status: f.status }).collect();
        let commits = r.range_commits.clone();
        let seen_n = r.range_files.iter().filter(|f| self.is_seen(r, ListKind::R, &f.path)).count();
        let total = files.len();
        let current = r.summary.as_ref().and_then(|s| s.branch.clone());
        let is_current = current.as_deref() == Some(head.as_str());
        let on_base = current.as_deref() == Some(base.as_str());
        let sel = r.sel.clone();
        let sel_r = sel.as_ref().filter(|s| s.list == ListKind::R).map(|s| s.path.clone());
        let status = sel.as_ref().and_then(|s| r.range_files.iter().find(|f| f.path == s.path)).map(|f| f.status);
        let tree = self.render_filetree(ListKind::R, &files, sel_r.as_deref(), cx);
        let path = r.path.clone();
        let (h1, h2, h3, b1) = (head.clone(), head.clone(), head.clone(), base.clone());
        let files_w = self.sizes.files;

        let cbar = div()
            .flex()
            .flex_none()
            .flex_wrap()
            .items_center()
            .gap(px(6.))
            .px(px(10.))
            .py(px(6.))
            .min_h(px(41.))
            .border_b_1()
            .border_color(p.border)
            .child(btn(&p, "range-pick", Bk::Normal, false, format!("{head} ▾")).on_click(cx.listener(move |this, ev: &ClickEvent, _, cx| {
                let items = cands.iter().map(|b| {
                    let n = b.clone();
                    MenuItem::new(b.clone(), move |t, _, cx| t.pick_range_branch(n.clone(), cx))
                }).collect();
                this.open_menu(ev.position(), items, false, cx);
            })))
            .child(sub(&p, crate::i18n::tr("vs {base} · {} 個 commit · {total} 個檔案 · 已看 {seen_n}", &[("base", (base).to_string()), ("total", (total).to_string()), ("seen_n", (seen_n).to_string())], &[(commits.len()).to_string()])))
            .child(div().flex_1())
            .when(!is_current && !head.is_empty(), |d| {
                d.child(btn(&p, "range-switch", Bk::Normal, true, crate::i18n::tr("切換到 {h1}", &[("h1", (h1).to_string())], &[])).on_click(cx.listener(move |this, _, _, cx| {
                    if let Some(pth) = this.active_path() {
                        this.switch_branch(&pth, &h1, cx);
                    }
                })))
            })
            .when(on_base && !commits.is_empty(), |d| {
                d.child(btn(&p, "range-merge", Bk::Primary, true, crate::i18n::tr("把 {h2} merge 進 {b1}", &[("h2", (h2).to_string()), ("b1", (b1).to_string())], &[])).on_click(cx.listener(move |this, _, _, cx| this.merge_into(h2.clone(), cx))))
            })
            .when(!on_base && !commits.is_empty(), |d| d.child(sub(&p, crate::i18n::tr("（切到 {base} 之後可以直接 merge）", &[("base", (base).to_string())], &[]))));

        if total == 0 {
            return div().flex().flex_col().flex_1().min_h_0().child(cbar).child(empty(&p, "", &crate::i18n::tr("{h3} 跟 {base} 沒有差異", &[("h3", (h3).to_string()), ("base", (base).to_string())], &[]), None)).into_any_element();
        }
        let right: AnyElement = match status {
            Some(st) => div().flex().flex_col().flex_1().min_w_0().min_h_0().child(self.render_diff_head(st, vec![], cx)).child(self.render_diff_area(PaneMode::ReadOnly, cx)).child(self.render_diff_hint(false)).into_any_element(),
            None => div().flex().flex_col().flex_1().child(empty(&p, "", crate::i18n::t("選一個檔案"), None)).child(self.render_diff_hint(false)).into_any_element(),
        };
        let left = div()
            .flex()
            .flex_col()
            .flex_none()
            .w(px(files_w))
            .h_full()
            .min_h_0()
            .overflow_hidden()
            .child(
                div()
                    .h(px(3.))
                    .mx(px(10.))
                    .mt(px(8.))
                    .mb(px(6.))
                    .rounded(px(2.))
                    .bg(p.border)
                    .child(div().h(px(3.)).rounded(px(2.)).bg(p.accent).w(gpui_kit::relative(seen_n as f32 / total.max(1) as f32))),
            )
            .child(div().id("range-files").flex_1().min_h_0().overflow_y_scroll().track_scroll(&self.files_scroll).child(tree))
            .child(
                div()
                    .id("range-commits")
                    .flex_none()
                    .max_h(gpui_kit::relative(0.35))
                    .overflow_y_scroll()
                    .border_t_1()
                    .border_color(p.border)
                    .pb(px(6.))
                    .child(div().px(px(10.)).pt(px(8.)).pb(px(4.)).text_size(px(12.)).text_color(p.muted).child(crate::i18n::t("這個分支的 commit")))
                    .children(commits.iter().map(|c| {
                        let sha = c.sha.clone();
                        div()
                            .id(SharedString::from(format!("rc-{}", c.sha)))
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .px(px(10.))
                            .py(px(2.))
                            .cursor_pointer()
                            .whitespace_nowrap()
                            .hover(move |s| s.bg(p.hover))
                            .on_click(cx.listener(move |this, _, _, cx| this.show_commit(sha.clone(), cx)))
                            .child(div().font_family("Cascadia Mono").child(sub(&p, short(&c.sha))))
                            .child(div().overflow_hidden().text_ellipsis().child(c.subject.clone()))
                            .child(div().flex_1())
                            .child(sub(&p, ago(c.time)))
                    })),
            );
        let _ = path;
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .child(cbar)
            .child(
                on_split(div().id("range-review").flex().flex_1().min_h_0(), SplitKind::Files, cx)
                    .child(left)
                    .child(splitter(&p, "split-range", SplitKind::Files, true))
                    .child(
                        div()
                            .id("range-nav")
                            .key_context("Nav")
                            .track_focus(&self.nav_focus)
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .min_h_0()
                            .on_mouse_down(gpui_kit::MouseButton::Left, cx.listener(|this, _, w, cx| this.nav_focus.focus(w, cx)))
                            .on_action(cx.listener(|this, _: &NextFile, _, cx| this.move_sel(1, cx)))
                            .on_action(cx.listener(|this, _: &PreviousFile, _, cx| this.move_sel(-1, cx)))
                            .on_action(cx.listener(|this, _: &NextHunk, _, cx| this.goto_hunk(1, cx)))
                            .on_action(cx.listener(|this, _: &PreviousHunk, _, cx| this.goto_hunk(-1, cx)))
                            .on_action(cx.listener(|this, _: &ToggleSeenAction, _, cx| this.toggle_seen(true, cx)))
                            .child(right),
                    ),
            )
            .into_any_element()
    }

    pub fn render_reflog(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let p = self.pal;
        let Some(r) = self.repo() else { return div().into_any_element() };
        let rows = r.reflog.clone();
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .px(px(10.))
                    .py(px(6.))
                    .min_h(px(41.))
                    .border_b_1()
                    .border_color(p.border)
                    .child(sub(&p, crate::i18n::t("HEAD 的移動紀錄。不小心 reset、rebase 或刪掉分支時，可以從這裡找回 commit。"))),
            )
            .child(
                div()
                    .id("reflog")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .children(rows.iter().map(|e| {
                        let (s1, s2, s3, s4) = (e.sha.clone(), e.sha.clone(), e.sha.clone(), e.sha.clone());
                        div()
                            .id(SharedString::from(format!("rl-{}", e.selector)))
                            .group("rl")
                            .flex()
                            .items_center()
                            .gap(px(10.))
                            .px(px(12.))
                            .py(px(4.))
                            .border_b_1()
                            .border_color(p.border)
                            .hover(move |s| s.bg(p.hover))
                            .child(div().w(px(90.)).flex_none().font_family("Cascadia Mono").child(sub(&p, e.selector.clone())))
                            .child(
                                div()
                                    .id(SharedString::from(format!("rls-{}", e.selector)))
                                    .w(px(70.))
                                    .flex_none()
                                    .font_family("Cascadia Mono")
                                    .text_color(p.accent)
                                    .cursor_pointer()
                                    .child(short(&e.sha))
                                    .on_click(cx.listener(move |this, _, _, cx| this.copy(s1.clone(), &short(&s1), cx))),
                            )
                            .child(div().flex_1().min_w_0().overflow_hidden().text_ellipsis().whitespace_nowrap().child(e.message.clone()))
                            .child(div().w(px(100.)).flex_none().child(sub(&p, ago(e.time))))
                            .child(
                                div()
                                    .flex()
                                    .flex_none()
                                    .gap(px(4.))
                                    .opacity(0.)
                                    .group_hover("rl", |s| s.opacity(1.))
                                    .child(btn(&p, SharedString::from(format!("rlb-{}", e.selector)), Bk::Normal, true, crate::i18n::t("從這裡開分支")).on_click(cx.listener(move |this, _, w, cx| this.new_branch(Some(s2.clone()), w, cx))))
                                    .child(btn(&p, SharedString::from(format!("rlc-{}", e.selector)), Bk::Normal, true, "Checkout").on_click(cx.listener(move |this, _, _, cx| this.checkout_commit(s3.clone(), cx))))
                                    .child(btn(&p, SharedString::from(format!("rlr-{}", e.selector)), Bk::Normal, true, crate::i18n::t("Reset 到這裡")).on_click(cx.listener(move |this, _, _, cx| this.reset_to(s4.clone(), cx)))),
                            )
                    }))
                    .when(rows.is_empty(), |d| d.child(empty(&p, "", crate::i18n::t("沒有紀錄"), None))),
            )
            .into_any_element()
    }
}
