//! A file tree with status letters and reviewed marks.

use std::collections::HashSet;

use gpui_kit::{
    AnyElement, ClickEvent, Context, InteractiveElement as _, IntoElement, MouseButton,
    ParentElement as _, SharedString, StatefulInteractiveElement as _, Styled as _, div,
    prelude::FluentBuilder as _, px, uniform_list,
};

use crate::app::GitTabs;
use crate::model::*;
use crate::tree::{Row, TreeFile, tree_rows};
use crate::ui::*;

/// What a tree line needs to know about its list: the folded folders, the open file, the files
/// picked together, and whether the line fills the width of the list.
#[derive(Clone, Copy)]
pub struct RowState<'a> {
    pub collapsed: &'a HashSet<String>,
    pub selected: Option<&'a str>,
    pub picked: &'a HashSet<String>,
    pub wide: bool,
}

/// Every line of the Changes list has this height: the list only draws what is on screen,
/// and that needs equal lines.
pub const ROW_H: f32 = 28.;

impl GitTabs {
    pub fn file_menu(&self, list: ListKind, path: &str) -> Vec<MenuItem> {
        let Some(r) = self.repo() else { return vec![] };
        let f = r.status.iter().find(|x| x.path == path).cloned();
        let untracked = f.as_ref().is_some_and(|f| f.untracked);
        let conflicted = f.as_ref().is_some_and(|f| f.conflicted);
        let (a, b, c, d, e, g) = (
            path.to_string(),
            path.to_string(),
            path.to_string(),
            path.to_string(),
            path.to_string(),
            path.to_string(),
        );
        let mut items = Vec::new();
        if list == ListKind::U {
            items.push(MenuItem::new("Stage", move |t, _, cx| {
                t.stage_files(vec![a.clone()], cx)
            }));
            if !conflicted {
                items.push(
                    MenuItem::new(
                        if untracked {
                            crate::i18n::t("刪除這個新檔案")
                        } else {
                            crate::i18n::t("Discard 修改")
                        },
                        move |t, _, cx| t.discard_files(vec![b.clone()], cx),
                    )
                    .danger(),
                );
            }
        } else {
            items.push(MenuItem::new(
                crate::i18n::t("取消 Stage"),
                move |t, _, cx| t.unstage_files(vec![a.clone()], cx),
            ));
        }
        items.push(MenuItem::sep());
        items.push(MenuItem::new(
            crate::i18n::t("用編輯器開啟"),
            move |t, _, cx| t.open_file(&c, cx),
        ));
        if !untracked {
            items.push(MenuItem::new(
                crate::i18n::t("這個檔案的 history"),
                move |t, _, cx| t.file_history(d.clone(), cx),
            ));
            items.push(MenuItem::new("Blame", move |t, _, cx| {
                t.open_blame(e.clone(), None, cx)
            }));
        }
        items.push(MenuItem::new(
            crate::i18n::t("複製路徑"),
            move |t, _, cx| t.copy(g.clone(), crate::i18n::t("路徑"), cx),
        ));
        items
    }

    /// The menu for several files picked together: stage (or unstage) them all, or discard them.
    pub fn multi_menu(&self, list: ListKind, paths: Vec<String>) -> Vec<MenuItem> {
        let n = paths.len();
        let conflicted: HashSet<&str> = self
            .repo()
            .map(|r| {
                r.status
                    .iter()
                    .filter(|f| f.conflicted)
                    .map(|f| f.path.as_str())
                    .collect()
            })
            .unwrap_or_default();
        let others: Vec<String> = paths
            .iter()
            .filter(|p| !conflicted.contains(p.as_str()))
            .cloned()
            .collect();
        let mut items = Vec::new();
        if list == ListKind::U {
            let a = others.clone();
            items.push(MenuItem::new(
                crate::i18n::tr("Stage {} 個檔案", &[], &[n.to_string()]),
                move |t, _, cx| t.stage_files(a.clone(), cx),
            ));
            let b = others.clone();
            items.push(
                MenuItem::new(
                    crate::i18n::tr("Discard {} 個檔案的修改", &[], &[b.len().to_string()]),
                    move |t, _, cx| t.discard_files(b.clone(), cx),
                )
                .danger(),
            );
        } else {
            let a = paths.clone();
            items.push(MenuItem::new(
                crate::i18n::tr("取消 Stage {} 個檔案", &[], &[n.to_string()]),
                move |t, _, cx| t.unstage_files(a.clone(), cx),
            ));
        }
        items
    }

    /// The folders of one list that are folded.
    fn collapsed_dirs(&self, kind: ListKind) -> HashSet<String> {
        self.collapsed
            .iter()
            .filter_map(|k| {
                k.strip_prefix(&format!("{}:", kind.tag()))
                    .map(String::from)
            })
            .collect()
    }

