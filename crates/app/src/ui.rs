//! Small hand-built widgets that follow the Vue build's CSS (buttons, chips, badges).

use gpui_kit::{
    AnyElement, Div, ElementId, FontWeight, Hsla, InteractiveElement as _, IntoElement, ParentElement as _, SharedString, Stateful,
    StatefulInteractiveElement as _, Styled as _, div, prelude::FluentBuilder as _, px,
};

use crate::theme::Pal;

#[derive(Clone, Copy, PartialEq)]
pub enum Bk {
    Normal,
    Primary,
    Danger,
    Ghost,
    On,
}

/// `.btn` / `.btn.sm`
pub fn btn(p: &Pal, id: impl Into<ElementId>, kind: Bk, small: bool, label: impl IntoElement) -> Stateful<Div> {
    let (bg, fg, border) = match kind {
        Bk::Normal => (p.panel, p.text, p.border_strong),
        Bk::Primary => (p.accent, gpui_kit::white(), Hsla::transparent_black()),
        Bk::Danger => (p.danger, gpui_kit::white(), Hsla::transparent_black()),
        Bk::Ghost => (Hsla::transparent_black(), p.text, Hsla::transparent_black()),
        Bk::On => (p.accent_bg, p.accent_tx, Hsla::transparent_black()),
    };
    let hover = if matches!(kind, Bk::Primary | Bk::Danger) { bg } else { p.hover };
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .gap_1()
        .h(px(if small { 24. } else { 28. }))
        .px(px(if small { 8. } else { 10. }))
        .rounded(px(7.))
        .border_1()
        .border_color(border)
        .bg(bg)
        .text_color(fg)
        .text_size(px(if small { 12. } else { 13. }))
        .whitespace_nowrap()
        .cursor_pointer()
        .hover(move |s| {
            let s = s.bg(hover);
            if matches!(kind, Bk::Primary | Bk::Danger) { s.opacity(0.92) } else { s }
        })
        .child(label)
}

/// A button that does nothing and looks disabled.
pub fn dim(b: Stateful<Div>, off: bool) -> Stateful<Div> {
    b.when(off, |b| b.opacity(0.5).cursor_default())
}

/// `.btn.icon`
pub fn icon_btn(p: &Pal, id: impl Into<ElementId>, glyph: &'static str) -> Stateful<Div> {
    let mut b = btn(p, id, Bk::Normal, false, glyph);
    b = b.w(px(30.)).px_0();
    b
}

/// `.sub`
pub fn sub(p: &Pal, s: impl Into<SharedString>) -> Div {
    div().text_size(px(11.5)).text_color(p.faint).child(s.into())
}

pub fn muted(p: &Pal, s: impl Into<SharedString>) -> Div {
    div().text_color(p.muted).child(s.into())
}

/// `.sep`
pub fn sep(p: &Pal) -> Div {
    div().w(px(1.)).h(px(20.)).mx(px(2.)).flex_none().bg(p.border)
}

/// `.st` status letter badge
pub fn st_badge(p: &Pal, letter: char) -> Div {
    let (bg, fg) = match letter {
        'M' | 'T' => (p.warn_bg, p.warn),
        'A' | 'C' | '?' => (p.ok_bg, p.ok),
        'D' => (p.danger_bg, p.danger),
        'R' => (p.accent_bg, p.accent_tx),
        'U' => (p.danger, gpui_kit::white()),
        _ => (p.warn_bg, p.warn),
    };
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(16.))
        .rounded(px(4.))
        .bg(bg)
        .text_color(fg)
        .text_size(px(10.5))
        .font_weight(FontWeight::SEMIBOLD)
        .child(if letter == '?' { 'A' } else { letter }.to_string())
}

/// `.ref` pill: branch, remote, tag, head, detached
pub fn ref_pill(p: &Pal, name: &str, kind: &str) -> Div {
    let (bg, fg) = match kind {
        "remote" => (p.accent_bg, p.accent_tx),
        "tag" => (p.warn_bg, p.warn),
        "detached" => (p.danger_bg, p.danger),
        _ => (p.ok_bg, p.ok),
    };
    div()
        .flex_none()
        .px(px(6.))
        .mr(px(4.))
        .h(px(16.))
        .flex()
        .items_center()
        .rounded(px(9.))
        .border_1()
        .border_color(if kind == "head" { p.ok } else { Hsla::transparent_black() })
        .when(kind == "head", |d| d.font_weight(FontWeight::SEMIBOLD))
        .bg(bg)
        .text_color(fg)
        .text_size(px(11.))
        .child(name.to_string())
}

