//! The colour palette of the Vue build (styles.css), applied to GPUI Kit's theme so its
//! components (diff, inputs, scrollbars) match the hand-built parts of the window.

use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::{App, Hsla, Rgba, SharedString, Window, px, rgb};

fn c(hex: u32) -> Hsla {
    Rgba::from(rgb(hex)).into()
}

/// One theme's colours, named after the CSS variables they come from.
#[derive(Clone, Copy)]
pub struct Pal {
    pub bg: Hsla,
    pub panel: Hsla,
    pub panel2: Hsla,
    pub hover: Hsla,
    pub text: Hsla,
    pub muted: Hsla,
    pub faint: Hsla,
    pub border: Hsla,
    pub border_strong: Hsla,
    pub accent: Hsla,
    pub accent_bg: Hsla,
    pub accent_tx: Hsla,
    pub warn: Hsla,
    pub warn_bg: Hsla,
    pub ok: Hsla,
    pub ok_bg: Hsla,
    pub danger: Hsla,
    pub danger_bg: Hsla,
}

impl Pal {
    pub fn new(dark: bool) -> Self {
        if dark {
            Self {
                bg: c(0x16181c),
                panel: c(0x1e2126),
                panel2: c(0x24282e),
                hover: c(0x2b3037),
                text: c(0xe4e7eb),
                muted: c(0xa3aab5),
                faint: c(0x6c7480),
                border: c(0x3b4048),
                border_strong: c(0x4e555f),
                accent: c(0x5b8ff0),
                accent_bg: c(0x22324f),
                accent_tx: c(0xa9c4fb),
                warn: c(0xe5b45a),
                warn_bg: c(0x3b3020),
                ok: c(0x8fd19e),
                ok_bg: c(0x1d3324),
                danger: c(0xff8a80),
                danger_bg: c(0x3d2224),
            }
        } else {
            Self {
                bg: c(0xeef0f3),
                panel: c(0xffffff),
                panel2: c(0xf6f7f9),
                hover: c(0xeef1f5),
                text: c(0x1d2129),
                muted: c(0x5f6672),
                faint: c(0x9aa1ad),
                border: c(0xd6dbe2),
                border_strong: c(0xc2c8d1),
                accent: c(0x2f6fdf),
                accent_bg: c(0xe5eeff),
                accent_tx: c(0x1f4fae),
                warn: c(0xa86a10),
                warn_bg: c(0xfdf1d6),
                ok: c(0x2b7a3b),
                ok_bg: c(0xe2f3e5),
                danger: c(0xc0362c),
                danger_bg: c(0xfde4e1),
            }
        }
    }
}

/// The graph's lane colours.
pub const LANE_COLORS: [u32; 8] = [0x2f6fdf, 0x1d9e75, 0xd85a30, 0x9b59d0, 0xc2185b, 0x0f8fa8, 0xb7791f, 0x5c6bc0];

pub fn lane_color(i: usize) -> Hsla {
    c(LANE_COLORS[i % LANE_COLORS.len()])
}

/// Puts the palette into GPUI Kit's theme.
pub fn apply(dark: bool, window: Option<&mut Window>, cx: &mut App) {
    Theme::change(if dark { ThemeMode::Dark } else { ThemeMode::Light }, window, cx);
    let p = Pal::new(dark);
    let t = Theme::global_mut(cx);
    t.font_family = SharedString::from("Segoe UI");
    t.font_size = px(13.);
    t.mono_font_family = SharedString::from("Cascadia Mono");
    t.mono_font_size = px(12.5);
    let k = &mut t.colors;
    k.background = p.panel;
    k.foreground = p.text;
    k.border = p.border;
    k.input = p.border_strong;
    k.muted = p.panel2;
    k.muted_foreground = p.muted;
    k.accent = p.hover;
    k.accent_foreground = p.text;
    k.primary = p.accent;
    k.primary_hover = p.accent;
    k.primary_active = p.accent;
    k.primary_foreground = c(0xffffff);
    k.ring = p.accent;
    k.caret = p.text;
    k.selection = p.accent.opacity(0.28);
    k.success = p.ok;
    k.danger = p.danger;
    k.warning = p.warn;
    k.info = p.accent;
    k.list = p.panel;
    k.list_hover = p.hover;
    k.list_active = p.accent_bg;
    k.list_active_border = p.accent;
    k.popover = p.panel;
    k.popover_foreground = p.text;
    k.sidebar = p.panel2;
    k.scrollbar = Hsla::transparent_black();
    k.scrollbar_thumb = p.border_strong;
    k.scrollbar_thumb_hover = p.faint;
    k.title_bar = p.bg;
    k.status_bar = p.panel2;
    k.status_bar_border = p.border;
}
