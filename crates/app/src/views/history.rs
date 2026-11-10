//! The history view (HistoryView.vue): search, commit list with graph, commit details.

use std::sync::Arc;

use gitcore::parse::Commit;
use gpui_kit::component::input::Input;
use gpui_kit::{
    AnyElement, Bounds, Context, Hsla, InteractiveElement as _, IntoElement, MouseButton, ParentElement as _, PathBuilder, SharedString,
    StatefulInteractiveElement as _, Styled as _, Window, canvas, div, fill, point, prelude::FluentBuilder as _, px, size, uniform_list,
};

use crate::app::*;
use crate::format::{ago, date_time, short};
use crate::graph::GraphRow;
use crate::model::*;
use crate::theme::{Pal, lane_color};
use crate::tree::TreeFile;
use crate::ui::*;
use crate::views::{SplitKind, diffpane::PaneMode, on_split, splitter};

const ROW: f32 = 28.;
const LANE: f32 = 16.;
const PAD: f32 = 9.;
const TAIL: f32 = 7.;

fn initials(name: &str) -> String {
    name.split_whitespace().filter_map(|w| w.chars().next()).take(2).collect::<String>().to_uppercase()
}

fn kind_of(k: &str) -> &str {
    match k {
        "head" => "head",
        "remote" => "remote",
        "tag" => "tag",
        "detached" => "detached",
        _ => "",
    }
}

/// One row's slice of the graph: lines through the row and the dot.
fn graph_cell(row: GraphRow, width: f32, ring: Hsla) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            let x = |c: usize| bounds.origin.x + px(PAD + c as f32 * LANE);
            let y = |k: u8| bounds.origin.y + px(ROW * k as f32 / 2.);
            for s in &row.segs {
                let (x1, x2, y1, y2) = (x(s.x1), x(s.x2), y(s.y1), y(s.y2));
                let mut pb = PathBuilder::stroke(px(2.));
                pb.move_to(point(x1, y1));
                if s.x1 == s.x2 {
                    pb.line_to(point(x2, y2));
                } else {
                    let mid = (y1 + y2) / 2.;
                    pb.cubic_bezier_to(point(x2, y2), point(x1, mid), point(x2, mid));
                }
                if let Ok(path) = pb.build() {
                    window.paint_path(path, lane_color(s.color));
                }
            }
            let c = point(x(row.col), y(1));
            window.paint_quad(fill(Bounds::centered_at(c, size(px(12.), px(12.))), ring).corner_radii(px(6.)));
            window.paint_quad(fill(Bounds::centered_at(c, size(px(9.), px(9.))), lane_color(row.color)).corner_radii(px(4.5)));
        },
    )
    .w(px(width))
    .h(px(ROW))
    .flex_none()
}

impl GitTabs {
    pub fn pick_commit(&mut self, sha: String, cx: &mut Context<Self>) {
        let Some(r) = self.repo_mut() else { return };
        r.sel_commit = Some(sha);
        let p = r.path.clone();
        self.load_detail(&p, cx);
        cx.notify();
    }

    fn commit_menu(&self, sha: &str) -> Vec<MenuItem> {
        let Some(r) = self.repo() else { return vec![] };
        let branch = r.summary.as_ref().and_then(|s| s.branch.clone()).unwrap_or_else(|| "HEAD".into());
        let subject = r.commits.iter().find(|c| c.sha == sha).map(|c| c.subject.clone()).unwrap_or_default();
        let (a, b, c, d, e, f, g) = (sha.to_string(), sha.to_string(), sha.to_string(), sha.to_string(), sha.to_string(), sha.to_string(), sha.to_string());
        vec![
            MenuItem::new(crate::i18n::t("Checkout 這個 commit…"), move |t, _, cx| t.checkout_commit(a.clone(), cx)),
            MenuItem::new(crate::i18n::t("從這裡開新分支…"), move |t, w, cx| t.new_branch(Some(b.clone()), w, cx)),
            MenuItem::new(crate::i18n::t("Cherry-pick 到目前分支"), move |t, _, cx| t.cherry_pick(c.clone(), cx)),
            MenuItem::new("Revert…", move |t, _, cx| t.revert(d.clone(), cx)),
            MenuItem::new(crate::i18n::tr("Reset {branch} 到這裡…", &[("branch", (branch).to_string())], &[]), move |t, _, cx| t.reset_to(e.clone(), cx)).danger(),
            MenuItem::new(crate::i18n::t("加 Tag…"), move |t, w, cx| t.create_tag(f.clone(), w, cx)),
            MenuItem::sep(),
            MenuItem::new(crate::i18n::t("複製 SHA"), {
                let s = g.clone();
                move |t, _, cx| t.copy(s.clone(), &short(&s), cx)
            }),
            MenuItem::new(crate::i18n::t("複製標題"), move |t, _, cx| t.copy(subject.clone(), crate::i18n::t("標題"), cx)),
        ]
    }