/// `kbd`
pub fn kbd(p: &Pal, k: &str) -> Div {
    div()
        .px(px(5.))
        .border_1()
        .border_b_2()
        .border_color(p.border)
        .rounded(px(4.))
        .bg(p.panel2)
        .text_color(p.muted)
        .text_size(px(11.))
        .font_family("Cascadia Mono")
        .child(k.to_string())
}

/// `.empty`
pub fn empty(p: &Pal, big: &str, msg: &str, sub_msg: Option<&str>) -> AnyElement {
    div()
        .flex_1()
        .min_h_0()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap_1()
        .p(px(40.))
        .text_color(p.muted)
        .when(!big.is_empty(), |d| d.child(div().text_size(px(30.)).text_color(p.faint).child(big.to_string())))
        .child(msg.to_string())
        .when_some(sub_msg, |d, s| d.child(sub(p, s.to_string())))
        .into_any_element()
}

/// Splits a message with `**bold**` and `` `mono` `` marks into styled runs.
pub fn rich(p: &Pal, s: &str) -> AnyElement {
    use gpui_kit::{HighlightStyle, StyledText};
    let mut out = String::new();
    let mut hl: Vec<(std::ops::Range<usize>, HighlightStyle)> = Vec::new();
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '*' && chars.get(i + 1) == Some(&'*') {
            if let Some(end) = (i + 2..chars.len().saturating_sub(1)).find(|&j| chars[j] == '*' && chars[j + 1] == '*') {
                let start = out.len();
                out.extend(&chars[i + 2..end]);
                hl.push((start..out.len(), HighlightStyle { font_weight: Some(FontWeight::BOLD), ..Default::default() }));
                i = end + 2;
                continue;
            }
        }
        if chars[i] == '`'
            && let Some(end) = (i + 1..chars.len()).find(|&j| chars[j] == '`')
        {
            let start = out.len();
            out.extend(&chars[i + 1..end]);
            hl.push((start..out.len(), HighlightStyle { background_color: Some(p.panel2), color: Some(p.accent_tx), ..Default::default() }));
            i = end + 1;
            continue;
        }
        out.push(chars[i]);
        i += 1;
    }
    div().text_color(p.text).child(StyledText::new(out).with_highlights(hl)).into_any_element()
}

/// `.tooltip_text("...")` on any stateful div.
pub trait TipExt: Sized {
    fn tooltip_text(self, t: impl Into<SharedString>) -> Self;
}

impl TipExt for Stateful<Div> {
    fn tooltip_text(self, t: impl Into<SharedString>) -> Self {
        let t: SharedString = t.into();
        self.tooltip(move |window, cx| gpui_kit::component::tooltip::Tooltip::new(t.clone()).build(window, cx))
    }
}

/// How the shortcut modifier is written on this system.
pub fn mod_name() -> &'static str {
    if cfg!(target_os = "macos") { "⌘" } else { "Ctrl" }
}

/// A 12x12 icon drawn from strokes, the way the Vue build draws its SVG icons, so it sits
/// exactly in the middle of its box (font glyphs such as ＋ and ⌄ sit at different heights).
pub fn stroke_icon(color: Hsla, strokes: &'static [&'static [(f32, f32)]]) -> impl IntoElement {
    use gpui_kit::{PathBuilder, canvas, point};
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            for pts in strokes {
                let mut pb = PathBuilder::stroke(px(1.6));
                for (k, (x, y)) in pts.iter().enumerate() {
                    let p = point(bounds.origin.x + px(*x), bounds.origin.y + px(*y));
                    if k == 0 {
                        pb.move_to(p);
                    } else {
                        pb.line_to(p);
                    }
                }
                if let Ok(path) = pb.build() {
                    window.paint_path(path, color);
                }
            }
        },
    )
    .size(px(12.))
}

pub fn icon_plus(color: Hsla) -> impl IntoElement {
    stroke_icon(color, &[&[(6., 1.), (6., 11.)], &[(1., 6.), (11., 6.)]])
}

pub fn icon_chevron_down(color: Hsla) -> impl IntoElement {
    stroke_icon(color, &[&[(2., 4.5), (6., 8.5), (10., 4.5)]])
}
