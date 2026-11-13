//! The toolbar under the tabs (Toolbar.vue).

use gpui_kit::{
    AnyElement, ClickEvent, Context, IntoElement, ParentElement as _, SharedString, StatefulInteractiveElement as _,
    Styled as _, Window, div, prelude::FluentBuilder as _, px,
};

use crate::app::GitTabs;
use crate::model::*;
use crate::ui::*;

impl GitTabs {
    fn branch_menu(&self) -> Vec<MenuItem> {
        let Some(r) = self.repo() else { return vec![] };
        let path = r.path.clone();
        let refs = r.refs.as_ref();
        let local: Vec<_> = refs.map(|x| x.refs.branches.clone()).unwrap_or_default();
        let remotes: Vec<_> = refs
            .map(|x| x.refs.remotes.iter().filter(|b| !local.iter().any(|l| Some(l.name.as_str()) == b.name.split_once('/').map(|(_, n)| n))).cloned().collect())
            .unwrap_or_default();
        let mut items = vec![MenuItem::header(crate::i18n::t("本地分支"))];
        for b in &local {
            let hint = [if b.ahead > 0 { format!("↑{}", b.ahead) } else { String::new() }, if b.behind > 0 { format!("↓{}", b.behind) } else { String::new() }].join(" ").trim().to_string();
            let (p, n, cur) = (path.clone(), b.name.clone(), b.current);
            items.push(MenuItem::new(b.name.clone(), move |t, _, cx| {
                if !cur {
                    t.switch_branch(&p, &n, cx)
                }
            }).checked(b.current).hint(hint));
        }
        if !remotes.is_empty() {
            items.push(MenuItem::header(crate::i18n::t("遠端（切換會建立本地分支）")));
            for b in remotes.iter().take(30) {
                let (p, n) = (path.clone(), b.name.clone());
                items.push(MenuItem::new(b.name.clone(), move |t, _, cx| t.switch_branch(&p, &n, cx)));
            }
        }
        items.push(MenuItem::sep());
        items.push(MenuItem::new(crate::i18n::t("新增分支…"), |t, w, cx| t.new_branch(None, w, cx)).icon(Ico::Plus));
        items
    }

    fn identity_menu(&self) -> Vec<MenuItem> {
        let Some(r) = self.repo() else { return vec![] };
        let w = self.identity_of(r);
        // Only the identities: the one in use is checked, whether it was picked here or comes from
        // a rule in the settings (then it says so). Picking one keeps it for this repo.
        let mut items = vec![MenuItem::header(crate::i18n::tr("這個 repo（{}）用哪個身分", &[], &[(r.name).to_string()]))];
        for id in &self.cfg.identities {
            let key = id.id.clone();
            let checked = w.id.as_ref().is_some_and(|x| x.id == id.id);
            items.push(
                MenuItem::new(id.name.clone(), move |t, _, cx| t.set_pin(key.clone(), cx))
                    .checked(checked)
                    .hint(if checked && !w.pinned { crate::i18n::t("符合規則") } else { "" }),
            );
        }
        items.push(MenuItem::sep());
        items.push(MenuItem::new(crate::i18n::t("管理身分…"), |t, w, cx| t.open_settings(SettingsTab::Ids, None, w, cx)).icon(Ico::Settings));
        items
    }