    /// `files` carries the status letter of each file; `selected` is the open file in this list.
    pub fn render_filetree(
        &self,
        kind: ListKind,
        files: &[TreeFile],
        selected: Option<&str>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let collapsed = self.collapsed_dirs(kind);
        let rows = tree_rows(files, &collapsed);
        let picked = self.multi_set(kind);
        let st = RowState {
            collapsed: &collapsed,
            selected,
            picked: &picked,
            wide: false,
        };
        div()
            .flex()
            .flex_col()
            .children(
                rows.into_iter()
                    .map(|row| self.tree_row_el(kind, row, &st, cx)),
            )
            .into_any_element()
    }

    /// One line of a file tree: a folder, or a file with its status letter.
    pub fn tree_row_el(
        &self,
        kind: ListKind,
        row: Row,
        st: &RowState,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let RowState {
            collapsed,
            selected,
            picked,
            wide,
        } = *st;
        let p = self.pal;
        let repo = self.repo();
        match row {
            Row::Dir { name, depth, dir } => {
                let closed = collapsed.contains(&dir);
                let key = format!("{}:{}", kind.tag(), dir);
                div()
                    .id(SharedString::from(format!("dir-{key}")))
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .mx(px(6.))
                    .pl(px(10. + depth as f32 * 14.))
                    .pr(px(10.))
                    .h(px(ROW_H))
                    .when(wide, |d| d.flex_1().min_w_0())
                    .rounded(px(5.))
                    .whitespace_nowrap()
                    .cursor_pointer()
                    .text_color(p.muted)
                    .hover(move |s| s.bg(p.hover))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if !this.collapsed.remove(&key) {
                            this.collapsed.insert(key.clone());
                        }
                        cx.notify();
                    }))
                    .child(icon(
                        if closed {
                            Ico::ChevronRight
                        } else {
                            Ico::ChevronDown
                        },
                        12.,
                        p.muted,
                    ))
                    .child(div().overflow_hidden().text_ellipsis().child(name))
                    .into_any_element()
            }
            Row::File {
                path,
                status,
                name,
                depth,
            } => {
                let in_multi = picked.contains(&path);
                let on = in_multi || (picked.is_empty() && selected == Some(path.as_str()));
                let seen = repo.is_some_and(|r| self.is_seen(r, kind, &path));
                let (p1, p2) = (path.clone(), path.clone());
                div()
                    .id(SharedString::from(format!("f-{}:{}", kind.tag(), path)))
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .mx(px(6.))
                    .pl(px(10. + depth as f32 * 14.))
                    .pr(px(10.))
                    .h(px(ROW_H))
                    .when(wide, |d| d.flex_1().min_w_0())
                    .rounded(px(5.))
                    .whitespace_nowrap()
                    .cursor_pointer()
                    .bg(if on {
                        p.accent_bg
                    } else {
                        gpui_kit::Hsla::transparent_black()
                    })
                    .text_color(if on { p.accent_tx } else { p.text })
                    .hover(move |s| s.bg(if on { p.accent_bg } else { p.hover }))
                    .on_click(cx.listener(move |this, ev: &ClickEvent, w, cx| {
                        // The arrow keys and S act on the file list, so it takes the focus.
                        this.nav_focus.focus(w, cx);
                        let m = ev.modifiers();
                        if matches!(kind, ListKind::U | ListKind::S) {
                            if m.control || m.platform {
                                this.toggle_multi(kind, p1.clone(), cx);
                                return;
                            }
                            if m.shift {
                                this.range_multi(kind, p1.clone(), cx);
                                return;
                            }
                        }
                        this.select(kind, p1.clone(), cx);
                        if ev.click_count() >= 2 {
                            match kind {
                                ListKind::U => this.stage_files(vec![p1.clone()], cx),
                                ListKind::S => this.unstage_files(vec![p1.clone()], cx),
                                _ => {}
                            }
                        }
                    }))
                    .when(matches!(kind, ListKind::U | ListKind::S), |d| {
                        d.on_mouse_down(
                            MouseButton::Right,
                            cx.listener(move |this, ev: &gpui_kit::MouseDownEvent, _, cx| {
                                let picked = this.multi_paths(kind);
                                let items = if picked.len() > 1 && picked.contains(&p2) {
                                    this.multi_menu(kind, picked)
                                } else {
                                    this.multi = None;
                                    this.file_menu(kind, &p2)
                                };
                                this.open_menu(ev.position, items, false, cx);
                            }),
                        )
                    })
                    .child(st_badge(&p, status))
                    .child(
                        div()
                            .overflow_hidden()
                            .text_ellipsis()
                            .text_color(if seen {
                                p.faint
                            } else if on {
                                p.accent_tx
                            } else {
                                p.text
                            })
                            .when(seen, |d| d.line_through())
                            .child(name),
                    )
                    .when(seen, |d| {
                        d.child(
                            div()
                                .ml_auto()
                                .text_size(px(12.))
                                .text_color(p.ok)
                                .child("✓"),
                        )
                    })
                    .into_any_element()
            }
        }
    }
}

/// One line of the Changes list: a section title, a folder or file of a section, or a note.
#[derive(Clone)]
pub enum ChangeItem {
    Head(ListKind, usize),
    Row(ListKind, Row),
    Note(ListKind),
}

