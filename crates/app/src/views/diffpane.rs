//! The diff header and body shared by the changes, history and range views
//! (DiffHead.vue and DiffView.vue).

use gitcore::ops::LineAction;
use gpui_kit::component::diff::{Diff, DiffLinePosition, DiffSide};
use gpui_kit::{
    AnyElement, ClickEvent, Context, InteractiveElement as _, IntoElement, ParentElement as _, SharedString, StatefulInteractiveElement as _,
    Styled as _, div, prelude::FluentBuilder as _, px,
};

use crate::app::GitTabs;
use crate::model::*;
use crate::ui::*;

/// What the viewer may do with the lines: stage/discard (unstaged), unstage (staged), or nothing.
#[derive(Clone, Copy, PartialEq)]
pub enum PaneMode {
    Unstaged,
    Staged,
    ReadOnly,
}

impl GitTabs {
    pub fn goto_hunk(&mut self, d: isize, cx: &mut Context<Self>) {
        let cur = self.cur_hunk;
        let Some(r) = self.repo() else { return };
        let (Some(fd), Some(sel)) = (r.diff.as_ref(), r.sel.as_ref()) else { return };
        if fd.hunks.is_empty() {
            return;
        }
        let i = (cur as isize + d).clamp(0, fd.hunks.len() as isize - 1) as usize;
        let h = &fd.hunks[i];
        let pos = if h.new_lines > 0 || fd.new_file {
            DiffLinePosition::new(SharedString::from(sel.path.clone()), DiffSide::Modified, (h.new_start as usize).max(1))
        } else {
            DiffLinePosition::new(SharedString::from(sel.path.clone()), DiffSide::Original, (h.old_start as usize).max(1))
        };
        self.cur_hunk = i;
        self.diff.update(cx, |s, cx| s.scroll_to_line(pos, cx));
        cx.notify();
    }

    fn diff_more_menu(&self) -> Vec<MenuItem> {
        let Some(r) = self.repo() else { return vec![] };
        let Some(sel) = r.sel.clone() else { return vec![] };
        let dir = sel.path.rsplit_once('/').map(|x| x.0.to_string()).unwrap_or_default();
        let full = format!("{}/{}", r.path.display(), dir).replace('/', "\\");
        let rev = if sel.list == ListKind::C { r.detail.as_ref().map(|d| d.sha.clone()) } else { None };
        let (a, b, c, d) = (sel.path.clone(), sel.path.clone(), sel.path.clone(), sel.path.clone());
        vec![
            MenuItem::new(crate::i18n::t("用編輯器開啟"), move |t, _, cx| t.open_file(&a, cx)).icon(Ico::SquarePen),
            MenuItem::new(crate::i18n::t("在檔案總管顯示資料夾"), move |t, _, cx| t.open_folder(full.clone(), cx)).icon(Ico::FolderOpen),
            MenuItem::new(crate::i18n::t("這個檔案的歷史"), move |t, _, cx| t.file_history(b.clone(), cx)).icon(Ico::FileClock),
            MenuItem::new(crate::i18n::t("Blame（逐行看是誰改的）"), move |t, _, cx| t.open_blame(c.clone(), rev.clone(), cx)).icon(Ico::ScrollText),
            MenuItem::new(crate::i18n::t("複製路徑"), move |t, _, cx| t.copy(d.clone(), crate::i18n::t("路徑"), cx)).icon(Ico::Copy),
        ]
    }

