//! Rendering. `root` lays out the window the way the Vue build's App.vue does:
//! tab bar, toolbar, a sidebar next to the content, and the command log at the bottom.

mod changes;
mod diffpane;
mod filetree;
mod history;
mod overlay;
mod range;
mod reflog;
mod settings;
mod sidebar;
mod tabs;
mod toolbar;

use gpui_kit::{
    AppContext as _, AnyElement, Context, DragMoveEvent, InteractiveElement as _, IntoElement, KeyBinding, MouseButton, ParentElement as _, Render,
    StatefulInteractiveElement as _, Styled as _, Window, div, prelude::FluentBuilder as _, px,
};

use crate::app::*;
use crate::model::*;
use crate::theme::Pal;
use crate::ui::*;

pub fn bind_keys(cx: &mut gpui_kit::App) {
    cx.bind_keys([
        KeyBinding::new("f5", Refresh, Some("GitTabs")),
        KeyBinding::new("ctrl-r", Refresh, Some("GitTabs")),
        KeyBinding::new("ctrl-k", OpenPalette, Some("GitTabs")),
        KeyBinding::new("ctrl-tab", NextTab, Some("GitTabs")),
        KeyBinding::new("ctrl-shift-tab", PreviousTab, Some("GitTabs")),
        KeyBinding::new("escape", CloseOverlay, Some("GitTabs")),
        KeyBinding::new("ctrl-enter", Commit, Some("CommitBox")),
        KeyBinding::new("ctrl-shift-enter", CommitAndPush, Some("CommitBox")),
        KeyBinding::new("down", NextFile, Some("Nav")),
        KeyBinding::new("up", PreviousFile, Some("Nav")),
        KeyBinding::new("right", NextHunk, Some("Nav")),
        KeyBinding::new("left", PreviousHunk, Some("Nav")),
        KeyBinding::new("enter", ToggleSeenAction, Some("Nav")),
        KeyBinding::new("s", StageToggle, Some("Nav")),
        KeyBinding::new("down", PaletteDown, Some("Palette")),
        KeyBinding::new("up", PaletteUp, Some("Palette")),
        KeyBinding::new("enter", PaletteRun, Some("Palette")),
        KeyBinding::new("enter", ModalEnter, Some("Modal")),
    ]);
    cx.bind_keys([
        KeyBinding::new("ctrl-1", Tab1, Some("GitTabs")),
        KeyBinding::new("ctrl-2", Tab2, Some("GitTabs")),
        KeyBinding::new("ctrl-3", Tab3, Some("GitTabs")),
        KeyBinding::new("ctrl-4", Tab4, Some("GitTabs")),
        KeyBinding::new("ctrl-5", Tab5, Some("GitTabs")),
        KeyBinding::new("ctrl-6", Tab6, Some("GitTabs")),
        KeyBinding::new("ctrl-7", Tab7, Some("GitTabs")),
        KeyBinding::new("ctrl-8", Tab8, Some("GitTabs")),
        KeyBinding::new("ctrl-9", Tab9, Some("GitTabs")),
    ]);
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SplitKind {
    Side,
    Files,
    HistTop,
    HistFiles,
}

#[derive(Clone)]
pub struct SplitDrag(pub SplitKind);

struct Ghost;
impl Render for Ghost {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_0()
    }
}

/// A draggable divider. `vertical` is a thin vertical bar (resizes columns); otherwise a
/// horizontal bar (resizes rows).
pub fn splitter(p: &Pal, id: &'static str, kind: SplitKind, vertical: bool) -> impl IntoElement {
    let line = p.border;
    let hot = p.accent;
    div()
        .id(id)
        .flex_none()
        .when(vertical, |d| d.w(px(5.)).h_full().cursor_col_resize())
        .when(!vertical, |d| d.h(px(5.)).w_full().cursor_row_resize())
        .flex()
        .items_center()
        .justify_center()
        .hover(move |s| s.bg(hot.opacity(0.25)))
        .child(div().when(vertical, |d| d.w(px(1.)).h_full()).when(!vertical, |d| d.h(px(1.)).w_full()).bg(line))
        .on_drag(SplitDrag(kind), |_, _, _, cx| cx.new(|_| Ghost))
}

/// Registers a container that turns a splitter drag into a new size for `kind`.
pub fn on_split<E: gpui_kit::InteractiveElement>(e: E, kind: SplitKind, cx: &mut Context<GitTabs>) -> E {
    e.on_drag_move::<SplitDrag>(cx.listener(move |this, ev: &DragMoveEvent<SplitDrag>, _, cx| {
        if ev.drag(cx).0 != kind {
            return;
        }
        let (px_pos, origin) = match kind {
            SplitKind::HistTop => (f32::from(ev.event.position.y), f32::from(ev.bounds.origin.y)),
            _ => (f32::from(ev.event.position.x), f32::from(ev.bounds.origin.x)),
        };
        let v = px_pos - origin;
        let s = &mut this.sizes;
        match kind {
            SplitKind::Side => s.side_bar = v.clamp(150., 420.),
            SplitKind::Files => s.files = v.clamp(180., 700.),
            SplitKind::HistTop => s.hist_top = v.clamp(120., 900.),
            SplitKind::HistFiles => s.hist_files = v.clamp(160., 700.),
        }
        this.sizes_dirty = true;
        cx.notify();
    }))
}

