//! A file tree with status letters and reviewed marks (FileTree.vue).

use std::collections::HashSet;

use gpui_kit::{
    AnyElement, ClickEvent, Context, InteractiveElement as _, IntoElement, MouseButton, ParentElement as _, SharedString,
    StatefulInteractiveElement as _, Styled as _, div, prelude::FluentBuilder as _, px,
};

use crate::app::GitTabs;
use crate::model::*;
use crate::tree::{Row, TreeFile, tree_rows};
use crate::ui::*;

impl GitTabs {
    pub fn file_menu(&self, list: ListKind, path: &str) -> Vec<MenuItem> {
        let Some(r) = self.repo() else { return vec![] };
        let f = r.status.iter().find(|x| x.path == path).cloned();
        let untracked = f.as_ref().is_some_and(|f| f.untracked);
        let conflicted = f.as_ref().is_some_and(|f| f.conflicted);
        let (a, b, c, d, e, g) = (path.to_string(), path.to_string(), path.to_string(), path.to_string(), path.to_string(), path.to_string());
        let mut items = Vec::new();
        if list == ListKind::U {
            items.push(MenuItem::new("Stage", move |t, _, cx| t.stage_files(vec![a.clone()], cx)));
            if !conflicted {
                items.push(MenuItem::new(if untracked { crate::i18n::t("刪除這個新檔案…") } else { crate::i18n::t("捨棄修改…") }, move |t, _, cx| t.discard_files(vec![b.clone()], cx)).danger());
            }
        } else {
            items.push(MenuItem::new(crate::i18n::t("取消 Stage"), move |t, _, cx| t.unstage_files(vec![a.clone()], cx)));
        }
        items.push(MenuItem::sep());
        items.push(MenuItem::new(crate::i18n::t("用編輯器開啟"), move |t, _, cx| t.open_file(&c, cx)));
        if !untracked {
            items.push(MenuItem::new(crate::i18n::t("這個檔案的歷史"), move |t, _, cx| t.file_history(d.clone(), cx)));
            items.push(MenuItem::new("Blame", move |t, _, cx| t.open_blame(e.clone(), None, cx)));
        }
        items.push(MenuItem::new(crate::i18n::t("複製路徑"), move |t, _, cx| t.copy(g.clone(), crate::i18n::t("路徑"), cx)));
        items
    }

    /// `files` carries the status letter of each file; `selected` is the open file in this list.
    pub fn render_filetree(&self, kind: ListKind, files: &[TreeFile], selected: Option<&str>, cx: &mut Context<Self>) -> AnyElement {
        let p = self.pal;
        let collapsed: HashSet<String> = self.collapsed.iter().filter_map(|k| k.strip_prefix(&format!("{}:", kind.tag())).map(String::from)).collect();
        let rows = tree_rows(files, &collapsed);
        let repo = self.repo();
        div()
            .flex()
            .flex_col()
            .children(rows.into_iter().map(|row| match row {
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
                        .min_h(px(24.))
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
                        .child(div().w(px(10.)).flex_none().text_size(px(10.)).child(if closed { "▸" } else { "▾" }))
                        .child(div().overflow_hidden().text_ellipsis().child(name))
                        .into_any_element()
                }
                Row::File { path, status, name, depth } => {
                    let on = selected == Some(path.as_str());
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
                        .min_h(px(24.))
                        .rounded(px(5.))
                        .whitespace_nowrap()
                        .cursor_pointer()
                        .bg(if on { p.accent_bg } else { gpui_kit::Hsla::transparent_black() })
                        .text_color(if on { p.accent_tx } else { p.text })
                        .hover(move |s| s.bg(if on { p.accent_bg } else { p.hover }))
                        .on_click(cx.listener(move |this, ev: &ClickEvent, _, cx| {
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
                                    let items = this.file_menu(kind, &p2);
                                    this.open_menu(ev.position, items, false, cx);
                                }),
                            )
                        })
                        .child(st_badge(&p, status))
                        .child(div().overflow_hidden().text_ellipsis().text_color(if seen { p.faint } else if on { p.accent_tx } else { p.text }).when(seen, |d| d.line_through()).child(name))
                        .when(seen, |d| d.child(div().ml_auto().text_size(px(12.)).text_color(p.ok).child("✓")))
                        .into_any_element()
                }
            }))
            .into_any_element()
    }
}
