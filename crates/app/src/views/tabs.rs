//! The tab bar (TabBar.vue).

use gpui_kit::{
    AnyElement, Context, InteractiveElement as _, IntoElement, MouseButton, ParentElement as _, Render, ScrollEvent, SharedString,
    StatefulInteractiveElement as _, Styled as _, Window, div, point, prelude::FluentBuilder as _, px,
};

use gpui_kit::{AppContext as _, Hsla};

use crate::app::GitTabs;
use crate::model::*;
use crate::ui::*;

#[derive(Clone)]
struct DraggedTab(usize);

struct TabGhost(String, Hsla);
impl Render for TabGhost {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().px_3().py_1().rounded(px(8.)).bg(self.1).opacity(0.85).child(self.0.clone())
    }
}

impl GitTabs {
    pub fn tab_menu(&self, i: usize) -> Vec<MenuItem> {
        let Some(r) = self.repos.get(i) else { return vec![] };
        let (path, ps) = (r.path.clone(), r.path.display().to_string());
        let (ps2, ps3) = (ps.clone(), ps.clone());
        vec![
            MenuItem::new(crate::i18n::t("重新整理"), move |t, _, cx| t.refresh(&path, cx)).icon("⟳"),
            MenuItem::new(crate::i18n::t("在檔案總管開啟"), move |t, _, cx| t.open_folder(ps.clone(), cx)).icon("📂"),
            MenuItem::new(crate::i18n::t("開終端機"), move |t, _, cx| t.open_terminal(ps2.clone(), cx)).icon("⌨"),
            MenuItem::new(crate::i18n::t("複製路徑"), move |t, _, cx| t.copy(ps3.clone(), crate::i18n::t("路徑"), cx)).icon("⧉"),
            MenuItem::sep(),
            MenuItem::new(crate::i18n::t("關閉分頁"), move |t, _, cx| t.close_repo(i, cx)).icon("×"),
        ]
    }

    pub fn render_tabs(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let p = self.pal;
        let active = self.active;
        let tabs: Vec<AnyElement> = self
            .repos
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let on = i == active;
                let lead: Option<AnyElement> = if r.busy.is_some() {
                    Some(div().size(px(10.)).rounded_full().border_2().border_color(p.accent).into_any_element())
                } else if r.error.is_some() {
                    Some(div().text_color(p.danger).font_weight(gpui_kit::FontWeight::BOLD).child("!").into_any_element())
                } else if r.changes() > 0 {
                    Some(div().size(px(7.)).flex_none().rounded_full().bg(gpui_kit::rgb(0xe8a33a)).into_any_element())
                } else {
                    None
                };
                let branch = r.summary.as_ref().map(|s| {
                    let mut t = s.branch.clone().unwrap_or_else(|| "(detached)".into());
                    if s.ahead > 0 {
                        t.push_str(&format!(" ↑{}", s.ahead));
                    }
                    if s.behind > 0 {
                        t.push_str(&format!(" ↓{}", s.behind));
                    }
                    t
                });
                let name = r.name.clone();
                div()
                    .id(SharedString::from(format!("tab-{i}")))
                    .group("tab")
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap(px(7.))
                    .h(px(30.))
                    .pl(px(12.))
                    .pr(px(8.))
                    .max_w(px(300.))
                    .rounded_t(px(8.))
                    .whitespace_nowrap()
                    .cursor_pointer()
                    .bg(if on { p.panel } else { gpui_kit::Hsla::transparent_black() })
                    .text_color(if on { p.text } else { p.muted })
                    .hover(move |s| s.bg(if on { p.panel } else { p.hover }))
                    .on_click(cx.listener(move |this, _, _, cx| this.switch_tab(i, cx)))
                    .on_mouse_down(MouseButton::Middle, cx.listener(move |this, _, _, cx| this.close_repo(i, cx)))
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(move |this, ev: &gpui_kit::MouseDownEvent, _, cx| {
                            let items = this.tab_menu(i);
                            this.open_menu(ev.position, items, false, cx);
                        }),
                    )
                    .on_drag(DraggedTab(i), {
                        let n = name.clone();
                        move |_, _, _, cx| cx.new(|_| TabGhost(n.clone(), p.panel))
                    })
                    .on_drop(cx.listener(move |this, d: &DraggedTab, _, cx| this.move_tab(d.0, i, cx)))
                    .when_some(lead, |d, l| d.child(l))
                    .child(div().overflow_hidden().text_ellipsis().child(name))
                    .when_some(branch, |d, b| d.child(div().text_size(px(11.5)).text_color(p.faint).whitespace_nowrap().child(b)))
                    .child(
                        div()
                            .id(SharedString::from(format!("x-{i}")))
                            .px(px(3.))
                            .rounded(px(4.))
                            .text_size(px(14.))
                            .text_color(p.faint)
                            .cursor_pointer()
                            .opacity(if on { 1. } else { 0. })
                            .group_hover("tab", |s| s.opacity(1.))
                            .hover(move |s| s.bg(p.hover).text_color(p.text))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                cx.stop_propagation();
                                this.close_repo(i, cx);
                            }))
                            .child("×"),
                    )
                    .into_any_element()
            })
            .collect();
        let plus = div()
            .id("tab-add")
            .flex_none()
            .w(px(32.))
            .h(px(30.))
            .flex()
            .items_center()
            .justify_center()
            .rounded_t(px(8.))
            .text_color(p.muted)
            .cursor_pointer()
            .hover(move |s| s.bg(p.hover))
            .on_click(cx.listener(|this, _, w, cx| this.open_add_repo(w, cx)))
            .child(icon_plus(p.muted));
        let all = div()
            .id("tab-all")
            .flex_none()
            .w(px(32.))
            .h(px(30.))
            .flex()
            .items_center()
            .justify_center()
            .rounded_t(px(8.))
            .text_color(p.muted)
            .cursor_pointer()
            .hover(move |s| s.bg(p.hover))
            .on_click(cx.listener(|this, ev: &gpui_kit::ClickEvent, _, cx| {
                let items: Vec<MenuItem> = this
                    .repos
                    .iter()
                    .enumerate()
                    .map(|(i, r)| MenuItem::new(r.name.clone(), move |t, _, cx| t.switch_tab(i, cx)).checked(i == this.active).hint(r.summary.as_ref().and_then(|s| s.branch.clone()).unwrap_or_default()))
                    .collect();
                this.open_menu(ev.position(), items, true, cx);
            }))
            .child(icon_chevron_down(p.muted));
        div()
            .flex()
            .flex_none()
            .items_end()
            .h(px(36.))
            .pr(px(4.))
            .min_w_0()
            .bg(p.bg)
            .child(
                div()
                    .id("tabs")
                    .flex()
                    .items_end()
                    .gap(px(2.))
                    .px(px(6.))
                    .min_w_0()
                    .h_full()
                    .overflow_x_scroll()
                    .track_scroll(&self.tab_scroll)
                    // A plain mouse wheel scrolls the row sideways: there is nothing to scroll vertically.
                    .on_scroll(cx.listener(|this, ev: &ScrollEvent, _, cx| {
                        let d = ev.delta.pixel_delta(px(16.));
                        let o = this.tab_scroll.offset();
                        let dx = if d.y.abs() > d.x.abs() { d.y } else { d.x };
                        this.tab_scroll.set_offset(point(o.x + dx, o.y));
                        cx.notify();
                    }))
                    .children(tabs),
            )
            .child(plus)
            .child(all)
            .into_any_element()
    }
}
