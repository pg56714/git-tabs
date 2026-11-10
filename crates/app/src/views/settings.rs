//! The big dialogs: settings (SettingsModal.vue), add repo (AddRepoModal.vue) and blame
//! (BlameModal.vue).

use gitcore::config::Identity;
use gpui_kit::component::input::Input;
use gpui_kit::{
    AnyElement, ClickEvent, Context, Hsla, InteractiveElement as _, IntoElement, ParentElement as _, SharedString, StatefulInteractiveElement as _,
    Styled as _, Window, div, prelude::FluentBuilder as _, px, uniform_list,
};

use crate::app::GitTabs;
use crate::format::{ago, base_name, date_time, short};
use crate::identity::{COLORS, auto_short, match_identity, norm_url, tag_of};
use crate::model::*;
use crate::theme::chip_color;
use crate::ui::*;

fn seg<'a>(p: &crate::theme::Pal, opts: &[(&'a str, &'a str)], current: &str, mut mk: impl FnMut(&'a str) -> Box<dyn Fn(&mut Window, &mut gpui_kit::App)>) -> AnyElement {
    let _ = &mut mk;
    let items: Vec<AnyElement> = opts
        .iter()
        .enumerate()
        .map(|(i, (v, label))| {
            let on = *v == current;
            let act = mk(v);
            div()
                .id(SharedString::from(format!("seg-{label}-{i}")))
                .px(px(10.))
                .py(px(3.))
                .cursor_pointer()
                .when(i > 0, |d| d.border_l_1().border_color(p.border_strong))
                .bg(if on { p.accent_bg } else { Hsla::transparent_black() })
                .text_color(if on { p.accent_tx } else { p.muted })
                .child(label.to_string())
                .on_click(move |_, w, cx| act(w, cx))
                .into_any_element()
        })
        .collect();
    div().flex().flex_none().border_1().border_color(p.border_strong).rounded(px(7.)).overflow_hidden().children(items).into_any_element()
}

impl GitTabs {
    fn labeled(&self, label: &str, content: AnyElement) -> AnyElement {
        div().flex().items_center().gap(px(10.)).child(div().w(px(110.)).flex_none().child(label.to_string())).child(div().flex_1().min_w_0().child(content)).into_any_element()
    }

    fn info_line(&self, kind: u8, text: String) -> AnyElement {
        let p = self.pal;
        let (bg, fg) = match kind {
            0 => (p.ok_bg, p.ok),
            1 => (p.warn_bg, p.warn),
            _ => (p.panel2, p.muted),
        };
        div().px(px(8.)).py(px(6.)).rounded(px(6.)).bg(bg).text_color(fg).text_size(px(12.)).child(text).into_any_element()
    }

    fn drafts_snapshot(&self, cx: &gpui_kit::App) -> Vec<Identity> {
        let Some(Modal::Settings(m)) = &self.modal else { return vec![] };
        m.ids
            .iter()
            .map(|d| {
                let mut id = d.id.clone();
                id.name = d.name.read(cx).value().to_string();
                id.short = d.short.read(cx).value().to_string();
                id.rules = d.rules.iter().map(|r| gitcore::config::Rule { t: r.t.clone(), v: r.v.read(cx).value().to_string() }).collect();
                id
            })
            .collect()
    }