    pub fn render_toolbar(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let p = self.pal;
        let Some(r) = self.repo() else { return div().into_any_element() };
        let s = r.summary.clone();
        let path = r.path.clone();
        let busy = r.busy.clone();
        let dis = busy.is_some();
        let branch_label = match &s {
            Some(s) => s.branch.clone().unwrap_or_else(|| format!("(detached {})", s.head.clone().unwrap_or_default().chars().take(7).collect::<String>())),
            None => "…".into(),
        };
        let who = self.identity_of(r);
        let have_ids = !self.cfg.identities.is_empty();
        let wide = f32::from(self.window_size.width) >= 1100.;
        let (behind, ahead) = s.as_ref().map(|s| (s.behind, s.ahead)).unwrap_or((0, 0));
        let theme_ico = match self.cfg.theme.as_str() {
            "light" => Ico::Sun,
            "dark" => Ico::Moon,
            _ => Ico::SunMoon,
        };
        let down = || icon(Ico::ChevronDown, 13., p.muted);
        let rpath = path.display().to_string();
        let (p1, p2, p3, p4, p5, p6) = (path.clone(), path.clone(), path.clone(), path.clone(), path.clone(), path.clone());
        let (q1, q2) = (rpath.clone(), rpath.clone());
        let split = |id: &'static str| btn(&p, id, Bk::Normal, false, down()).px(px(6.)).ml(px(-1.)).rounded_l(px(0.));

        div()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(6.))
            .px(px(10.))
            .h(px(42.))
            .overflow_hidden()
            .bg(p.panel)
            .border_b_1()
            .border_color(p.border)
            .child(
                btn(&p, "branch", Bk::Normal, false, div().flex().items_center().gap_1().child(icon(Ico::GitBranch, 14., p.muted)).child(div().font_weight(gpui_kit::FontWeight::SEMIBOLD).child(branch_label)).child(down()))
                    .tooltip_text(crate::i18n::t("切換分支"))
                    .on_click(cx.listener(|this, ev: &ClickEvent, _, cx| {
                        let items = this.branch_menu();
                        this.open_menu(ev.position(), items, false, cx);
                    })),
            )
            .child(sep(&p))
            .child(dim(
                btn(&p, "sync", Bk::Primary, false, div().flex().items_center().gap(px(5.)).child(icon(Ico::RefreshCw, 14., gpui_kit::white())).child(format!("Sync{}{}", if behind > 0 { format!(" ↓{behind}") } else { String::new() }, if ahead > 0 { format!(" ↑{ahead}") } else { String::new() })))
                    .tooltip_text("fetch → pull → push")
                    .when(!dis, |b| b.on_click(cx.listener(move |this, _, _, cx| this.sync_repo(&p1, cx)))),
                dis,
            ))
            .child(dim(btn(&p, "fetch", Bk::Normal, false, "Fetch").when(!dis, |b| b.on_click(cx.listener(move |this, _, _, cx| this.fetch_repo(&p2, cx)))), dis))
            .child(
                div()
                    .flex()
                    .flex_none()
                    .child(dim(
                        btn(&p, "pull", Bk::Normal, false, div().flex().child("Pull").when(behind > 0, |d| d.child(div().ml_1().text_color(p.warn).child(format!("↓{behind}")))))
                            .rounded_r(px(0.))
                            .when(!dis, |b| b.on_click(cx.listener(move |this, _, _, cx| this.pull_repo(&p3, "ff", cx)))),
                        dis,
                    ))
                    .child(split("pull-more").on_click(cx.listener(move |this, ev: &ClickEvent, _, cx| {
                        let (a, b, c) = (p4.clone(), p4.clone(), p4.clone());
                        let items = vec![
                            MenuItem::new(crate::i18n::t("Pull（只快轉）"), move |t, _, cx| t.pull_repo(&a, "ff", cx)),
                            MenuItem::new("Pull --rebase", move |t, _, cx| t.pull_repo(&b, "rebase", cx)),
                            MenuItem::new(crate::i18n::t("Pull（merge）"), move |t, _, cx| t.pull_repo(&c, "merge", cx)),
                        ];
                        this.open_menu(ev.position(), items, false, cx);
                    }))),
            )
            .child(
                div()
                    .flex()
                    .flex_none()
                    .child(dim(
                        btn(&p, "push", Bk::Normal, false, div().flex().child("Push").when(ahead > 0, |d| d.child(div().ml_1().text_color(p.accent).child(format!("↑{ahead}")))))
                            .rounded_r(px(0.))
                            .when(!dis, |b| b.on_click(cx.listener(move |this, _, _, cx| this.push_repo(&p5, false, cx)))),
                        dis,
                    ))
                    .child(split("push-more").on_click(cx.listener(move |this, ev: &ClickEvent, _, cx| {
                        let (a, b) = (p6.clone(), p6.clone());
                        let items = vec![
                            MenuItem::new("Push", move |t, _, cx| t.push_repo(&a, false, cx)),
                            MenuItem::new(crate::i18n::t("強制 Push（--force-with-lease）"), move |t, _, cx| t.push_repo(&b, true, cx)).danger(),
                        ];
                        this.open_menu(ev.position(), items, false, cx);
                    }))),
            )
            .child(sep(&p))
            .child(btn(&p, "stash", Bk::Normal, false, "Stash").on_click(cx.listener(|this, _, w, cx| this.stash_push(w, cx))))
            .child(btn(&p, "newbranch", Bk::Normal, false, crate::i18n::t("＋ 分支")).on_click(cx.listener(|this, _, w, cx| this.new_branch(None, w, cx))))
            .when_some(busy, |d, b| d.child(div().ml(px(6.)).child(sub(&p, crate::i18n::tr("{b} 執行中…", &[("b", (b).to_string())], &[])))))
            .child(div().flex_1())
            .child(if have_ids {
                // The identity's name; icon only on a narrow window.
                let label: String = match &who.id {
                    Some(id) if wide => id.name.clone(),
                    None if wide => crate::i18n::t("沒有對應的身分").into(),
                    None => "?".into(),
                    _ => String::new(),
                };
                btn(&p, "identity", Bk::Normal, false, div().flex().items_center().gap_1().child(icon(Ico::User, 15., if who.id.is_some() { p.muted } else { p.warn })).child(div().text_color(if who.id.is_some() { p.text } else { p.warn }).child(label)).child(down()))
                    .tooltip_text(crate::i18n::t("切換這個 repo 用的身分"))
                    .on_click(cx.listener(|this, ev: &ClickEvent, _, cx| {
                        let items = this.identity_menu();
                        this.open_menu(ev.position(), items, true, cx);
                    }))
            } else {
                btn(&p, "identity", Bk::Normal, false, div().flex().items_center().gap_1().child(icon(Ico::User, 15., p.muted)).when(wide, |d| d.child(crate::i18n::t(" 設定身分").trim_start())))
                    .tooltip_text(crate::i18n::t("設定 SSH key 和身分"))
                    .on_click(cx.listener(|this, _, w, cx| this.open_settings(SettingsTab::Ids, None, w, cx)))
            })
            .child(
                btn(&p, "palette", Bk::Normal, false, div().flex().items_center().gap_1().child(icon(Ico::Search, 15., p.muted)).when(wide, |d| d.child(kbd(&p, &format!("{} K", crate::ui::mod_name())))))
                    .tooltip_text(crate::i18n::t("搜尋指令、repo、分支、commit"))
                    .on_click(cx.listener(|this, _, w, cx| this.open_palette(w, cx))),
            )
            .child(icon_btn(&p, "open-folder", Ico::FolderOpen).tooltip_text(crate::i18n::t("在檔案總管開啟")).on_click(cx.listener(move |this, _, _, cx| this.open_folder(q1.clone(), cx))))
            .child(icon_btn(&p, "open-term", Ico::SquareTerminal).tooltip_text(crate::i18n::t("開終端機")).on_click(cx.listener(move |this, _, _, cx| this.open_terminal(q2.clone(), cx))))
            .child(
                icon_btn(&p, "theme", theme_ico)
                    .tooltip_text(match self.cfg.theme.as_str() {
                        "light" => crate::i18n::t("外觀：淺色"),
                        "dark" => crate::i18n::t("外觀：深色"),
                        _ => crate::i18n::t("外觀：跟系統"),
                    })
                    .on_click(cx.listener(|this, _, w, cx| this.cycle_theme(w, cx))),
            )
            .child(icon_btn(&p, "settings", Ico::Settings).tooltip_text(crate::i18n::t("設定")).on_click(cx.listener(|this, _, w, cx| this.open_settings(SettingsTab::Ids, None, w, cx))))
            .into_any_element()
    }
}

#[allow(dead_code)]
fn _id(s: &str) -> SharedString {
    SharedString::from(s.to_string())
}