    pub fn render_history(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let _ = window;
        let p = self.pal;
        let Some(r) = self.repo() else { return div().into_any_element() };
        let commits: Arc<Vec<Commit>> = r.commits.clone();
        let graph = r.graph.clone();
        let (log_path, log_done, log_loading, query) = (r.log_path.clone(), r.log_done, r.log_loading, r.log_query.clone());
        let path = r.path.clone();
        let sel_commit = r.sel_commit.clone();
        let detail = r.detail.clone();
        let nothing = commits.is_empty() && !log_loading;
        let count = commits.len();

        // ---- list geometry
        let list_w = f32::from(self.window_size.width) - self.sizes.side_bar - 5.;
        let (show_author, show_date) = (list_w >= 720., list_w >= 520.);
        let gw = graph.as_ref().map_or(16., |g| (g.iter().map(|x| x.width).max().unwrap_or(1).clamp(1, 10) - 1) as f32 * LANE + PAD + TAIL);
        let owner = cx.entity().downgrade();
        let list = uniform_list("commits", count, {
            let (commits, graph, sel, owner, path) = (commits.clone(), graph.clone(), sel_commit.clone(), owner.clone(), path.clone());
            move |range, window, cx| {
                // Near the end of what is loaded: fetch the next page.
                if !log_done && !log_loading && range.end + 30 >= commits.len() {
                    let (o, pth) = (owner.clone(), path.clone());
                    window.defer(cx, move |_, cx| {
                        let _ = o.update(cx, |this, cx| this.load_log(&pth, false, cx));
                    });
                }
                range
                    .map(|ix| {
                        let c = &commits[ix];
                        let on = sel.as_deref() == Some(c.sha.as_str());
                        let (sha, sha2) = (c.sha.clone(), c.sha.clone());
                        let (o1, o2) = (owner.clone(), owner.clone());
                        div()
                            .id(SharedString::from(format!("c-{}", c.sha)))
                            .flex()
                            .w_full()
                            .items_center()
                            .gap(px(6.))
                            .h(px(ROW))
                            .pr(px(10.))
                            .cursor_pointer()
                            .bg(if on { p.accent_bg } else { Hsla::transparent_black() })
                            .hover(move |s| s.bg(if on { p.accent_bg } else { p.hover }))
                            .on_click(move |_, _, cx| {
                                let s = sha.clone();
                                let _ = o1.update(cx, |this, cx| this.pick_commit(s, cx));
                            })
                            .on_mouse_down(MouseButton::Right, move |ev, _, cx| {
                                let (s, pos) = (sha2.clone(), ev.position);
                                let _ = o2.update(cx, |this, cx| {
                                    this.pick_commit(s.clone(), cx);
                                    let items = this.commit_menu(&s);
                                    this.open_menu(pos, items, false, cx);
                                });
                            })
                            .child(match graph.as_ref().and_then(|g| g.get(ix)) {
                                Some(g) => graph_cell(g.clone(), gw, if on { p.accent_bg } else { p.panel }).into_any_element(),
                                None => div().w(px(gw)).flex_none().text_color(p.accent).text_size(px(10.)).text_center().child("●").into_any_element(),
                            })
                            .child(
                                div()
                                    .flex_1()
                                    .min_w(px(120.))
                                    .flex()
                                    .items_center()
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .children(c.refs.iter().map(|x| ref_pill(&p, &x.name, kind_of(x.kind))))
                                    .child(div().overflow_hidden().text_ellipsis().child(c.subject.clone())),
                            )
                            .when(show_author, |d| d.child(div().w(px(110.)).flex_none().overflow_hidden().text_ellipsis().whitespace_nowrap().text_size(px(12.)).text_color(p.muted).child(c.author.clone())))
                            .child(div().w(px(64.)).flex_none().text_size(px(12.)).text_color(p.muted).font_family("Cascadia Mono").child(short(&c.sha)))
                            .when(show_date, |d| d.child(div().w(px(88.)).flex_none().overflow_hidden().whitespace_nowrap().text_size(px(12.)).text_color(p.muted).child(ago(c.time))))
                            .into_any_element()
                    })
                    .collect()
            }
        })
        .size_full()
        .track_scroll(&self.commit_scroll);

        // ---- search bar
        let cbar = div()
            .flex()
            .flex_none()
            .flex_wrap()
            .items_center()
            .gap(px(6.))
            .px(px(10.))
            .py(px(6.))
            .min_h(px(41.))
            .border_b_1()
            .border_color(p.border)
            .child(div().w(px(340.)).child(Input::new(&self.search)))
            .when_some(log_path, |d, lp| {
                d.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_1()
                        .pl(px(10.))
                        .pr(px(4.))
                        .py(px(2.))
                        .rounded(px(12.))
                        .bg(p.accent_bg)
                        .text_color(p.accent_tx)
                        .text_size(px(12.))
                        .child(div().flex().gap_1().child(crate::i18n::t("只看")).child(div().font_family("Cascadia Mono").child(lp)))
                        .child(div().id("clear-fh").px(px(4.)).cursor_pointer().child("×").on_click(cx.listener(|this, _, _, cx| this.clear_file_history(cx)))),
                )
            })
            .child(sub(&p, crate::i18n::tr("{count}{} 個 commit", &[("count", (count).to_string())], &[(if log_done { "" } else { "+" }).to_string()])))
            .when(log_loading, |d| d.child(sub(&p, crate::i18n::t("載入中…"))));