    pub fn render_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        self.settings_take_pending(window, cx);
        let p = self.pal;
        let snapshot = self.drafts_snapshot(cx);
        let Some(Modal::Settings(m)) = &self.modal else { return div().into_any_element() };
        let tab = m.tab;
        let tabs = seg(&p, &[("ids", crate::i18n::t("身分與 SSH key")), ("general", crate::i18n::t("一般"))], if tab == SettingsTab::Ids { "ids" } else { "general" }, |v| {
            let _ = v;
            Box::new(|_, _| {})
        });
        // The segment needs `cx.listener`, which the generic helper cannot take: build it here.
        let tab_btn = |id: &'static str, label: &str, t: SettingsTab| {
            div()
                .id(id)
                .px(px(10.))
                .py(px(3.))
                .cursor_pointer()
                .bg(if tab == t { p.accent_bg } else { Hsla::transparent_black() })
                .text_color(if tab == t { p.accent_tx } else { p.muted })
                .child(label.to_string())
                .on_click(cx.listener(move |this, _, _, cx| {
                    if let Some(Modal::Settings(m)) = &mut this.modal {
                        m.tab = t;
                    }
                    cx.notify();
                }))
        };
        let _ = tabs;
        let tabs = div()
            .flex()
            .flex_none()
            .w_auto()
            .border_1()
            .border_color(p.border_strong)
            .rounded(px(7.))
            .overflow_hidden()
            .child(tab_btn("st-ids", crate::i18n::t("身分與 SSH key"), SettingsTab::Ids))
            .child(div().w(px(1.)).bg(p.border_strong))
            .child(tab_btn("st-kg", crate::i18n::t("一般"), SettingsTab::General));

        let mut body: Vec<AnyElement> = vec![div().pb(px(10.)).border_b_1().border_color(p.border).flex().child(tabs).into_any_element()];
        let Some(Modal::Settings(m)) = &self.modal else { return div().into_any_element() };
        if tab == SettingsTab::Ids {
            if let Some(n) = &m.notice {
                body.push(self.info_line(0, n.clone()));
            }
            body.push(sub(&p, crate::i18n::t("依照 remote 網址或資料夾自動選身分。儲存時會寫進 git 全域設定的 includeIf，所以在終端機執行的 git 也會用同一個身分。")).into_any_element());
            if m.ids.is_empty() {
                body.push(div().p(px(16.)).border_1().border_color(p.border).rounded(px(10.)).text_color(p.muted).text_center().child(crate::i18n::t("還沒有身分。按「＋ 新增身分」，填好 SSH key 和 email。")).into_any_element());
            }
            for (i, d) in m.ids.iter().enumerate() {
                body.push(self.render_id_card(i, d, &snapshot, cx));
            }
            // ---- add / scan / default
            let def_name = snapshot.iter().find(|x| x.id == m.default).map(|x| x.name.clone()).unwrap_or_default();
            let scanning = m.scanning;
            let ids_for_menu: Vec<(String, String)> = snapshot.iter().map(|x| (x.id.clone(), x.name.clone())).collect();
            body.push(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(px(8.))
                    .child(btn(&p, "id-add", Bk::Normal, false, crate::i18n::t("＋ 新增身分")).on_click(cx.listener(|this, _, w, cx| this.settings_add(w, cx))))
                    .child(btn(&p, "id-scan", Bk::Normal, false, if scanning { crate::i18n::t("尋找中…") } else { crate::i18n::t("從這台電腦找") }).tooltip_text(crate::i18n::t("找 ~/.ssh 的 key、~/.ssh/config 和 git 的 includeIf")).when(!scanning, |b| b.on_click(cx.listener(|this, _, _, cx| this.settings_scan(cx)))))
                    .child(div().flex_1())
                    .when(!snapshot.is_empty(), |d| {
                        d.child(sub(&p, crate::i18n::t("都沒對應時用"))).child(btn(&p, "id-default", Bk::Normal, false, format!("{} ▾", if def_name.is_empty() { crate::i18n::t("git 原本的全域設定").to_string() } else { def_name.clone() })).on_click(cx.listener(move |this, ev: &ClickEvent, _, cx| {
                            let mut items = vec![MenuItem::new(crate::i18n::t("git 原本的全域設定"), |t, _, cx| {
                                if let Some(Modal::Settings(m)) = &mut t.modal {
                                    m.default.clear();
                                }
                                cx.notify();
                            })];
                            for (id, name) in ids_for_menu.clone() {
                                items.push(MenuItem::new(name, move |t, _, cx| {
                                    if let Some(Modal::Settings(m)) = &mut t.modal {
                                        m.default = id.clone();
                                    }
                                    cx.notify();
                                }));
                            }
                            this.open_menu(ev.position(), items, true, cx);
                        })))
                    })
                    .into_any_element(),
            );
            // ---- repos without a matching rule
            if !snapshot.is_empty() {
                let unmatched: Vec<usize> = self
                    .repos
                    .iter()
                    .enumerate()
                    .filter(|(_, r)| self.pin_of(r).is_none() && match_identity(&snapshot, &r.path.display().to_string(), r.summary.as_ref().and_then(|s| s.remote_url.as_deref())).is_none())
                    .map(|(i, _)| i)
                    .collect();
                if !unmatched.is_empty() && !def_name.is_empty() {
                    body.push(sub(&p, crate::i18n::tr("這些 repo 沒有符合的規則，會用預設的「{def_name}」；要換成別的身分可以在這裡指定：", &[("def_name", (def_name).to_string())], &[])).into_any_element());
                }
                for ri in &unmatched {
                    let r = &self.repos[*ri];
                    let url = norm_url(r.summary.as_ref().and_then(|s| s.remote_url.as_deref()).unwrap_or(""));
                    let ri = *ri;
                    let names: Vec<String> = snapshot.iter().map(|x| x.name.clone()).collect();
                    body.push(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .px(px(8.))
                            .py(px(4.))
                            .rounded(px(6.))
                            .text_size(px(12.))
                            .bg(if def_name.is_empty() { p.warn_bg } else { p.panel2 })
                            .text_color(if def_name.is_empty() { p.warn } else { p.muted })
                            .child(div().child(crate::i18n::tr("{}（{}）{}", &[], &[(r.name).to_string(), (if url.is_empty() { crate::i18n::t("沒有 remote").into() } else { url }).to_string(), (if def_name.is_empty() { crate::i18n::t("沒有對應的身分") } else { crate::i18n::t("用預設") }).to_string()])))
                            .child(div().flex_1())
                            .child(btn(&p, SharedString::from(format!("assign-{ri}")), Bk::Normal, true, crate::i18n::t("指定給… ▾")).on_click(cx.listener(move |this, ev: &ClickEvent, _, cx| {
                                let items = names.iter().enumerate().map(|(k, n)| MenuItem::new(n.clone(), move |t, w, cx| t.settings_assign(ri, k, w, cx))).collect();
                                this.open_menu(ev.position(), items, true, cx);
                            })))
                            .into_any_element(),
                    );
                }
                if unmatched.is_empty() && !self.repos.is_empty() {
                    body.push(self.info_line(0, crate::i18n::t("✓ 開著的 repo 都有對應的身分").into()));
                }
            }
        } else {
            let theme = m.theme.clone();
            let fetch = m.fetch.clone();
            let tbtn = |id: &'static str, label: &'static str, v: &'static str, cur: &str| {
                let on = cur == v;
                div()
                    .id(id)
                    .px(px(10.))
                    .py(px(3.))
                    .cursor_pointer()
                    .bg(if on { p.accent_bg } else { Hsla::transparent_black() })
                    .text_color(if on { p.accent_tx } else { p.muted })
                    .child(label)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(Modal::Settings(m)) = &mut this.modal {
                            m.theme = v.to_string();
                        }
                        cx.notify();
                    }))
            };
            let fbtn = |id: String, label: &'static str, v: &'static str, cur: &str| {
                let on = cur == v;
                div()
                    .id(SharedString::from(id))
                    .px(px(10.))
                    .py(px(3.))
                    .cursor_pointer()
                    .bg(if on { p.accent_bg } else { Hsla::transparent_black() })
                    .text_color(if on { p.accent_tx } else { p.muted })
                    .child(label)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(Modal::Settings(m)) = &mut this.modal {
                            m.fetch = v.to_string();
                        }
                        cx.notify();
                    }))
            };
            let box_ = |c: Vec<AnyElement>| div().flex().child(div().flex().flex_none().border_1().border_color(p.border_strong).rounded(px(7.)).overflow_hidden().children(c)).into_any_element();
            let lang = self.lang.clone();
            let lbtn = |id: &'static str, label: &'static str, v: &'static str, cur: &str| {
                let on = cur == v;
                div()
                    .id(id)
                    .px(px(10.))
                    .py(px(3.))
                    .cursor_pointer()
                    .bg(if on { p.accent_bg } else { Hsla::transparent_black() })
                    .text_color(if on { p.accent_tx } else { p.muted })
                    .child(label)
                    .on_click(cx.listener(move |this, _, w, cx| this.set_lang(v, w, cx)))
            };
            body.push(self.labeled(
                &(if crate::i18n::is_en() { "Language".to_string() } else { "語言 / Language".to_string() }),
                box_(vec![lbtn("lg-auto", crate::i18n::t("自動"), "auto", &lang).into_any_element(), lbtn("lg-zh", "繁體中文", "zh-TW", &lang).into_any_element(), lbtn("lg-en", "English", "en", &lang).into_any_element()]),
            ));
            body.push(self.labeled(crate::i18n::t("外觀"), box_(vec![tbtn("th-sys", crate::i18n::t("跟系統"), "system", &theme).into_any_element(), tbtn("th-light", crate::i18n::t("淺色"), "light", &theme).into_any_element(), tbtn("th-dark", crate::i18n::t("深色"), "dark", &theme).into_any_element()])));
            body.push(self.labeled(crate::i18n::t("git 路徑"), Input::new(&m.git).into_any_element()));
            body.push(div().ml(px(120.)).child(sub(&p, crate::i18n::tr("目前：{}", &[], &[(if self.git_version.is_empty() { crate::i18n::t("找不到 git") } else { &self.git_version }).to_string()]))).into_any_element());
            body.push(self.labeled(
                crate::i18n::t("背景 fetch"),
                box_(vec![
                    fbtn("f0".into(), crate::i18n::t("不要"), "0", &fetch).into_any_element(),
                    fbtn("f5".into(), crate::i18n::t("5 分"), "5", &fetch).into_any_element(),
                    fbtn("f10".into(), crate::i18n::t("10 分"), "10", &fetch).into_any_element(),
                    fbtn("f30".into(), crate::i18n::t("30 分"), "30", &fetch).into_any_element(),
                    fbtn("f60".into(), crate::i18n::t("1 小時"), "60", &fetch).into_any_element(),
                ]),
            ));
            body.push(self.labeled(crate::i18n::t("Repo 資料夾"), Input::new(&m.clone).into_any_element()));
            body.push(div().ml(px(120.)).child(sub(&p, crate::i18n::t("「加入 repo」會先掃描這裡，clone 也預設放這裡。"))).into_any_element());
            body.push(sub(&p, crate::i18n::tr("HTTPS 的 repo 用 Git Credential Manager 存帳號 token；網址帶帳號名稱（https://帳號@github.com/…）就能分開存。設定檔位置：app 的設定資料夾 config.json。目前開了 {} 個 repo：{}", &[], &[(self.repos.len()).to_string(), (self.repos.iter().map(|r| base_name(&r.path.display().to_string())).collect::<Vec<_>>().join(crate::i18n::t("、"))).to_string()])).into_any_element());
        }
        if let Some(e) = &m.err {
            body.push(div().text_size(px(12.)).text_color(p.danger).child(e.clone()).into_any_element());
        }
        let saving = m.saving;
        let cancel = btn(&p, "s-cancel", Bk::Normal, false, crate::i18n::t("取消")).on_click(cx.listener(|this, _, w, cx| this.close_modal(w, cx)));
        let save = dim(btn(&p, "s-save", Bk::Primary, false, if saving { crate::i18n::t("儲存中…") } else { crate::i18n::t("儲存") }).when(!saving, |b| b.on_click(cx.listener(|this, _, w, cx| this.settings_save(w, cx)))), saving);
        self.dialog(div().child(crate::i18n::t("設定")).into_any_element(), div().flex().flex_col().gap(px(10.)).children(body).into_any_element(), vec![cancel.into_any_element(), save.into_any_element()], 880., cx)
    }

    fn render_id_card(&self, i: usize, d: &IdDraft, snapshot: &[Identity], cx: &mut Context<Self>) -> AnyElement {
        let p = self.pal;
        let snap = snapshot.get(i).cloned().unwrap_or_else(|| d.id.clone());
        let tag = tag_of(&snap);
        let (cbg, cfg) = chip_color(&d.id.color, p.dark);
        let matched: Vec<String> = self
            .repos
            .iter()
            .filter(|r| {
                let pin = self.pin_of(r);
                if let Some(pin) = pin {
                    return pin == d.id.id;
                }
                let hit = match_identity(snapshot, &r.path.display().to_string(), r.summary.as_ref().and_then(|s| s.remote_url.as_deref())).map(|x| x.id.clone());
                hit.or_else(|| Some(String::new()).filter(|_| false)).is_some_and(|x| x == d.id.id)
            })
            .map(|r| r.name.clone())
            .collect();
        let row = |label: &str, content: AnyElement| self.labeled(label, content);
        let sw: Vec<AnyElement> = COLORS
            .iter()
            .map(|c| {
                let (bg, fg) = chip_color(c, p.dark);
                let _ = bg;
                let name = c.to_string();
                div()
                    .id(SharedString::from(format!("sw-{i}-{c}")))
                    .size(px(18.))
                    .rounded_full()
                    .cursor_pointer()
                    .bg(fg)
                    .when(d.id.color == *c, |d| d.border_2().border_color(p.text))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(Modal::Settings(m)) = &mut this.modal
                            && let Some(d) = m.ids.get_mut(i)
                        {
                            d.id.color = name.clone();
                        }
                        cx.notify();
                    }))
                    .into_any_element()
            })
            .collect();
        let mut rows: Vec<AnyElement> = Vec::new();
        // ---- title row
        rows.push(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(div().w(px(200.)).child(Input::new(&d.name)))
                .child(div().px(px(6.)).h(px(16.)).flex().items_center().rounded(px(9.)).bg(cbg).text_color(cfg).text_size(px(10.5)).child(tag))
                .child(if d.id.verified {
                    div().flex().gap_1().text_size(px(11.5)).text_color(p.faint).child(div().text_color(p.ok).child(crate::i18n::t("✓ 已驗證"))).child(crate::i18n::tr("帳號 {}", &[], &[(d.id.account).to_string()]))
                } else {
                    div().text_size(px(11.5)).text_color(p.faint).child(crate::i18n::t("還沒驗證這把 key 對應哪個帳號"))
                })
                .child(div().flex_1())
                .child(btn(&p, SharedString::from(format!("rm-{i}")), Bk::Ghost, true, "🗑").tooltip_text(crate::i18n::t("刪除這個身分")).on_click(cx.listener(move |this, _, _, cx| this.settings_remove(i, cx))))
                .into_any_element(),
        );
        rows.push(row(
            crate::i18n::t("標籤"),
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(div().w(px(110.)).child(Input::new(&d.short)))
                .child(sub(&p, crate::i18n::tr("留空會自動用「{}」", &[], &[(auto_short(&snap.name)).to_string()])))
                .child(div().flex_1())
                .child(div().flex().gap(px(6.)).children(sw))
                .into_any_element(),
        ));
        rows.push(row(
            "SSH key",
            div()
                .flex()
                .items_center()
                .gap(px(6.))
                .child(div().flex_1().child(Input::new(&d.key)))
                .child(btn(&p, SharedString::from(format!("kb-{i}")), Bk::Normal, true, crate::i18n::t("選檔案…")).on_click(cx.listener(move |this, _, w, cx| this.settings_browse_key(i, w, cx))))
                .child(dim(btn(&p, SharedString::from(format!("kt-{i}")), Bk::Normal, true, if d.testing { crate::i18n::t("連線中…") } else { crate::i18n::t("測試連線") }).when(!d.testing, |b| b.on_click(cx.listener(move |this, _, _, cx| this.settings_test(i, cx)))), d.testing))
                .child(btn(&p, SharedString::from(format!("kg-{i}")), if d.kg.open { Bk::On } else { Bk::Normal }, true, crate::i18n::t("產生新 key")).on_click(cx.listener(move |this, _, _, cx| {
                    if let Some(Modal::Settings(m)) = &mut this.modal
                        && let Some(d) = m.ids.get_mut(i)
                    {
                        d.kg.open = !d.kg.open;
                    }
                    cx.notify();
                })))
                .into_any_element(),
        ));
        if let Some(Ok(t)) = &d.test {
            rows.push(div().font_family("Cascadia Mono").child(self.info_line(if t.ok { 0 } else { 1 }, if t.output.is_empty() { crate::i18n::t("（沒有輸出）").into() } else { t.output.clone() })).into_any_element());
        }
        if d.kg.open {
            let mut g: Vec<AnyElement> = Vec::new();
            match &d.kg.public {
                None => {
                    g.push(row(crate::i18n::t("存到"), Input::new(&d.gen_path).into_any_element()));
                    g.push(row(crate::i18n::t("密碼（可留空）"), Input::new(&d.gen_pass).into_any_element()));
                    g.push(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(dim(btn(&p, SharedString::from(format!("kgo-{i}")), Bk::Primary, true, crate::i18n::t("產生 ed25519 key")).when(!d.kg.busy, |b| b.on_click(cx.listener(move |this, _, _, cx| this.settings_gen(i, cx)))), d.kg.busy))
                            .child(sub(&p, crate::i18n::t("私鑰只會留在這台電腦。已經存在的檔案不會被覆蓋。")))
                            .into_any_element(),
                    );
                    if let Some(e) = &d.kg.err {
                        g.push(div().text_size(px(12.)).text_color(p.danger).child(e.clone()).into_any_element());
                    }
                }
                Some(publ) => {
                    let (pub1, host) = (publ.clone(), d.host.read(cx).value().to_string());
                    let host = if host.trim().is_empty() { "github.com".to_string() } else { host };
                    g.push(sub(&p, crate::i18n::tr("已產生 {}。把下面的公鑰加到這個帳號的 GitHub SSH keys：", &[], &[(d.kg.path).to_string()])).into_any_element());
                    g.push(div().px(px(8.)).py(px(6.)).rounded(px(6.)).border_1().border_color(p.border).bg(p.panel2).font_family("Cascadia Mono").text_size(px(11.5)).child(publ.clone()).into_any_element());
                    g.push(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(btn(&p, SharedString::from(format!("kcp-{i}")), Bk::Normal, true, crate::i18n::t("複製公鑰")).on_click(cx.listener(move |this, _, _, cx| this.copy(pub1.clone(), crate::i18n::t("公鑰"), cx))))
                            .child(btn(&p, SharedString::from(format!("kgh-{i}")), Bk::Normal, true, crate::i18n::t("打開 GitHub 新增 SSH key 頁面")).on_click(cx.listener(move |this, _, _, cx| this.open_url(&format!("https://{host}/settings/ssh/new"), cx))))
                            .child(sub(&p, crate::i18n::t("貼好之後按「測試連線」")))
                            .into_any_element(),
                    );
                }
            }
            rows.push(div().flex().flex_col().gap(px(6.)).p(px(8.)).border_1().border_color(p.border).rounded(px(8.)).children(g).into_any_element());
        }
        rows.push(row(crate::i18n::t("主機"), div().w(px(200.)).child(Input::new(&d.host)).into_any_element()));
        rows.push(row("user.name", Input::new(&d.user).into_any_element()));
        rows.push(row("user.email", Input::new(&d.email).into_any_element()));
        rows.push(sub(&p, crate::i18n::t("什麼時候用這個身分（符合任一條）")).into_any_element());
        for (j, ru) in d.rules.iter().enumerate() {
            let remote = ru.t == "remote";
            rows.push(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .child(btn(&p, SharedString::from(format!("rt-{i}-{j}")), Bk::Normal, true, if remote { crate::i18n::t("remote 網址符合 ▾") } else { crate::i18n::t("資料夾在底下 ▾") }).w(px(160.)).on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(Modal::Settings(m)) = &mut this.modal
                            && let Some(r) = m.ids.get_mut(i).and_then(|d| d.rules.get_mut(j))
                        {
                            r.t = if r.t == "remote" { "dir".into() } else { "remote".into() };
                        }
                        cx.notify();
                    })))
                    .child(div().flex_1().child(Input::new(&ru.v)))
                    .child(btn(&p, SharedString::from(format!("rd-{i}-{j}")), Bk::Ghost, true, "×").tooltip_text(crate::i18n::t("刪除規則")).on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(Modal::Settings(m)) = &mut this.modal
                            && let Some(d) = m.ids.get_mut(i)
                            && j < d.rules.len()
                        {
                            d.rules.remove(j);
                        }
                        cx.notify();
                    })))
                    .into_any_element(),
            );
        }
        rows.push(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(btn(&p, SharedString::from(format!("ra-{i}")), Bk::Normal, true, crate::i18n::t("＋ 規則")).on_click(cx.listener(move |this, _, w, cx| this.settings_add_rule(i, w, cx))))
                .child(sub(&p, crate::i18n::tr("目前對應：{}", &[], &[(if matched.is_empty() { crate::i18n::t("（沒有開著的 repo 符合）").to_string() } else { matched.join(crate::i18n::t("、")) }).to_string()])))
                .into_any_element(),
        );
        div().flex().flex_col().gap(px(8.)).p(px(12.)).border_1().border_color(p.border).rounded(px(10.)).children(rows).into_any_element()
    }

    pub fn render_add_repo(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let p = self.pal;
        let Some(Modal::AddRepo(m)) = &self.modal else { return div().into_any_element() };
        let open: Vec<String> = self.repos.iter().map(|r| r.path.display().to_string().replace('/', "\\").to_lowercase()).collect();
        let fresh: Vec<String> = m.found.iter().filter(|f| !open.contains(&f.replace('/', "\\").to_lowercase())).cloned().collect();
        let all_picked = !fresh.is_empty() && fresh.iter().all(|f| m.picked.contains(f));
        let n_picked = m.picked.len();
        let pin = m.pin.clone();
        let busy = m.busy;
        let ids: Vec<(String, String)> = self.cfg.identities.iter().map(|i| (i.id.clone(), crate::i18n::tr("{}（{}）", &[], &[(i.name).to_string(), (if i.account.is_empty() { &i.email } else { &i.account }).to_string()]))).collect();
        let pin_label = ids.iter().find(|x| x.0 == pin).map(|x| x.1.clone()).unwrap_or_else(|| crate::i18n::t("自動（依規則）").into());
        let fresh2 = fresh.clone();
        let rows: Vec<AnyElement> = fresh
            .iter()
            .map(|f| {
                let on = m.picked.contains(f);
                let f2 = f.clone();
                div()
                    .id(SharedString::from(format!("ar-{f}")))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .px(px(4.))
                    .py(px(3.))
                    .rounded(px(5.))
                    .cursor_pointer()
                    .whitespace_nowrap()
                    .overflow_hidden()
                    .hover(move |s| s.bg(p.hover))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(Modal::AddRepo(m)) = &mut this.modal
                            && !m.picked.remove(&f2)
                        {
                            m.picked.insert(f2.clone());
                        }
                        cx.notify();
                    }))
                    .child(if on { "☑" } else { "☐" })
                    .child(div().flex_none().font_weight(gpui_kit::FontWeight::SEMIBOLD).child(base_name(f)))
                    .child(div().overflow_hidden().text_ellipsis().font_family("Cascadia Mono").child(sub(&p, f.clone())))
                    .into_any_element()
            })
            .collect();
        let body = div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(
                div()
                    .flex()
                    .gap(px(6.))
                    .items_center()
                    .child(div().flex_1().child(Input::new(&m.scan_dir)))
                    .child(btn(&p, "ar-scan", Bk::Normal, false, crate::i18n::t("掃描")).on_click(cx.listener(|this, _, _, cx| this.add_repo_scan(cx))))
                    .child(btn(&p, "ar-browse", Bk::Normal, false, crate::i18n::t("選資料夾…")).on_click(cx.listener(|this, _, w, cx| this.add_repo_browse(w, cx)))),
            )
            .child(
                div()
                    .flex()
                    .gap(px(8.))
                    .items_center()
                    .when(!fresh.is_empty(), |d| {
                        d.child(
                            div()
                                .id("ar-all")
                                .px(px(4.))
                                .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                .cursor_pointer()
                                .child(crate::i18n::tr("{} 全選", &[], &[(if all_picked { "☑" } else { "☐" }).to_string()]))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    if let Some(Modal::AddRepo(m)) = &mut this.modal {
                                        if all_picked {
                                            m.picked.clear();
                                        } else {
                                            m.picked = fresh2.iter().cloned().collect();
                                        }
                                    }
                                    cx.notify();
                                })),
                        )
                    })
                    .child(sub(&p, crate::i18n::tr("在這個資料夾底下找到 {} 個 repo，其中 {} 個還沒開{}", &[], &[(m.found.len()).to_string(), (fresh.len()).to_string(), (if n_picked > 0 { crate::i18n::tr("，已選 {n_picked} 個", &[("n_picked", (n_picked).to_string())], &[]) } else { String::new() }).to_string()]))),
            )
            .child(
                div()
                    .id("ar-list")
                    .max_h(px(260.))
                    .overflow_y_scroll()
                    .border_1()
                    .border_color(p.border)
                    .rounded(px(8.))
                    .p(px(6.))
                    .children(rows)
                    .when(fresh.is_empty(), |d| d.child(sub(&p, crate::i18n::t("（都已經開了）")))),
            )
            .child(div().flex().items_center().gap(px(8.)).child(sub(&p, crate::i18n::t("或"))).child(btn(&p, "ar-one", Bk::Normal, true, crate::i18n::t("直接選一個 repo 資料夾…")).on_click(cx.listener(|this, _, _, cx| this.add_repo_pick_one(cx)))))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .p(px(10.))
                    .border_1()
                    .border_color(p.border)
                    .rounded(px(8.))
                    .child(div().text_size(px(12.)).text_color(p.muted).child(crate::i18n::t("Clone 新的 repo")))
                    .child(Input::new(&m.url))
                    .child(div().flex().gap(px(6.)).child(div().flex_1().child(Input::new(&m.clone_to))).child(dim(btn(&p, "ar-clone", Bk::Normal, false, "Clone").when(!busy, |b| b.on_click(cx.listener(|this, _, _, cx| this.add_repo_clone(cx)))), busy))),
            )
            .when(!ids.is_empty(), |d| {
                d.child(self.labeled(
                    crate::i18n::t("身分"),
                    btn(&p, "ar-pin", Bk::Normal, false, format!("{pin_label} ▾"))
                        .on_click(cx.listener(move |this, ev: &ClickEvent, _, cx| {
                            let mut items = vec![MenuItem::new(crate::i18n::t("自動（依規則）"), |t, _, cx| {
                                if let Some(Modal::AddRepo(m)) = &mut t.modal {
                                    m.pin.clear();
                                }
                                cx.notify();
                            })];
                            for (id, name) in ids.clone() {
                                items.push(MenuItem::new(name, move |t, _, cx| {
                                    if let Some(Modal::AddRepo(m)) = &mut t.modal {
                                        m.pin = id.clone();
                                    }
                                    cx.notify();
                                }));
                            }
                            this.open_menu(ev.position(), items, false, cx);
                        }))
                        .into_any_element(),
                ))
            })
            .when_some(m.err.clone(), |d, e| d.child(div().text_size(px(12.)).text_color(p.danger).child(e)));
        let cancel = btn(&p, "ar-cancel", Bk::Normal, false, crate::i18n::t("取消")).on_click(cx.listener(|this, _, w, cx| this.close_modal(w, cx)));
        let add = dim(btn(&p, "ar-add", Bk::Primary, false, crate::i18n::tr("加入 {}", &[], &[(if n_picked > 0 { n_picked.to_string() } else { String::new() }).to_string()])).when(n_picked > 0 && !busy, |b| b.on_click(cx.listener(|this, _, w, cx| this.add_repo_add_picked(w, cx)))), n_picked == 0 || busy);
        self.dialog(div().child(crate::i18n::t("加入 repo")).into_any_element(), body.into_any_element(), vec![div().child(if busy { sub(&p, crate::i18n::t("處理中…")) } else { div() }).into_any_element(), cancel.into_any_element(), add.into_any_element()], 880., cx)
    }

    pub fn render_blame(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let p = self.pal;
        let Some(Modal::Blame(m)) = &self.modal else { return div().into_any_element() };
        let title = div().flex().gap_1().child(crate::i18n::t("Blame：")).child(div().font_family("Cascadia Mono").child(m.path.clone())).when_some(m.rev.clone(), |d, r| d.child(sub(&p, format!(" @ {}", short(&r))))).into_any_element();
        let body: AnyElement = if m.loading {
            sub(&p, crate::i18n::t("載入中…")).into_any_element()
        } else if let Some(e) = &m.err {
            div().text_size(px(12.)).text_color(p.danger).child(e.clone()).into_any_element()
        } else {
            let lines = std::sync::Arc::new(m.lines.clone());
            let n = lines.len();
            uniform_list("blame", n, move |range, _, _| {
                range
                    .map(|i| {
                        let l = &lines[i];
                        let first = i == 0 || lines[i - 1].sha != l.sha;
                        let alt = u8::from_str_radix(l.sha.get(0..2).unwrap_or("00"), 16).unwrap_or(0) % 2 == 1;
                        div()
                            .flex()
                            .h(px(20.))
                            .font_family("Cascadia Mono")
                            .text_size(px(12.5))
                            .when(alt, |d| d.bg(p.panel2))
                            .when(first, |d| d.border_t_1().border_color(p.border))
                            .child(
                                div()
                                    .w(px(300.))
                                    .flex_none()
                                    .px(px(8.))
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .text_ellipsis()
                                    .text_size(px(12.))
                                    .text_color(p.muted)
                                    .font_family("Segoe UI")
                                    .child(if first { format!("{} {} · {}", short(&l.sha), l.author, ago(l.time)) } else { String::new() }),
                            )
                            .child(div().w(px(48.)).flex_none().text_right().pr(px(8.)).border_r_1().border_color(p.border).text_color(p.faint).child(l.line.to_string()))
                            .child(div().pl(px(10.)).whitespace_nowrap().child(l.text.clone()))
                            .into_any_element()
                    })
                    .collect()
            })
            .h(px(520.))
            .into_any_element()
        };
        let _ = date_time(0);
        let close = btn(&p, "bl-close", Bk::Normal, false, crate::i18n::t("關閉")).on_click(cx.listener(|this, _, w, cx| this.close_modal(w, cx)));
        self.dialog(title, div().border_1().border_color(p.border).rounded(px(8.)).overflow_hidden().child(body).into_any_element(), vec![close.into_any_element()], 1100., cx)
    }
}
