//! Context menu, dialogs, command palette, toasts and the command-log bar.

use gpui_kit::component::input::{Input, Textarea};
use gpui_kit::{
    AnyElement, Context, Hsla, InteractiveElement as _, IntoElement, MouseButton, ParentElement as _, SharedString, StatefulInteractiveElement as _,
    Styled as _, Window, div, point, prelude::FluentBuilder as _, px,
};

use crate::app::*;
use crate::model::*;
use crate::ui::*;

impl GitTabs {
    pub fn render_overlays(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let mut layers: Vec<AnyElement> = Vec::new();
        if self.modal.is_some() {
            layers.push(self.render_modal(window, cx));
        }
        if self.palette.is_some() {
            layers.push(self.render_palette(cx));
        }
        // Last of the dismissable layers, so a menu opened from inside a dialog (settings'
        // "assign to…") draws over it instead of behind.
        if self.menu.is_some() {
            layers.push(self.render_menu(cx));
        }
        layers.push(self.render_toasts(cx));
        // Absolute children are placed against their parent, so this layer must fill the window.
        div().absolute().inset_0().children(layers).into_any_element()
    }

    // ----- context menu -----

    fn render_menu(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let p = self.pal;
        let Some(m) = &self.menu else { return div().into_any_element() };
        let (ww, wh) = (f32::from(self.window_size.width), f32::from(self.window_size.height));
        // Wide enough for the longest label and hint (CJK glyphs count double), 220..420 px.
        let text_w = |t: &str| t.chars().map(|c| if c.is_ascii() { 7.0f32 } else { 13.0 }).sum::<f32>();
        let width = m.items.iter().filter(|i| !i.sep).map(|i| if i.header { text_w(&i.label) + 24. } else { text_w(&i.label) + if i.hint.is_empty() { 0. } else { text_w(&i.hint) + 16. } + 56. }).fold(220.0f32, f32::max).min(420.0);
        let height = m.items.iter().map(|i| if i.sep { 9. } else if i.header { 26. } else { 28. }).sum::<f32>() + 8.;
        let mut x = f32::from(m.pos.x);
        let mut y = f32::from(m.pos.y) + 2.;
        if m.right && x + width > ww - 8. {
            x = (x - width).max(8.);
        }
        x = x.min(ww - width - 8.).max(8.);
        y = y.min(wh - height - 8.).max(8.);
        let items: Vec<AnyElement> = m
            .items
            .iter()
            .enumerate()
            .map(|(i, it)| {
                if it.sep {
                    return div().h(px(1.)).my(px(4.)).mx(px(6.)).bg(p.border).into_any_element();
                }
                if it.header {
                    return div().px(px(12.)).pt(px(6.)).pb(px(2.)).text_size(px(11.5)).text_color(p.faint).child(it.label.clone()).into_any_element();
                }
                let act = it.on.clone();
                let off = it.disabled;
                let label = it.label.clone();
                div()
                    .id(SharedString::from(format!("mi-{i}")))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .h(px(28.))
                    .mx(px(4.))
                    .px(px(8.))
                    .rounded(px(5.))
                    .whitespace_nowrap()
                    .text_color(if it.danger { p.danger } else { p.text })
                    .when(off, |d| d.opacity(0.45))
                    .when(!off, |d| {
                        d.cursor_pointer().hover(move |s| s.bg(p.hover)).on_click(cx.listener(move |this, _, w, cx| {
                            this.menu = None;
                            cx.notify();
                            crate::debug::log("menu", || format!("pick: {label}"));
                            if let Some(a) = &act {
                                a(this, w, cx);
                            }
                        }))
                    })
                    .child(div().w(px(16.)).flex_none().flex().items_center().text_color(p.ok).child(match (it.checked, it.icon) {
                        (true, _) => "✓".into_any_element(),
                        (false, Some(i)) => icon(i, 14., p.muted).into_any_element(),
                        (false, None) => div().into_any_element(),
                    }))
                    .child(div().flex_1().overflow_hidden().text_ellipsis().child(it.label.clone()))
                    .when(!it.hint.is_empty(), |d| d.child(div().flex_none().text_size(px(11.5)).text_color(p.faint).child(it.hint.clone())))
                    .into_any_element()
            })
            .collect();
        div()
            .absolute()
            .inset_0()
            .occlude()
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| this.close_menu(cx)))
            .on_mouse_down(MouseButton::Right, cx.listener(|this, _, _, cx| this.close_menu(cx)))
            .child(
                div()
                    .absolute()
                    .left(px(x))
                    .top(px(y))
                    .w(px(width))
                    .py(px(4.))
                    .rounded(px(8.))
                    .border_1()
                    .border_color(p.border)
                    .bg(p.panel)
                    .shadow_lg()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .children(items),
            )
            .into_any_element()
    }

    // ----- dialogs -----

    /// Wraps a dialog: title, scrolling body, footer buttons.
    pub fn dialog(&self, title: AnyElement, body: AnyElement, foot: Vec<AnyElement>, wide: f32, cx: &mut Context<Self>) -> AnyElement {
        let p = self.pal;
        let ww = f32::from(self.window_size.width);
        let w = wide.min(ww * 0.94);
        div()
            .absolute()
            .inset_0()
            .occlude()
            .bg(Hsla { h: 0.6, s: 0.2, l: 0.05, a: 0.35 })
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, w, cx| this.close_modal(w, cx)))
            .child(
                div()
                    .id("dialog")
                    .key_context("Modal")
                    .track_focus(&self.modal_focus)
                    .absolute()
                    .top(gpui_kit::relative(0.10))
                    .left(px(((ww - w) / 2.).max(0.)))
                    .w(px(w))
                    .max_h(gpui_kit::relative(0.82))
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .rounded(px(12.))
                    .border_1()
                    .border_color(p.border)
                    .bg(p.panel)
                    .shadow_lg()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_action(cx.listener(|this, _: &ModalEnter, w, cx| this.modal_enter(w, cx)))
                    .child(div().px(px(16.)).pt(px(14.)).pb(px(10.)).text_size(px(15.)).font_weight(gpui_kit::FontWeight::SEMIBOLD).child(title))
                    .child(div().id("dialog-body").flex().flex_col().gap(px(10.)).px(px(16.)).pb(px(14.)).overflow_y_scroll().min_h_0().child(body))
                    .child(div().flex().flex_none().items_center().justify_end().gap(px(8.)).px(px(16.)).py(px(10.)).border_t_1().border_color(p.border).bg(p.panel2).children(foot)),
            )
            .into_any_element()
    }

    pub fn modal_enter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match &self.modal {
            Some(Modal::Confirm(m)) => {
                let on_ok = m.on_ok.clone();
                self.close_modal(window, cx);
                on_ok(self, window, cx);
            }
            Some(Modal::Prompt(_)) => self.submit_prompt(window, cx),
            _ => {}
        }
    }

    fn render_modal(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let p = self.pal;
        // Confirm/choice dialogs have no text box, so give the dialog itself the focus for Enter.
        if matches!(self.modal, Some(Modal::Confirm(_) | Modal::Choice(_))) && !self.modal_focus.is_focused(window) {
            let h = self.modal_focus.clone();
            window.defer(cx, move |window, cx| window.focus(&h, cx));
        }
        let cancel = btn(&p, "m-cancel", Bk::Normal, false, crate::i18n::t("取消")).on_click(cx.listener(|this, _, w, cx| this.close_modal(w, cx))).into_any_element();
        match &self.modal {
            Some(Modal::Confirm(m)) => {
                let on_ok = m.on_ok.clone();
                let ok = btn(&p, "m-ok", if m.danger { Bk::Danger } else { Bk::Primary }, false, m.ok.clone()).on_click(cx.listener(move |this, _, w, cx| {
                    this.close_modal(w, cx);
                    on_ok(this, w, cx);
                }));
                let body = div().flex().flex_col().gap_1().children(m.body.split('\n').map(|l| rich(&p, l)).collect::<Vec<_>>()).into_any_element();
                self.dialog(div().child(m.title.clone()).into_any_element(), body, vec![cancel, ok.into_any_element()], 540., cx)
            }
            Some(Modal::Choice(m)) => {
                let buttons: Vec<AnyElement> = m
                    .choices
                    .iter()
                    .map(|c| {
                        let (v, pick) = (c.value.to_string(), m.on_pick.clone());
                        let kind = if c.primary { Bk::Primary } else if c.danger { Bk::Danger } else { Bk::Normal };
                        btn(&p, SharedString::from(format!("ch-{}", c.value)), kind, false, c.label.clone())
                            .w_full()
                            .justify_start()
                            .py(px(8.))
                            .h_auto()
                            .on_click(cx.listener(move |this, _, w, cx| {
                                this.close_modal(w, cx);
                                pick(this, v.clone(), w, cx);
                            }))
                            .into_any_element()
                    })
                    .collect();
                let body = div().flex().flex_col().gap(px(10.)).child(div().child(m.body.clone())).children(buttons).into_any_element();
                self.dialog(div().child(m.title.clone()).into_any_element(), body, vec![cancel], 540., cx)
            }
            Some(Modal::Prompt(m)) => {
                let rows: Vec<AnyElement> = m
                    .fields
                    .iter()
                    .enumerate()
                    .flat_map(|(i, f)| {
                        let mut v: Vec<AnyElement> = Vec::new();
                        match f.spec.kind {
                            FieldKind::Checkbox => v.push(
                                div()
                                    .id(SharedString::from(format!("pf-{i}")))
                                    .flex()
                                    .items_center()
                                    .gap(px(6.))
                                    .cursor_pointer()
                                    .child(check_box(&p, f.checked))
                                    .child(f.spec.label.clone())
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        if let Some(Modal::Prompt(m)) = &mut this.modal
                                            && let Some(f) = m.fields.get_mut(i)
                                        {
                                            f.checked = !f.checked;
                                        }
                                        cx.notify();
                                    }))
                                    .into_any_element(),
                            ),
                            _ => v.push(
                                div()
                                    .grid()
                                    .grid_cols(1)
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(10.))
                                            .child(div().w(px(110.)).flex_none().child(f.spec.label.clone()))
                                            .child(div().flex_1().child(match (&f.input, &f.textarea) {
                                                (Some(i), _) => Input::new(i).into_any_element(),
                                                (_, Some(t)) => div().h(px(76.)).child(Textarea::new(t).h_full()).into_any_element(),
                                                _ => div().into_any_element(),
                                            })),
                                    )
                                    .into_any_element(),
                            ),
                        }
                        if let Some(e) = &f.err {
                            v.push(div().text_size(px(12.)).text_color(p.danger).child(e.clone()).into_any_element());
                        }
                        v
                    })
                    .collect();
                let ok = btn(&p, "m-ok", Bk::Primary, false, m.ok.clone()).on_click(cx.listener(|this, _, w, cx| this.submit_prompt(w, cx)));
                self.dialog(div().child(m.title.clone()).into_any_element(), div().flex().flex_col().gap(px(10.)).children(rows).into_any_element(), vec![cancel, ok.into_any_element()], 540., cx)
            }
            Some(Modal::AddRepo(_)) => self.render_add_repo(cx),
            Some(Modal::Settings(_)) => self.render_settings(window, cx),
            Some(Modal::Blame(_)) => self.render_blame(cx),
            None => div().into_any_element(),
        }
    }

    // ----- command palette -----

    fn render_palette(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let p = self.pal;
        let items = self.palette_shown();
        let sel = self.palette.as_ref().map_or(0, |x| x.sel);
        let ww = f32::from(self.window_size.width);
        let w = 620f32.min(ww * 0.92);
        let input = self.palette.as_ref().map(|x| x.input.clone());
        let rows: Vec<AnyElement> = items
            .iter()
            .enumerate()
            .map(|(i, it)| {
                let run = it.run.clone();
                div()
                    .id(SharedString::from(format!("pi-{i}")))
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .px(px(10.))
                    .py(px(7.))
                    .rounded(px(7.))
                    .cursor_pointer()
                    .bg(if i == sel { p.accent_bg } else { Hsla::transparent_black() })
                    .text_color(if i == sel { p.accent_tx } else { p.text })
                    .on_mouse_move(cx.listener(move |this, _, _, cx| {
                        if let Some(pal) = &mut this.palette
                            && pal.sel != i
                        {
                            pal.sel = i;
                            cx.notify();
                        }
                    }))
                    .on_click(cx.listener(move |this, _, w, cx| {
                        this.close_palette(w, cx);
                        run(this, w, cx);
                    }))
                    .child(div().overflow_hidden().text_ellipsis().whitespace_nowrap().child(it.title.clone()))
                    .child(div().ml_auto().flex_none().text_size(px(11.5)).text_color(p.faint).whitespace_nowrap().child(it.group.clone()))
                    .into_any_element()
            })
            .collect();
        div()
            .absolute()
            .inset_0()
            .occlude()
            .bg(Hsla { h: 0.6, s: 0.2, l: 0.05, a: 0.35 })
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, w, cx| this.close_palette(w, cx)))
            .child(
                div()
                    .id("palette")
                    .key_context("Palette")
                    .absolute()
                    .top(gpui_kit::relative(0.12))
                    .left(px(((ww - w) / 2.).max(0.)))
                    .w(px(w))
                    .overflow_hidden()
                    .rounded(px(12.))
                    .border_1()
                    .border_color(p.border)
                    .bg(p.panel)
                    .shadow_lg()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_action(cx.listener(|this, _: &PaletteDown, _, cx| this.palette_move(1, cx)))
                    .on_action(cx.listener(|this, _: &PaletteUp, _, cx| this.palette_move(-1, cx)))
                    .on_action(cx.listener(|this, _: &PaletteRun, w, cx| this.run_palette(w, cx)))
                    .child(div().p(px(10.)).border_b_1().border_color(p.border).when_some(input, |d, i| d.child(Input::new(&i))))
                    .child(
                        div()
                            .id("palette-items")
                            .max_h(px((f32::from(self.window_size.height) * 0.5).max(160.)))
                            .overflow_y_scroll()
                            .p(px(6.))
                            .children(rows)
                            .when(items.is_empty(), |d| d.child(div().px(px(10.)).py(px(7.)).child(crate::i18n::t("沒有符合的項目")))),
                    ),
            )
            .into_any_element()
    }

    // ----- toasts -----

    fn render_toasts(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let p = self.pal;
        let toasts: Vec<AnyElement> = self
            .toasts
            .iter()
            .map(|t| {
                let id = t.id;
                let (bg, fg) = if t.kind == ToastKind::Err { (p.danger, gpui_kit::white()) } else { (p.text, p.panel) };
                div()
                    .id(SharedString::from(format!("toast-{id}")))
                    .px(px(14.))
                    .py(px(8.))
                    .rounded(px(8.))
                    .bg(bg)
                    .text_color(fg)
                    .shadow_lg()
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.toasts.retain(|t| t.id != id);
                        cx.notify();
                    }))
                    .child(format!("{}{}", match t.kind { ToastKind::Err => "✕ ", ToastKind::Ok => "✓ ", ToastKind::Info => "" }, t.text))
                    .when_some(t.detail.clone(), |d, det| {
                        d.child(div().mt(px(6.)).max_h(px(160.)).overflow_hidden().text_size(px(12.)).font_family("Cascadia Mono").opacity(0.92).child(det))
                    })
                    .into_any_element()
            })
            .collect();
        div().absolute().right(px(16.)).bottom(px(40.)).max_w(px(520.)).flex().flex_col().gap(px(8.)).children(toasts).into_any_element()
    }

    // ----- command log -----

    pub fn render_log_bar(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let p = self.pal;
        let last = self.log.last().cloned();
        let n = self.log.len();
        let open = self.log_open;
        let list: Option<AnyElement> = open.then(|| {
            div()
                .id("log-list")
                .max_h(px(200.))
                .overflow_y_scroll()
                .track_scroll(&self.log_scroll)
                .px(px(10.))
                .pt(px(4.))
                .pb(px(8.))
                .font_family("Cascadia Mono")
                .text_size(px(12.))
                .children(self.log.iter().map(|l| {
                    let id = l.id;
                    let expanded = self.log_expand == Some(l.id) || (!l.ok && !l.output.is_empty());
                    div()
                        .id(SharedString::from(format!("log-{id}")))
                        .cursor_pointer()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.log_expand = if this.log_expand == Some(id) { None } else { Some(id) };
                            cx.notify();
                        }))
                        .child(
                            div()
                                .flex()
                                .gap(px(6.))
                                .child(sub(&p, crate::format::date_time(l.at as i64 / 1000).split(' ').nth(1).unwrap_or("").to_string()))
                                .child(div().text_color(if l.ok { p.ok } else { p.danger }).child(if l.ok { "✓" } else { "✕" }))
                                .when(!l.repo.is_empty(), |d| d.child(sub(&p, format!("[{}]", crate::format::base_name(&l.repo)))))
                                .child(l.cmd.clone())
                                .child(sub(&p, format!("({:.1}s)", l.ms as f32 / 1000.))),
                        )
                        .when(expanded, |d| {
                            d.child(div().ml(px(24.)).mt(px(2.)).mb(px(6.)).text_color(p.muted).child(if l.output.is_empty() { crate::i18n::t("（沒有輸出）").to_string() } else { l.output.clone() }))
                        })
                }))
                .when(self.log.is_empty(), |d| d.child(sub(&p, crate::i18n::t("（空）"))))
                .into_any_element()
        });
        div()
            .flex_none()
            .bg(p.panel2)
            .border_t_1()
            .border_color(p.border)
            .text_size(px(12.))
            .child(
                div()
                    .id("log-bar")
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .px(px(10.))
                    .h(px(26.))
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_color(p.muted)
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.log_open = !this.log_open;
                        cx.notify();
                    }))
                    .child(crate::i18n::tr("{} 指令紀錄", &[], &[(if open { "▾" } else { "▸" }).to_string()]))
                    .child(match last {
                        Some(l) => div()
                            .flex()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .gap(px(8.))
                            .child(div().min_w_0().overflow_hidden().text_ellipsis().text_color(p.text).font_family("Cascadia Mono").child(l.cmd.clone()))
                            .child(div().text_color(if l.ok { p.ok } else { p.danger }).child(format!("{} {:.1}s", if l.ok { "✓" } else { "✕" }, l.ms as f32 / 1000.)))
                            .into_any_element(),
                        None => div().child(crate::i18n::t("還沒有執行任何會改變東西的指令")).into_any_element(),
                    })
                    .child(div().flex_1())
                    .child(crate::i18n::tr("{n} 筆", &[("n", (n).to_string())], &[]))
                    .when(!self.git_version.is_empty(), |d| d.child(sub(&p, format!("· {}", self.git_version)))),
            )
            .when_some(list, |d, l| d.child(l))
            .into_any_element()
    }
}

#[allow(dead_code)]
fn _unused(_: gpui_kit::Point<gpui_kit::Pixels>) {
    let _ = point(px(0.), px(0.));
}
