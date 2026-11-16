//! The "a newer version is out" bar and what its button does. The work itself is in
//! `gitcore::update`.

use gpui_kit::{AnyElement, Context, IntoElement, ParentElement as _, StatefulInteractiveElement as _, Styled as _, div, px};

use crate::app::GitTabs;
use crate::model::ToastKind;
use crate::ui::*;

const VERSION: &str = env!("CARGO_PKG_VERSION");

impl GitTabs {
    /// Looks for a newer release in the background; says nothing when there is none or the
    /// network is down.
    pub fn check_update(&mut self, cx: &mut Context<Self>) {
        if self.updating || self.update.is_some() {
            return;
        }
        self.work(
            cx,
            || gitcore::update::check(VERSION),
            |this, found, cx| {
                if let Ok(Some(rel)) = found {
                    this.update = Some(rel);
                    cx.notify();
                }
            },
        );
    }

    /// Downloads the new version, swaps it in and starts it (or opens the release page, for
    /// installs the app cannot replace by itself).
    pub fn install_update(&mut self, cx: &mut Context<Self>) {
        let Some(rel) = self.update.clone() else { return };
        if self.updating {
            return;
        }
        self.updating = true;
        cx.notify();
        self.work(
            cx,
            move || gitcore::update::apply(&rel),
            |this, r, cx| {
                this.updating = false;
                match r {
                    Ok(gitcore::update::Outcome::Restart(p)) => match gitcore::update::restart(&p) {
                        Ok(()) => cx.quit(),
                        Err(e) => this.toast(crate::i18n::t("已更新，但無法重新啟動，請手動開啟"), ToastKind::Err, Some(e), cx),
                    },
                    Ok(gitcore::update::Outcome::Quit) => cx.quit(),
                    Ok(gitcore::update::Outcome::OpenPage(url)) => {
                        let _ = gitcore::open_url(&url);
                        this.toast(crate::i18n::t("已開啟下載頁面"), ToastKind::Info, None, cx);
                    }
                    Err(e) => this.toast(crate::i18n::t("更新失敗"), ToastKind::Err, Some(e), cx),
                }
                cx.notify();
            },
        );
    }

    pub fn render_update_bar(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let rel = self.update.as_ref()?;
        let p = self.pal;
        let text = crate::i18n::tr("有新版本 {v}（目前 {cur}）", &[("v", rel.version().to_string()), ("cur", VERSION.to_string())], &[]);
        let updating = self.updating;
        Some(
            div()
                .flex()
                .items_center()
                .gap(px(10.))
                .px(px(12.))
                .py(px(5.))
                .bg(p.accent.opacity(0.14))
                .border_b_1()
                .border_color(p.border)
                .child(div().child(text))
                .child(div().flex_1())
                .child(
                    btn(&p, "update-now", Bk::Primary, true, if updating { crate::i18n::t("更新中…") } else { crate::i18n::t("立即更新") })
                        .on_click(cx.listener(|this, _, _, cx| this.install_update(cx))),
                )
                .child(btn(&p, "update-later", Bk::Ghost, true, crate::i18n::t("稍後")).on_click(cx.listener(|this, _, _, cx| {
                    this.update = None;
                    cx.notify();
                })))
                .into_any_element(),
        )
    }
}