pub fn root(this: &mut GitTabs, window: &mut Window, cx: &mut Context<GitTabs>) -> impl IntoElement {
    let p = this.pal;
    this.window_size = window.viewport_size();
    let has_repo = this.repo().is_some();
    let body: AnyElement = if has_repo {
        let content = match this.repo().map(|r| (r.view, r.error.clone())) {
            Some((_, Some(err))) => div()
                .flex()
                .items_center()
                .gap_2()
                .px_3()
                .py(px(8.))
                .bg(p.danger_bg)
                .text_color(p.danger)
                .child(div().child(err))
                .child(div().flex_1())
                .child(btn(&p, "retry", Bk::Normal, true, "重試").on_click(cx.listener(|this, _, _, cx| this.refresh_active(cx))))
                .into_any_element(),
            Some((View::Changes, _)) => this.render_changes(window, cx),
            Some((View::History, _)) => this.render_history(window, cx),
            Some((View::Range, _)) => this.render_range(window, cx),
            _ => this.render_reflog(window, cx),
        };
        let side_w = this.sizes.side_bar;
        on_split(div().id("main").flex().flex_1().min_h_0().min_w_0(), SplitKind::Side, cx)
            .child(div().flex_none().w(px(side_w)).h_full().child(this.render_sidebar(window, cx)))
            .child(splitter(&p, "split-side", SplitKind::Side, true))
            .child(div().flex_1().min_w_0().min_h_0().flex().flex_col().bg(p.panel).child(content))
            .into_any_element()
    } else {
        div()
            .flex_1()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(10.))
            .bg(p.panel)
            .child(div().size(px(72.)).rounded(px(16.)).bg(p.accent).flex().items_center().justify_center().text_color(gpui_kit::white()).text_size(px(34.)).child("⑂"))
            .child(div().text_size(px(22.)).font_weight(gpui_kit::FontWeight::SEMIBOLD).child("git-tabs"))
            .child(muted(&p, "每個 repo 一個分頁。先加入你的 repo。"))
            .child(btn(&p, "welcome-add", Bk::Primary, false, "＋ 加入 repo").on_click(cx.listener(|this, _, w, cx| this.open_add_repo(w, cx))))
            .child(div().flex().items_center().gap_1().child(sub(&p, "之後可以用")).child(kbd(&p, "Ctrl")).child(sub(&p, "+")).child(kbd(&p, "K")).child(sub(&p, "搜尋所有指令")))
            .into_any_element()
    };
    div()
        .id("app")
        .key_context("GitTabs")
        .relative()
        .size_full()
        .flex()
        .flex_col()
        .bg(p.bg)
        .text_color(p.text)
        .text_size(px(13.))
        .font_family("Segoe UI")
        .on_action(cx.listener(|this, _: &Refresh, _, cx| this.refresh_active(cx)))
        .on_action(cx.listener(|this, _: &OpenPalette, w, cx| {
            if this.palette.is_some() {
                this.close_palette(w, cx);
            } else {
                this.open_palette(w, cx);
            }
        }))
        .on_action(cx.listener(|this, _: &Tab1, _, cx| this.switch_tab(0, cx)))
        .on_action(cx.listener(|this, _: &Tab2, _, cx| this.switch_tab(1, cx)))
        .on_action(cx.listener(|this, _: &Tab3, _, cx| this.switch_tab(2, cx)))
        .on_action(cx.listener(|this, _: &Tab4, _, cx| this.switch_tab(3, cx)))
        .on_action(cx.listener(|this, _: &Tab5, _, cx| this.switch_tab(4, cx)))
        .on_action(cx.listener(|this, _: &Tab6, _, cx| this.switch_tab(5, cx)))
        .on_action(cx.listener(|this, _: &Tab7, _, cx| this.switch_tab(6, cx)))
        .on_action(cx.listener(|this, _: &Tab8, _, cx| this.switch_tab(7, cx)))
        .on_action(cx.listener(|this, _: &Tab9, _, cx| this.switch_tab(this.repos.len().saturating_sub(1), cx)))
        .on_action(cx.listener(|this, _: &NextTab, _, cx| {
            let n = this.repos.len();
            if n > 0 {
                this.switch_tab((this.active + 1) % n, cx);
            }
        }))
        .on_action(cx.listener(|this, _: &PreviousTab, _, cx| {
            let n = this.repos.len();
            if n > 0 {
                this.switch_tab((this.active + n - 1) % n, cx);
            }
        }))
        .on_action(cx.listener(|this, _: &CloseOverlay, w, cx| {
            if this.menu.is_some() {
                this.close_menu(cx);
            } else if this.palette.is_some() {
                this.close_palette(w, cx);
            } else if this.modal.is_some() {
                this.close_modal(w, cx);
            } else {
                this.nav_focus.focus(w, cx);
            }
        }))
        .on_mouse_up(MouseButton::Left, cx.listener(|this, _, _, _| {
            if this.sizes_dirty {
                this.sizes_dirty = false;
                this.save_prefs();
            }
        }))
        .child(this.render_tabs(window, cx))
        .when(has_repo, |d| d.child(this.render_toolbar(window, cx)))
        .child(body)
        .child(this.render_log_bar(cx))
        .child(this.render_overlays(window, cx))
}