        // ---- detail pane
        let tab = self.hist_tab;
        let files_n = detail.as_ref().map(|d| d.files.len());
        let tabs = div()
            .flex()
            .flex_none()
            .gap(px(2.))
            .px(px(10.))
            .border_b_1()
            .border_color(p.border)
            .child(self.hist_tab_button("tab-commit", "Commit", None, tab == HistTab::Commit, HistTab::Commit, cx))
            .child(self.hist_tab_button("tab-changes", "Changes", files_n, tab == HistTab::Changes, HistTab::Changes, cx));
        let body: AnyElement = match detail {
            None => empty(&p, "", crate::i18n::t("選一個 commit"), None),
            Some(d) if tab == HistTab::Commit => self.render_commit_tab(d, cx),
            Some(d) => self.render_changes_tab(d, cx),
        };
        let _ = nothing;
        let empty_msg = if count == 0 && !log_loading {
            Some(if query.is_empty() { crate::i18n::t("還沒有任何 commit") } else { crate::i18n::t("找不到符合的 commit") })
        } else {
            None
        };
        let hist_top = self.sizes.hist_top;
        on_split(div().id("hist").flex().flex_col().flex_1().min_h_0(), SplitKind::HistTop, cx)
            .child(cbar)
            .child(
                div()
                    .id("commit-list")
                    .key_context("Nav")
                    .track_focus(&self.nav_focus)
                    .flex_none()
                    .h(px(hist_top))
                    .max_h(gpui_kit::relative(0.8))
                    .relative()
                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, w, cx| this.nav_focus.focus(w, cx)))
                    .on_action(cx.listener(|this, _: &NextFile, _, cx| this.move_commit(1, cx)))
                    .on_action(cx.listener(|this, _: &PreviousFile, _, cx| this.move_commit(-1, cx)))
                    .child(list)
                    .when_some(empty_msg, |d, m| d.child(div().absolute().inset_0().flex().items_center().justify_center().text_color(p.muted).child(m))),
            )
            .child(splitter(&p, "split-hist", SplitKind::HistTop, false))
            .child(div().flex().flex_col().flex_1().min_h_0().min_w_0().child(tabs).child(body))
            .into_any_element()
    }

    fn hist_tab_button(&self, id: &'static str, label: &str, n: Option<usize>, on: bool, t: HistTab, cx: &mut Context<Self>) -> AnyElement {
        let p = self.pal;
        div()
            .id(id)
            .flex()
            .items_center()
            .gap_1()
            .px(px(14.))
            .pt(px(8.))
            .pb(px(6.))
            .cursor_pointer()
            .border_b_2()
            .border_color(if on { p.accent } else { Hsla::transparent_black() })
            .text_color(if on { p.accent } else { p.muted })
            .child(label.to_string())
            .when_some(n, |d, n| d.child(div().px(px(6.)).rounded(px(9.)).bg(p.panel2).text_color(p.muted).text_size(px(11.)).child(n.to_string())))
            .on_click(cx.listener(move |this, _, _, cx| this.set_hist_tab(t, cx)))
            .into_any_element()
    }

    fn render_commit_tab(&mut self, d: gitcore::ops::CommitDetail, cx: &mut Context<Self>) -> AnyElement {
        let p = self.pal;
        let subject = d.message.lines().next().unwrap_or("").to_string();
        let body = d.message.lines().skip(1).collect::<Vec<_>>().join("\n").trim().to_string();
        let sha = d.sha.clone();
        let (s1, s2, s3, s4, s5, s6) = (sha.clone(), sha.clone(), sha.clone(), sha.clone(), sha.clone(), sha.clone());
        let kv = |k: &'static str, v: AnyElement| div().flex().gap(px(10.)).items_center().child(div().w(px(72.)).flex_none().child(k)).child(v);
        let act = |id: &'static str, label: &'static str| btn(&p, id, Bk::Normal, true, label);
        div()
            .id("commit-tab")
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .track_scroll(&self.detail_scroll)
            .px(px(18.))
            .py(px(14.))
            .gap(px(10.))
            .child(
                div()
                    .flex()
                    .gap(px(12.))
                    .items_center()
                    .child(div().size(px(40.)).flex_none().rounded(px(6.)).bg(gpui_kit::rgb(0xf2a65a)).text_color(gpui_kit::white()).flex().items_center().justify_center().font_weight(gpui_kit::FontWeight::SEMIBOLD).child(initials(&d.author)))
                    .child(
                        div()
                            .child(div().flex().gap_1().child(div().font_weight(gpui_kit::FontWeight::SEMIBOLD).child(d.author.clone())).child(sub(&p, d.email.clone())))
                            .child(sub(&p, format!("{}{}", date_time(d.time), if d.committer != d.author { crate::i18n::tr(" · commit 者 {}", &[], &[(d.committer).to_string()]) } else { String::new() }))),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .text_size(px(12.))
                    .text_color(p.muted)
                    .when(!d.refs.is_empty(), |c| c.child(kv("REFS", div().flex().flex_wrap().text_color(p.text).children(d.refs.iter().map(|x| ref_pill(&p, &x.name, kind_of(x.kind)))).into_any_element())))
                    .child(kv(
                        "SHA",
                        div()
                            .flex()
                            .gap_2()
                            .text_color(p.text)
                            .font_family("Cascadia Mono")
                            .child(sha.clone())
                            .child(div().id("copy-sha").text_color(p.accent).cursor_pointer().child(crate::i18n::t("複製")).on_click(cx.listener(move |this, _, _, cx| this.copy(s1.clone(), &short(&s1), cx))))
                            .into_any_element(),
                    ))
                    .child(kv(
                        "PARENTS",
                        div()
                            .flex()
                            .gap(px(8.))
                            .text_color(p.text)
                            .font_family("Cascadia Mono")
                            .children(d.parents.iter().map(|pa| {
                                let pa2 = pa.clone();
                                div().id(SharedString::from(format!("par-{pa}"))).text_color(p.accent).cursor_pointer().child(short(pa)).on_click(cx.listener(move |this, _, _, cx| this.pick_commit(pa2.clone(), cx)))
                            }))
                            .when(d.parents.is_empty(), |c| c.child(crate::i18n::t("（第一個 commit）")))
                            .into_any_element(),
                    )),
            )
            .child(div().mt(px(6.)).text_size(px(15.)).font_weight(gpui_kit::FontWeight::SEMIBOLD).child(subject))
            .when(!body.is_empty(), |c| c.child(div().font_family("Cascadia Mono").text_size(px(12.5)).text_color(p.muted).child(body)))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(6.))
                    .child(act("a-cp", "Cherry-pick").on_click(cx.listener(move |this, _, _, cx| this.cherry_pick(s2.clone(), cx))))
                    .child(act("a-rv", "Revert").on_click(cx.listener(move |this, _, _, cx| this.revert(s3.clone(), cx))))
                    .child(act("a-co", "Checkout").on_click(cx.listener(move |this, _, _, cx| this.checkout_commit(s4.clone(), cx))))
                    .child(act("a-br", crate::i18n::t("開分支")).on_click(cx.listener({
                        let s = sha.clone();
                        move |this, _, w, cx| this.new_branch(Some(s.clone()), w, cx)
                    })))
                    .child(act("a-tg", crate::i18n::t("加 Tag")).on_click(cx.listener(move |this, _, w, cx| this.create_tag(s5.clone(), w, cx))))
                    .child(act("a-rs", crate::i18n::t("Reset 到這裡…")).on_click(cx.listener(move |this, _, _, cx| this.reset_to(s6.clone(), cx)))),
            )
            .child(
                div()
                    .border_t_1()
                    .border_color(p.border)
                    .pt(px(6.))
                    .children(d.files.iter().map(|f| {
                        let path = f.path.clone();
                        div()
                            .id(SharedString::from(format!("hf-{}", f.path)))
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .px(px(4.))
                            .py(px(3.))
                            .rounded(px(5.))
                            .cursor_pointer()
                            .whitespace_nowrap()
                            .hover(move |s| s.bg(p.hover))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.select(ListKind::C, path.clone(), cx);
                                this.set_hist_tab(HistTab::Changes, cx);
                            }))
                            .child(st_badge(&p, f.status))
                            .child(div().overflow_hidden().text_ellipsis().text_size(px(12.)).font_family("Cascadia Mono").child(f.path.clone()))
                            .child(div().flex_1())
                            .when_some(f.add, |d, a| d.child(div().text_size(px(12.)).text_color(p.ok).child(format!("+{a}"))))
                            .when_some(f.del, |d, a| d.child(div().text_size(px(12.)).text_color(p.danger).child(format!("−{a}"))))
                    }))
                    .when(d.files.is_empty(), |c| c.child(div().px(px(12.)).py(px(6.)).child(sub(&p, crate::i18n::t("沒有檔案變更"))))),
            )
            .into_any_element()
    }

    fn render_changes_tab(&mut self, d: gitcore::ops::CommitDetail, cx: &mut Context<Self>) -> AnyElement {
        let p = self.pal;
        let files: Vec<TreeFile> = d.files.iter().map(|f| TreeFile { path: f.path.clone(), status: f.status }).collect();
        let sel = self.repo().and_then(|r| r.sel.clone());
        let sel_c = sel.as_ref().filter(|s| s.list == ListKind::C).map(|s| s.path.clone());
        let tree = self.render_filetree(ListKind::C, &files, sel_c.as_deref(), cx);
        let status = sel.as_ref().and_then(|s| d.files.iter().find(|f| f.path == s.path)).map(|f| f.status);
        let hf = self.sizes.hist_files;
        let right: AnyElement = match status {
            Some(st) => div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .min_h_0()
                .child(self.render_diff_head(st, vec![], cx))
                .child(self.render_diff_area(PaneMode::ReadOnly, cx))
                .into_any_element(),
            None => empty(&p, "", crate::i18n::t("選一個檔案"), None),
        };
        on_split(div().id("chtab").flex().flex_1().min_h_0(), SplitKind::HistFiles, cx)
            .child(div().id("hist-tree").flex_none().w(px(hf)).h_full().overflow_y_scroll().py(px(6.)).child(tree))
            .child(splitter(&p, "split-hf", SplitKind::HistFiles, true))
            .child(right)
            .into_any_element()
    }
}
