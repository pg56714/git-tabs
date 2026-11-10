//! The "未提交的修改" view (ChangesView.vue): file lists, review progress, commit box, diff.

use gpui_kit::component::input::{Input, Textarea};
use gpui_kit::{
    AnyElement, Context, InteractiveElement as _, IntoElement, ParentElement as _, StatefulInteractiveElement as _, Styled as _, Window, div,
    prelude::FluentBuilder as _, px,
};

use crate::app::*;
use crate::model::*;
use crate::tree::TreeFile;
use crate::ui::*;
use crate::views::{SplitKind, diffpane::PaneMode, on_split, splitter};

impl GitTabs {
    pub fn render_changes(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let p = self.pal;
        let Some(r) = self.repo() else { return div().into_any_element() };
        let un: Vec<TreeFile> = r.unstaged().iter().map(|f| TreeFile { path: f.path.clone(), status: if f.conflicted { 'U' } else if f.untracked { 'A' } else { f.y } }).collect();
        let st: Vec<TreeFile> = r.staged().iter().map(|f| TreeFile { path: f.path.clone(), status: f.x }).collect();
        let mut all: Vec<&str> = r.status.iter().map(|f| f.path.as_str()).collect();
        all.sort();
        all.dedup();
        let seen_n = all.iter().filter(|x| self.is_seen(r, ListKind::U, x)).count();
        let total = all.len();
        let sel = r.sel.clone();
        let sel_entry = sel.as_ref().and_then(|s| r.status.iter().find(|f| f.path == s.path)).cloned();
        let sel_status = match (&sel, &sel_entry) {
            (Some(s), Some(e)) => if s.list == ListKind::S { e.x } else if e.conflicted { 'U' } else if e.untracked { 'A' } else { e.y },
            _ => 'M',
        };
        let conflicts = un.iter().filter(|f| f.status == 'U').count();
        let state = r.summary.as_ref().and_then(|s| s.state);
        let state_name = match state {
            Some("merge") => "Merge",
            Some("rebase") => "Rebase",
            Some("cherry-pick") => "Cherry-pick",
            Some("revert") => "Revert",
            _ => "Merge",
        };
        let empty_status = r.status.is_empty();
        let sel_u = sel.as_ref().filter(|s| s.list == ListKind::U).map(|s| s.path.clone());
        let sel_s = sel.as_ref().filter(|s| s.list == ListKind::S).map(|s| s.path.clone());
        let files_w = self.sizes.files;
        let (un_tree, st_tree) = (self.render_filetree(ListKind::U, &un, sel_u.as_deref(), cx), self.render_filetree(ListKind::S, &st, sel_s.as_deref(), cx));
        let (un_n, st_n) = (un.len(), st.len());
        let amend = self.amend;
        let committing = self.committing;
        let err = self.commit_err.clone();

        let banner: Option<AnyElement> = state.map(|_| {
            div()
                .flex()
                .flex_none()
                .flex_wrap()
                .items_center()
                .gap(px(8.))
                .px(px(12.))
                .py(px(8.))
                .bg(p.warn_bg)
                .text_color(p.warn)
                .border_b_1()
                .border_color(p.border)
                .child(div().font_weight(gpui_kit::FontWeight::BOLD).child(crate::i18n::tr("正在 {state_name}", &[("state_name", (state_name).to_string())], &[])))
                .child(if conflicts > 0 { crate::i18n::tr("還有 {conflicts} 個衝突的檔案。解決後按「繼續」。", &[("conflicts", (conflicts).to_string())], &[]) } else { crate::i18n::t("衝突都解決了，按「繼續」完成。").to_string() })
                .child(div().flex_1())
                .child(dim(
                    btn(&p, "continue", Bk::Primary, true, crate::i18n::t("繼續")).when(conflicts == 0, |b| b.on_click(cx.listener(|this, _, _, cx| this.abort_or_continue("continue", cx)))),
                    conflicts > 0,
                ))
                .child(btn(&p, "abort", Bk::Normal, true, crate::i18n::tr("中止 {state_name}", &[("state_name", (state_name).to_string())], &[])).on_click(cx.listener(|this, _, _, cx| this.abort_or_continue("abort", cx))))
                .into_any_element()
        });

        if empty_status {
            return div()
                .flex()
                .flex_col()
                .flex_1()
                .min_h_0()
                .when_some(banner, |d, b| d.child(b))
                .child(empty(&p, "✓", crate::i18n::t("沒有未提交的修改"), Some(crate::i18n::t("工作目錄是乾淨的"))))
                .into_any_element();
        }

        // ---- diff side
        let head_extra: Vec<AnyElement> = match (&sel, &sel_entry) {
            (Some(s), Some(e)) if e.conflicted => {
                let (a, b, c, d) = (s.path.clone(), s.path.clone(), s.path.clone(), s.path.clone());
                vec![
                    btn(&p, "c-ours", Bk::Normal, true, crate::i18n::t("用我的")).on_click(cx.listener(move |this, _, _, cx| this.resolve_conflict(a.clone(), "use-ours", cx))).into_any_element(),
                    btn(&p, "c-theirs", Bk::Normal, true, crate::i18n::t("用對方的")).on_click(cx.listener(move |this, _, _, cx| this.resolve_conflict(b.clone(), "use-theirs", cx))).into_any_element(),
                    btn(&p, "c-edit", Bk::Normal, true, crate::i18n::t("用編輯器開啟")).on_click(cx.listener(move |this, _, _, cx| this.open_file(&c, cx))).into_any_element(),
                    btn(&p, "c-done", Bk::Primary, true, crate::i18n::t("標記已解決")).on_click(cx.listener(move |this, _, _, cx| this.resolve_conflict(d.clone(), "mark-resolved", cx))).into_any_element(),
                ]
            }
            (Some(s), _) if s.list == ListKind::U => {
                let (a, b) = (s.path.clone(), s.path.clone());
                vec![
                    btn(&p, "h-stage", Bk::Normal, true, crate::i18n::t("Stage 檔案")).tooltip_text("S").on_click(cx.listener(move |this, _, _, cx| this.stage_files(vec![a.clone()], cx))).into_any_element(),
                    btn(&p, "h-discard", Bk::Normal, true, crate::i18n::t("捨棄")).on_click(cx.listener(move |this, _, _, cx| this.discard_files(vec![b.clone()], cx))).into_any_element(),
                ]
            }
            (Some(s), _) => {
                let a = s.path.clone();
                vec![btn(&p, "h-unstage", Bk::Normal, true, crate::i18n::t("取消 Stage")).tooltip_text("S").on_click(cx.listener(move |this, _, _, cx| this.unstage_files(vec![a.clone()], cx))).into_any_element()]
            }
            _ => vec![],
        };
        let mode = match (&sel, &sel_entry) {
            (_, Some(e)) if e.conflicted => PaneMode::ReadOnly,
            (Some(s), _) if s.list == ListKind::S => PaneMode::Staged,
            _ => PaneMode::Unstaged,
        };
        let diff_side: AnyElement = if sel.is_some() {
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .min_h_0()
                .child(self.render_diff_head(sel_status, head_extra, cx))
                .child(self.render_diff_area(mode, cx))
                .child(self.render_diff_hint(true))
                .into_any_element()
        } else {
            div().flex().flex_col().flex_1().min_w_0().child(empty(&p, "", crate::i18n::t("選一個檔案"), None)).child(self.render_diff_hint(true)).into_any_element()
        };

        // ---- left: lists + commit box
        let section = |id: &'static str, title: &str, n: usize, btn_el: Option<AnyElement>| {
            div()
                .flex()
                .items_center()
                .gap(px(6.))
                .px(px(10.))
                .pt(px(8.))
                .pb(px(4.))
                .text_size(px(12.))
                .text_color(p.muted)
                .child(div().child(title.to_string()))
                .child(sub(&p, n.to_string()))
                .child(div().flex_1())
                .when_some(btn_el, |d, b| d.child(b))
                .id(id)
        };
        let left = div()
            .flex()
            .flex_col()
            .flex_none()
            .w(px(files_w))
            .h_full()
            .min_h_0()
            .overflow_hidden()
            .child(div().px(px(10.)).pt(px(8.)).pb(px(4.)).child(sub(&p, crate::i18n::tr("已看 {seen_n} / {total} 個檔案", &[("seen_n", (seen_n).to_string()), ("total", (total).to_string())], &[]))))
            .child(
                div()
                    .h(px(3.))
                    .mx(px(10.))
                    .mb(px(4.))
                    .rounded(px(2.))
                    .bg(p.border)
                    .child(div().h(px(3.)).rounded(px(2.)).bg(p.accent).w(gpui_kit::relative(if total == 0 { 0. } else { seen_n as f32 / total as f32 }))),
            )
            .child(
                div()
                    .id("files-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.files_scroll)
                    .pb(px(8.))
                    .child(section(
                        "sec-u",
                        crate::i18n::t("未暫存"),
                        un_n,
                        (un_n > 0).then(|| btn(&p, "stage-all", Bk::Normal, true, crate::i18n::t("全部 Stage")).on_click(cx.listener(|this, _, _, cx| this.stage_all(cx))).into_any_element()),
                    ))
                    .child(un_tree)
                    .when(un_n == 0, |d| d.child(div().px(px(16.)).text_size(px(12.)).text_color(p.faint).child(crate::i18n::t("（沒有）"))))
                    .child(section(
                        "sec-s",
                        crate::i18n::t("已暫存"),
                        st_n,
                        (st_n > 0).then(|| btn(&p, "unstage-all", Bk::Normal, true, crate::i18n::t("全部取消")).on_click(cx.listener(|this, _, _, cx| this.unstage_all(cx))).into_any_element()),
                    ))
                    .child(st_tree)
                    .when(st_n == 0, |d| d.child(div().px(px(16.)).text_size(px(12.)).text_color(p.faint).child(crate::i18n::t("（沒有。按 Stage 把要提交的檔案放進來）")))),
            )
            .child(
                div()
                    .key_context("CommitBox")
                    .flex()
                    .flex_col()
                    .flex_none()
                    .gap(px(6.))
                    .p(px(8.))
                    .border_t_1()
                    .border_color(p.border)
                    .on_action(cx.listener(|this, _: &Commit, w, cx| this.do_commit(false, w, cx)))
                    .on_action(cx.listener(|this, _: &CommitAndPush, w, cx| this.do_commit(true, w, cx)))
                    .child(Input::new(&self.subject))
                    .child(div().h(px(64.)).child(Textarea::new(&self.desc).h_full()))
                    .when(!err.is_empty(), |d| d.child(div().text_size(px(12.)).text_color(p.danger).child(err)))
                    .child(
                        div()
                            .id("amend")
                            .flex()
                            .items_center()
                            .gap_1()
                            .cursor_pointer()
                            .text_size(px(11.5))
                            .text_color(p.faint)
                            .child(if amend { "☑" } else { "☐" })
                            .child(crate::i18n::t("Amend（改寫上一個 commit）"))
                            .on_click(cx.listener(|this, _, w, cx| this.toggle_amend(w, cx))),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap(px(6.))
                            .child(dim(btn(&p, "commit-push", Bk::Normal, false, crate::i18n::t("Commit 並 Push")).flex_1().min_w(px(110.)).tooltip_text("Ctrl+Shift+Enter").when(!committing, |b| b.on_click(cx.listener(|this, _, w, cx| this.do_commit(true, w, cx)))), committing))
                            .child(dim(btn(&p, "commit", Bk::Primary, false, if amend { "Amend" } else { "Commit" }).flex_1().min_w(px(110.)).tooltip_text("Ctrl+Enter").when(!committing, |b| b.on_click(cx.listener(|this, _, w, cx| this.do_commit(false, w, cx)))), committing)),
                    ),
            );
        let review = on_split(div().id("review").flex().flex_1().min_h_0(), SplitKind::Files, cx)
            .child(left)
            .child(splitter(&p, "split-files", SplitKind::Files, true))
            .child(
                div()
                    .id("nav-area")
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
                    .on_action(cx.listener(|this, _: &StageToggle, _, cx| this.stage_toggle(cx)))
                    .child(diff_side),
            );
        div().flex().flex_col().flex_1().min_h_0().when_some(banner, |d, b| d.child(b)).child(review).into_any_element()
    }
}