    /// The bar above a diff: file, counts, `extra` buttons, reviewed mark, layout switches.
    pub fn render_diff_head(&self, status: char, extra: Vec<AnyElement>, cx: &mut Context<Self>) -> AnyElement {
        let p = self.pal;
        let Some(r) = self.repo() else { return div().into_any_element() };
        let Some(sel) = r.sel.clone() else { return div().into_any_element() };
        let (add, del) = r.diff.as_ref().map_or((0, 0), |d| {
            d.hunks.iter().flat_map(|h| h.lines.iter()).fold((0, 0), |(a, r), l| match l.t {
                '+' => (a + 1, r),
                '-' => (a, r + 1),
                _ => (a, r),
            })
        });
        let old = r.diff.as_ref().and_then(|d| d.old_path.clone());
        let seen = self.is_seen(r, sel.list, &sel.path);
        let can_seen = sel.list != ListKind::C;
        let (side, wrap) = (self.side_by_side, self.wrap);
        div()
            .flex()
            .flex_none()
            .flex_wrap()
            .items_center()
            .gap(px(8.))
            .px(px(10.))
            .py(px(6.))
            .min_h(px(41.))
            .border_b_1()
            .border_color(p.border)
            .child(st_badge(&p, status))
            .child(
                div()
                    .min_w(px(80.))
                    .overflow_hidden()
                    .text_ellipsis()
                    .whitespace_nowrap()
                    .font_family("Cascadia Mono")
                    .when_some(old, |d, o| d.child(div().text_size(px(11.5)).text_color(p.faint).child(format!("{o} → "))))
                    .child(sel.path.clone()),
            )
            .child(div().text_size(px(12.)).text_color(p.ok).child(format!("+{add}")))
            .child(div().text_size(px(12.)).text_color(p.danger).child(format!("−{del}")))
            .child(div().flex_1())
            .children(extra)
            .when(can_seen, |d| {
                d.child(
                    btn(&p, "seen", if seen { Bk::On } else { Bk::Normal }, true, div().flex().items_center().gap_1().child(check_box(&p, seen)).child(crate::i18n::t("已看")))
                        .tooltip_text(crate::i18n::t("看完勾起來，會跳到下一個沒看的檔案（Enter）"))
                        .on_click(cx.listener(|this, _, _, cx| this.toggle_seen(true, cx))),
                )
            })
            .child(
                div()
                    .flex()
                    .flex_none()
                    .border_1()
                    .border_color(p.border_strong)
                    .rounded(px(7.))
                    .overflow_hidden()
                    .child(
                        div()
                            .id("lay-unified")
                            .px(px(10.))
                            .py(px(3.))
                            .cursor_pointer()
                            .bg(if !side { p.accent_bg } else { gpui_kit::Hsla::transparent_black() })
                            .text_color(if !side { p.accent_tx } else { p.muted })
                            .child(crate::i18n::t("單欄"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.side_by_side = false;
                                this.apply_diff_view(cx);
                                this.save_prefs();
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .id("lay-split")
                            .px(px(10.))
                            .py(px(3.))
                            .cursor_pointer()
                            .border_l_1()
                            .border_color(p.border_strong)
                            .bg(if side { p.accent_bg } else { gpui_kit::Hsla::transparent_black() })
                            .text_color(if side { p.accent_tx } else { p.muted })
                            .child(crate::i18n::t("並排"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.side_by_side = true;
                                this.apply_diff_view(cx);
                                this.save_prefs();
                                cx.notify();
                            })),
                    ),
            )
            .child(
                btn(&p, "wrap", if wrap { Bk::On } else { Bk::Normal }, true, icon(Ico::TextWrap, 14., if wrap { p.accent_tx } else { p.text })).tooltip_text(crate::i18n::t("自動換行")).on_click(cx.listener(|this, _, _, cx| {
                    this.wrap = !this.wrap;
                    this.save_prefs();
                    cx.notify();
                })),
            )
            .child(btn(&p, "more", Bk::Normal, true, icon(Ico::Ellipsis, 14., p.text)).tooltip_text(crate::i18n::t("更多")).on_click(cx.listener(|this, ev: &ClickEvent, _, cx| {
                let items = this.diff_more_menu();
                this.open_menu(ev.position(), items, true, cx);
            })))
            .into_any_element()
    }

    /// Selection bar, hunk bar and the diff itself.
    pub fn render_diff_area(&mut self, mode: PaneMode, cx: &mut Context<Self>) -> AnyElement {
        let p = self.pal;
        let Some(r) = self.repo() else { return div().into_any_element() };
        let Some(sel) = r.sel.clone() else { return empty(&p, "◇", crate::i18n::t("選一個檔案來看修改"), None) };
        let key = format!("{}:{}", sel.list.tag(), sel.path);
        let Some(fd) = r.diff.clone().filter(|_| r.diff_for == key) else { return empty(&p, "", crate::i18n::t("載入中…"), None) };
        if fd.binary {
            return empty(&p, "⬚", crate::i18n::t("二進位檔案，無法顯示文字差異"), None);
        }
        if fd.hunks.is_empty() {
            return empty(&p, "≡", crate::i18n::t("沒有內容差異"), Some(crate::i18n::t("可能只改了檔案權限，或是空檔案")));
        }
        // ---- selected lines
        let selected = self.stored_sel.clone().map(|range| crate::diffmap::selected_pairs(&fd, (range.side(), range.start()), (range.end_side(), range.end())).len());
        let selbar: Option<AnyElement> = selected.filter(|n| *n > 0).map(|n| {
            div()
                .id("selbar")
                .on_mouse_down(gpui_kit::MouseButton::Left, cx.listener(|this, _, _, _| this.bar_press = Some(std::time::Instant::now())))
                .flex()
                .flex_none()
                .items_center()
                .gap(px(8.))
                .px(px(10.))
                .py(px(6.))
                .bg(p.accent_bg)
                .text_color(p.accent_tx)
                .text_size(px(12.5))
                .child(crate::i18n::tr("已選 {n} 行", &[("n", (n).to_string())], &[]))
                .when(mode == PaneMode::Unstaged, |d| {
                    d.child(btn(&p, "sel-stage", Bk::Primary, true, div().flex().gap_1().child(crate::i18n::t("Stage 選取的行")).child(kbd(&p, "S"))).on_click(cx.listener(|this, _, _, cx| {
                        this.apply_selection(false, cx);
                    })))
                        .child(btn(&p, "sel-discard", Bk::Normal, true, crate::i18n::t("捨棄選取的行")).on_click(cx.listener(|this, _, _, cx| {
                            this.apply_selection(true, cx);
                        })))
                })
                .when(mode == PaneMode::Staged, |d| {
                    d.child(btn(&p, "sel-unstage", Bk::Primary, true, div().flex().gap_1().child(crate::i18n::t("取消 Stage 選取的行")).child(kbd(&p, "S"))).on_click(cx.listener(|this, _, _, cx| {
                        this.apply_selection(false, cx);
                    })))
                })
                .child(btn(&p, "sel-clear", Bk::Ghost, true, crate::i18n::t("清除選取")).on_click(cx.listener(|this, _, _, cx| {
                    this.stored_sel = None;
                    this.diff.update(cx, |d, cx| d.set_selected_lines(None, cx));
                })))
                .into_any_element()
        });
        // ---- current hunk
        let n = fd.hunks.len();
        let cur = self.cur_hunk.min(n - 1);
        // The conflict view's pseudo-hunk header comes from gitcore in Chinese.
        let header = if fd.hunks[cur].header == crate::i18n::CONFLICT_HEADER { crate::i18n::t(crate::i18n::CONFLICT_HEADER).to_string() } else { fd.hunks[cur].header.clone() };
        let hunkbar = div()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(6.))
            .px(px(10.))
            .min_h(px(28.))
            .bg(p.accent_bg)
            .text_color(p.accent_tx)
            .text_size(px(12.))
            .child(btn(&p, "hunk-prev", Bk::Ghost, true, icon(Ico::ChevronLeft, 15., p.text)).tooltip_text(crate::i18n::t("上一段改動 (←)")).on_click(cx.listener(|this, _, _, cx| this.goto_hunk(-1, cx))))
            .child(btn(&p, "hunk-next", Bk::Ghost, true, icon(Ico::ChevronRight, 15., p.text)).tooltip_text(crate::i18n::t("下一段改動 (→)")).on_click(cx.listener(|this, _, _, cx| this.goto_hunk(1, cx))))
            .child(div().flex_none().child(crate::i18n::tr("第 {}/{n} 段", &[("n", (n).to_string())], &[(cur + 1).to_string()])))
            .child(div().flex_1().min_w_0().overflow_hidden().text_ellipsis().whitespace_nowrap().font_family("Cascadia Mono").child(header))
            .when(mode == PaneMode::Unstaged, |d| {
                d.child(btn(&p, "hunk-stage", Bk::Normal, true, crate::i18n::t("Stage 這段")).on_click(cx.listener(move |this, _, _, cx| this.apply_hunk(cur, LineAction::Stage, cx))))
                    .child(btn(&p, "hunk-discard", Bk::Normal, true, crate::i18n::t("捨棄這段")).on_click(cx.listener(move |this, _, _, cx| this.apply_hunk(cur, LineAction::Discard, cx))))
            })
            .when(mode == PaneMode::Staged, |d| d.child(btn(&p, "hunk-unstage", Bk::Normal, true, crate::i18n::t("取消 Stage 這段")).on_click(cx.listener(move |this, _, _, cx| this.apply_hunk(cur, LineAction::Unstage, cx)))));
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .when_some(selbar, |d, s| d.child(s))
            .child(hunkbar)
            .child(div().flex_1().min_h_0().child(Diff::new(&self.diff).line_number(true).header_visible(false).soft_wrap(self.wrap)))
            .into_any_element()
    }

    /// The key-hint line under a diff.
    pub fn render_diff_hint(&self, with_stage: bool) -> AnyElement {
        let p = self.pal;
        let item = |k: &str, t: &str| div().flex().items_center().gap_1().child(kbd(&p, k)).child(t.to_string());
        div()
            .flex()
            .flex_none()
            .flex_wrap()
            .items_center()
            .gap(px(12.))
            .px(px(10.))
            .py(px(4.))
            .border_t_1()
            .border_color(p.border)
            .text_size(px(11.5))
            .text_color(p.faint)
            .child(item("↑/↓", crate::i18n::t("上/下一個檔案")))
            .child(item("←/→", crate::i18n::t("上/下一段改動")))
            .when(with_stage, |d| d.child(item("S", "Stage")))
            .child(item("Enter", crate::i18n::t("已看")))
            .when(with_stage, |d| d.child(crate::i18n::t("在差異裡點行號選取單行，拖曳或 Shift 選多行")))
            .child(div().flex_1())
            .when(self.side_by_side, |d| d.child(crate::i18n::t("並排")))
            .into_any_element()
    }
}