impl GitTabs {
    /// The unstaged and staged sections as one flat list of lines.
    pub fn change_items(&self, un: &[TreeFile], st: &[TreeFile]) -> Vec<ChangeItem> {
        let mut out = Vec::with_capacity(un.len() + st.len() + 8);
        for (kind, files) in [(ListKind::U, un), (ListKind::S, st)] {
            out.push(ChangeItem::Head(kind, files.len()));
            out.extend(
                tree_rows(files, &self.collapsed_dirs(kind))
                    .into_iter()
                    .map(|r| ChangeItem::Row(kind, r)),
            );
            if files.is_empty() {
                out.push(ChangeItem::Note(kind));
            }
        }
        out
    }

    /// Scrolls the Changes list so the file is in view (the arrow keys move the selection).
    pub fn scroll_to_change(&self, list: ListKind, path: &str) {
        let Some(r) = self.repo() else { return };
        let tf = |files: Vec<&gitcore::parse::StatusEntry>| -> Vec<TreeFile> {
            files
                .into_iter()
                .map(|f| TreeFile {
                    path: f.path.clone(),
                    status: '.',
                })
                .collect()
        };
        let items = self.change_items(&tf(r.unstaged()), &tf(r.staged()));
        let ix = items.iter().position(|i| {
            matches!(i, ChangeItem::Row(k, Row::File { path: p, .. }) if *k == list && p == path)
        });
        if let Some(ix) = ix {
            self.files_scroll
                .scroll_to_item(ix, gpui_kit::ScrollStrategy::Nearest);
        }
    }

    /// The lines are built only for the part in view, so thousands of files stay quick.
    pub fn render_change_list(
        &self,
        items: Vec<ChangeItem>,
        cx: &mut Context<Self>,
    ) -> gpui_kit::UniformList {
        let items = std::sync::Arc::new(items);
        uniform_list(
            "changes-list",
            items.len(),
            cx.processor(move |this, range: std::ops::Range<usize>, _, cx| {
                let p = this.pal;
                let collapsed = [
                    (ListKind::U, this.collapsed_dirs(ListKind::U)),
                    (ListKind::S, this.collapsed_dirs(ListKind::S)),
                ];
                let picked = [this.multi_set(ListKind::U), this.multi_set(ListKind::S)];
                let sel = this.repo().and_then(|r| r.sel.clone());
                range
                    .map(|ix| match &items[ix] {
                        ChangeItem::Head(kind, n) => {
                            let (kind, n) = (*kind, *n);
                            let (id, title) = if kind == ListKind::U {
                                ("sec-u", crate::i18n::t("未暫存"))
                            } else {
                                ("sec-s", crate::i18n::t("已暫存"))
                            };
                            div()
                                .id(id)
                                .flex()
                                .items_center()
                                .gap(px(6.))
                                .h(px(ROW_H))
                                .px(px(10.))
                                .text_size(px(12.))
                                .text_color(p.muted)
                                .child(div().child(title))
                                .child(sub(&p, n.to_string()))
                                .child(div().flex_1())
                                .when(n > 0, |d| {
                                    if kind == ListKind::U {
                                        d.child(
                                            btn(
                                                &p,
                                                "stage-all",
                                                Bk::Normal,
                                                true,
                                                crate::i18n::t("全部 Stage"),
                                            )
                                            .on_click(
                                                cx.listener(|this, _, _, cx| this.stage_all(cx)),
                                            ),
                                        )
                                    } else {
                                        d.child(
                                            btn(
                                                &p,
                                                "unstage-all",
                                                Bk::Normal,
                                                true,
                                                crate::i18n::t("全部取消"),
                                            )
                                            .on_click(
                                                cx.listener(|this, _, _, cx| this.unstage_all(cx)),
                                            ),
                                        )
                                    }
                                })
                                .into_any_element()
                        }
                        ChangeItem::Row(kind, row) => {
                            let k = if *kind == ListKind::U { 0 } else { 1 };
                            let selected = sel
                                .as_ref()
                                .filter(|s| s.list == *kind)
                                .map(|s| s.path.as_str());
                            div()
                                .w_full()
                                .flex()
                                .child(this.tree_row_el(
                                    *kind,
                                    row.clone(),
                                    &RowState {
                                        collapsed: &collapsed[k].1,
                                        selected,
                                        picked: &picked[k],
                                        wide: true,
                                    },
                                    cx,
                                ))
                                .into_any_element()
                        }
                        ChangeItem::Note(kind) => div()
                            .h(px(ROW_H))
                            .px(px(16.))
                            .flex()
                            .items_center()
                            .text_size(px(12.))
                            .text_color(p.faint)
                            .child(if *kind == ListKind::U {
                                crate::i18n::t("（沒有）")
                            } else {
                                crate::i18n::t("（沒有。按 Stage 把要 commit 的檔案放進來）")
                            })
                            .into_any_element(),
                    })
                    .collect::<Vec<_>>()
            }),
        )
    }
}
