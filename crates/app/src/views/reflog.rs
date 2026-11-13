//! The Reflog list (ReflogView.vue).

use gpui_kit::{
    AnyElement, Context, InteractiveElement as _, IntoElement, ParentElement as _, SharedString, StatefulInteractiveElement as _,
    Styled as _, Window, div, prelude::FluentBuilder as _, px,
};

use crate::app::*;
use crate::format::{ago, short};
use crate::ui::*;

impl GitTabs {
    pub fn render_reflog(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let p = self.pal;
        let Some(r) = self.repo() else { return div().into_any_element() };
        let rows = r.reflog.clone();
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .px(px(10.))
                    .py(px(6.))
                    .min_h(px(41.))
                    .border_b_1()
                    .border_color(p.border)
                    .child(sub(&p, crate::i18n::t("HEAD 的移動紀錄。不小心 reset、rebase 或刪掉分支時，可以從這裡找回 commit。"))),
            )
            .child(
                div()
                    .id("reflog")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .children(rows.iter().map(|e| {
                        let (s1, s2, s3, s4) = (e.sha.clone(), e.sha.clone(), e.sha.clone(), e.sha.clone());
                        div()
                            .id(SharedString::from(format!("rl-{}", e.selector)))
                            .group("rl")
                            .flex()
                            .items_center()
                            .gap(px(10.))
                            .px(px(12.))
                            .py(px(4.))
                            .border_b_1()
                            .border_color(p.border)
                            .hover(move |s| s.bg(p.hover))
                            .child(div().w(px(90.)).flex_none().font_family("Cascadia Mono").child(sub(&p, e.selector.clone())))
                            .child(
                                div()
                                    .id(SharedString::from(format!("rls-{}", e.selector)))
                                    .w(px(70.))
                                    .flex_none()
                                    .font_family("Cascadia Mono")
                                    .text_color(p.accent)
                                    .cursor_pointer()
                                    .child(short(&e.sha))
                                    .on_click(cx.listener(move |this, _, _, cx| this.copy(s1.clone(), &short(&s1), cx))),
                            )
                            .child(div().flex_1().min_w_0().overflow_hidden().text_ellipsis().whitespace_nowrap().child(e.message.clone()))
                            .child(div().w(px(100.)).flex_none().child(sub(&p, ago(e.time))))
                            .child(
                                div()
                                    .flex()
                                    .flex_none()
                                    .gap(px(4.))
                                    .opacity(0.)
                                    .group_hover("rl", |s| s.opacity(1.))
                                    .child(btn(&p, SharedString::from(format!("rlb-{}", e.selector)), Bk::Normal, true, crate::i18n::t("從這裡開分支")).on_click(cx.listener(move |this, _, w, cx| this.new_branch(Some(s2.clone()), w, cx))))
                                    .child(btn(&p, SharedString::from(format!("rlc-{}", e.selector)), Bk::Normal, true, "Checkout").on_click(cx.listener(move |this, _, _, cx| this.checkout_commit(s3.clone(), cx))))
                                    .child(btn(&p, SharedString::from(format!("rlr-{}", e.selector)), Bk::Normal, true, crate::i18n::t("Reset 到這裡")).on_click(cx.listener(move |this, _, _, cx| this.reset_to(s4.clone(), cx)))),
                            )
                    }))
                    .when(rows.is_empty(), |d| d.child(empty(&p, "", crate::i18n::t("沒有紀錄"), None))),
            )
            .into_any_element()
    }
}
