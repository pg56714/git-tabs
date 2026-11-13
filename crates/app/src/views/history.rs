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
use crate::theme::lane_color;
use crate::tree::TreeFile;
use crate::ui::*;
use crate::views::{SplitKind, diffpane::PaneMode, on_split, splitter};

const ROW: f32 = 24.;
const LANE: f32 = 12.;
const PAD: f32 = 10.;
const TAIL: f32 = 8.;
/// The text stops moving right after this many lanes.
const MAX_LANES: usize = 16;

/// How wide a row's graph is: the text starts right after the row's own lanes, as in Fork.
fn cell_width(row: &GraphRow) -> f32 {
    PAD + (row.width.clamp(1, MAX_LANES) - 1) as f32 * LANE + TAIL
}

fn initials(name: &str) -> String {
    name.split_whitespace().filter_map(|w| w.chars().next()).take(2).collect::<String>().to_uppercase()
}

/// One row's slice of the graph, drawn the way Fork does: lanes run straight down, a lane that
/// joins or leaves the commit turns into its row with a rounded corner, and a merge has a hollow dot.
fn graph_cell(row: GraphRow, width: f32, bg: Hsla, dot: bool) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            let (ox, oy) = (f32::from(bounds.origin.x), f32::from(bounds.origin.y));
            let pt = |x: f32, y: f32| point(px(ox + x), px(oy + y));
            let lane_x = |c: usize| PAD + c as f32 * LANE;
            let mid = ROW / 2.;
            // How far a cubic's control points sit from its ends to draw a quarter circle.
            const K: f32 = 0.5523;
            for s in &row.segs {
                let (x1, x2) = (lane_x(s.x1), lane_x(s.x2));
                let mut pb = PathBuilder::stroke(px(2.));
                if s.x1 == s.x2 {
                    pb.move_to(pt(x1, if s.y1 == 0 { 0. } else { mid }));
                    pb.line_to(pt(x2, if s.y2 == 2 { ROW } else { mid }));
                } else {
                    let dir = (x2 - x1).signum();
                    let r = (x2 - x1).abs().min(mid).min(9.);
                    if s.y1 == 0 {
                        // From a lane above into the dot: down, round the corner, across.
                        pb.move_to(pt(x1, 0.));
                        pb.line_to(pt(x1, mid - r));
                        pb.cubic_bezier_to(pt(x1 + dir * r, mid), pt(x1, mid - r + K * r), pt(x1 + dir * r * (1. - K), mid));
                        if (x2 - x1).abs() - r > 0.1 {
                            pb.line_to(pt(x2, mid));
                        }
                    } else {
                        // From the dot to a parent's lane: across, round the corner, down.
                        pb.move_to(pt(x1, mid));
                        if (x2 - x1).abs() - r > 0.1 {
                            pb.line_to(pt(x2 - dir * r, mid));
                        }
                        pb.cubic_bezier_to(pt(x2, mid + r), pt(x2 - dir * r * (1. - K), mid), pt(x2, mid + r - K * r));
                        pb.line_to(pt(x2, ROW));
                    }
                }
                if let Ok(path) = pb.build() {
                    window.paint_path(path, lane_color(s.color));
                }
            }
            // A merge's dot is a button of its own (it folds the merge), drawn over this cell.
            if dot {
                let c = pt(lane_x(row.col), mid);
                let disc = |d: f32, color: Hsla| fill(Bounds::centered_at(c, size(px(d), px(d))), color).corner_radii(px(d / 2.));
                window.paint_quad(disc(10., bg));
                window.paint_quad(disc(8., lane_color(row.color)));
            }
        },
    )
    .w(px(width))
    .h(px(ROW))
    .flex_none()
}

impl GitTabs {
    /// Whether the open repo's remote is on github.com: remote branches then carry GitHub's mark.
    pub fn on_github(&self) -> bool {
        self.repo().and_then(|r| r.summary.as_ref()).and_then(|s| s.remote_url.as_deref()).is_some_and(|u| u.contains("github.com"))
    }

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
        let (w1, w2, w3) = (sha.to_string(), sha.to_string(), sha.to_string());
        // A commit can be rewritten when it is the checked-out branch's own and not a merge.
        let ix = r.commits.iter().position(|c| c.sha == sha);
        let row = ix.and_then(|i| r.graph.as_ref().and_then(|g| g.get(i)));
        let own = r.summary.as_ref().is_some_and(|s| s.branch.is_some()) && row.is_some_and(|g| !g.off_head && !g.merge);
        let has_parent = ix.is_some_and(|i| !r.commits[i].parents.is_empty());
        vec![
            MenuItem::new(crate::i18n::t("Checkout 這個 commit…"), move |t, _, cx| t.checkout_commit(a.clone(), cx)),
            MenuItem::new(crate::i18n::t("從這裡開新分支…"), move |t, w, cx| t.new_branch(Some(b.clone()), w, cx)),
            MenuItem::new(crate::i18n::t("Cherry-pick 到目前分支"), move |t, _, cx| t.cherry_pick(c.clone(), cx)),
            MenuItem::new("Revert…", move |t, _, cx| t.revert(d.clone(), cx)),
            MenuItem::new(crate::i18n::tr("Reset {branch} 到這裡…", &[("branch", (branch).to_string())], &[]), move |t, _, cx| t.reset_to(e.clone(), cx)).danger(),
            MenuItem::new(crate::i18n::t("加 Tag…"), move |t, w, cx| t.create_tag(f.clone(), w, cx)),
            MenuItem::sep(),
            MenuItem::new(crate::i18n::t("改訊息…"), move |t, w, cx| t.reword_commit(w1.clone(), w, cx)).disabled(!own),
            MenuItem::new(crate::i18n::t("併入上一個 commit…"), move |t, _, cx| t.squash_commit(w2.clone(), cx)).disabled(!own || !has_parent),
            MenuItem::new(crate::i18n::t("丟掉這個 commit…"), move |t, _, cx| t.drop_commit(w3.clone(), cx)).danger().disabled(!own),
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
        let gh = self.on_github();
        let Some(r) = self.repo() else { return div().into_any_element() };
        let commits: Arc<Vec<Commit>> = r.commits.clone();
        let graph = r.graph.clone();
        let (log_path, log_done, log_loading, query) = (r.log_path.clone(), r.log_done, r.log_loading, r.log_query.clone());
        let path = r.path.clone();
        let sel_commit = r.sel_commit.clone();
        let detail = r.detail.clone();
        let nothing = commits.is_empty() && !log_loading;
        // The rows on screen: every commit, or what is left while a merge is folded.
        let shown = r.shown.clone();
        let collapsed = Arc::new(r.collapsed.clone());
        let unpushed = r.unpushed.clone();
        let count = shown.as_ref().map_or(commits.len(), |v| v.len());

        // ---- list geometry
        let list_w = f32::from(self.window_size.width) - self.sizes.side_bar - 5.;
        let (show_author, show_date) = (list_w >= 720., list_w >= 520.);
        let owner = cx.entity().downgrade();
        let list = uniform_list("commits", count, {
            let (commits, graph, sel, owner, path) = (commits.clone(), graph.clone(), sel_commit.clone(), owner.clone(), path.clone());
            let (shown, collapsed, unpushed) = (shown.clone(), collapsed.clone(), unpushed.clone());
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
                        let at = shown.as_ref().map_or(ix, |v| v[ix]);
                        let Some(c) = commits.get(at) else { return div().into_any_element() };
                        // Not on any remote yet: a dot between the graph and the text, as in Fork (white on the
                        // selected row, to stay visible).
                        let ahead = unpushed.get(at).copied().unwrap_or(false).then(|| if sel.as_deref() == Some(c.sha.as_str()) { gpui_kit::white() } else { p.accent });
                        let on = sel.as_deref() == Some(c.sha.as_str());
                        let (sha, sha2) = (c.sha.clone(), c.sha.clone());
                        let (o1, o2) = (owner.clone(), owner.clone());
                        let lane = graph.as_ref().and_then(|g| g.get(ix));
                        // Commits HEAD does not reach are greyed, as in Fork. (A merge is not: only its dot differs.)
                        let dim = !on && lane.is_some_and(|g| g.off_head);
                        div()
                            .id(SharedString::from(format!("c-{}", c.sha)))
                            .flex()
                            .w_full()
                            .items_center()
                            .gap(px(6.))
                            .h(px(ROW))
                            .pr(px(10.))
                            .cursor_pointer()
                            .bg(if on { p.accent } else { Hsla::transparent_black() })
                            .when(on, |d| d.text_color(gpui_kit::white()))
                            .hover(move |s| s.bg(if on { p.accent } else { p.hover }))
                            .on_click(move |ev, _, cx| {
                                let s = sha.clone();
                                // Double-click checks the commit out (Fork's "Checkout Commit" dialog).
                                if ev.click_count() >= 2 {
                                    let _ = o1.update(cx, |this, cx| this.checkout_commit(s, cx));
                                } else {
                                    let _ = o1.update(cx, |this, cx| this.pick_commit(s, cx));
                                }
                            })
                            .on_mouse_down(MouseButton::Right, move |ev, _, cx| {
                                let (s, pos) = (sha2.clone(), ev.position);
                                let _ = o2.update(cx, |this, cx| {
                                    this.pick_commit(s.clone(), cx);
                                    let items = this.commit_menu(&s);
                                    this.open_menu(pos, items, false, cx);
                                });
                            })
                            .child(match lane {
                                // A merge: its dot folds away what the merge brought in, and shows it again.
                                Some(g) if c.parents.len() > 1 => {
                                    let (bg, color, folded) = (if on { p.accent } else { p.panel }, lane_color(g.color), collapsed.contains(&c.sha));
                                    let (o5, s5) = (owner.clone(), c.sha.clone());
                                    div()
                                        .relative()
                                        .flex_none()
                                        .w(px(cell_width(g)))
                                        .h(px(ROW))
                                        .child(graph_cell(g.clone(), cell_width(g), bg, false))
                                        .child(
                                            div()
                                                .id(SharedString::from(format!("fold-{}", c.sha)))
                                                .absolute()
                                                .left(px(PAD + g.col as f32 * LANE - 7.))
                                                .top(px(ROW / 2. - 7.))
                                                .size(px(14.))
                                                .rounded(px(7.))
                                                .border_1()
                                                .border_color(color)
                                                .bg(if folded { color } else { bg })
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .cursor_pointer()
                                                .tooltip_text(if folded { crate::i18n::t("展開這次 merge 帶進來的 commit") } else { crate::i18n::t("收合這次 merge 帶進來的 commit") })
                                                .on_click(move |_, _, cx| {
                                                    // Not also a click on the row, which would select the commit.
                                                    cx.stop_propagation();
                                                    let s = s5.clone();
                                                    let _ = o5.update(cx, |this, cx| this.toggle_merge(s, cx));
                                                })
                                                .child(icon(if folded { Ico::ChevronRight } else { Ico::ChevronDown }, 10., if folded { gpui_kit::white() } else { color })),
                                        )
                                        .into_any_element()
                                }
                                Some(g) => graph_cell(g.clone(), cell_width(g), if on { p.accent } else { p.panel }, true).into_any_element(),
                                None => div().w(px(PAD + TAIL)).flex_none().text_color(p.accent).text_size(px(10.)).text_center().child("●").into_any_element(),
                            })
                            .child(
                                div()
                                    .flex_1()
                                    .min_w(px(120.))
                                    .flex()
                                    .items_center()
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    // As in Fork: only a row with a dot makes room for it, the others sit close to the graph.
                                    .when_some(ahead, |d, color| d.child(div().flex_none().size(px(7.)).rounded(px(3.5)).ml(px(2.)).mr(px(7.)).bg(color)))
                                    .children({
                                        // Branch labels act like Fork's: double-click switches, right-click opens the branch menu.
                                        let make = |pill: gpui_kit::Div, id: String, name: String, kind: &'static str| {
                                            let (o3, o4) = (owner.clone(), owner.clone());
                                            let (pa, pb) = (path.clone(), path.clone());
                                            let (n1, n2) = (name.clone(), name);
                                            pill.id(SharedString::from(id))
                                                .cursor_pointer()
                                                .on_click(move |ev, _, cx| {
                                                    if ev.click_count() >= 2 {
                                                        // Not also a double-click on the commit row (which would offer a checkout).
                                                        cx.stop_propagation();
                                                        if kind != "head" {
                                                            let (pa, n) = (pa.clone(), n1.clone());
                                                            let _ = o3.update(cx, |this, cx| this.switch_branch(&pa, &n, cx));
                                                        }
                                                    }
                                                })
                                                .on_mouse_down(MouseButton::Right, move |ev, _, cx| {
                                                    cx.stop_propagation();
                                                    let (pos, pb, n) = (ev.position, pb.clone(), n2.clone());
                                                    let _ = o4.update(cx, |this, cx| {
                                                        let items = if kind == "remote" { this.remote_item_menu(&pb, &n) } else { this.branch_item_menu(&pb, &n, kind == "head") };
                                                        this.open_menu(pos, items, false, cx);
                                                    });
                                                })
                                                .into_any_element()
                                        };
                                        ref_views(&c.refs)
                                            .into_iter()
                                            .enumerate()
                                            .map(|(k, v)| {
                                                let id = format!("pill-{ix}-{k}");
                                                match v {
                                                    RefView::Branch { name, current, remotes } => {
                                                        make(ref_group(&p, name, current, remotes, gh), id, name.to_string(), if current { "head" } else { "branch" })
                                                    }
                                                    RefView::Remote(name) => make(ref_pill(&p, name, "remote", gh), id, name.to_string(), "remote"),
                                                    RefView::Detached => ref_pill(&p, "", "detached", gh).into_any_element(),
                                                    RefView::Tag(name) => ref_pill(&p, name, "tag", gh).into_any_element(),
                                                }
                                            })
                                            .collect::<Vec<AnyElement>>()
                                    })
                                    .child({
                                        // Black for what the checked-out branch has, clearly lighter for the rest; the
                                        // `fix(scope):` prefix in bold. Both as Fork draws them.
                                        let (head, rest) = crate::format::split_subject(&c.subject);
                                        div()
                                            .flex()
                                            .items_center()
                                            .min_w_0()
                                            .overflow_hidden()
                                            .when(dim, |d| d.text_color(p.faint))
                                            .when(!head.is_empty(), |d| d.child(div().flex_none().mr(px(4.)).font_weight(gpui_kit::FontWeight::SEMIBOLD).child(head.to_string())))
                                            .child(div().min_w_0().overflow_hidden().text_ellipsis().child(rest.to_string()))
                                    }),
                            )
                            .when(show_author, |d| d.child(div().w(px(110.)).flex_none().overflow_hidden().text_ellipsis().whitespace_nowrap().text_size(px(12.)).text_color(if on { gpui_kit::white().opacity(0.85) } else { p.muted }).child(c.author.clone())))
                            .child(div().w(px(64.)).flex_none().text_size(px(12.)).text_color(if on { gpui_kit::white().opacity(0.85) } else { p.muted }).font_family("Cascadia Mono").child(short(&c.sha)))
                            .when(show_date, |d| d.child(div().w(px(88.)).flex_none().overflow_hidden().whitespace_nowrap().text_size(px(12.)).text_color(if on { gpui_kit::white().opacity(0.85) } else { p.muted }).child(ago(c.time))))
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
            .child(self.hist_tab_button("tab-changes", "Changes", files_n, tab == HistTab::Changes, HistTab::Changes, cx))
            .child(self.hist_tab_button("tab-tree", "File Tree", None, tab == HistTab::Tree, HistTab::Tree, cx));
        let body: AnyElement = match detail {
            None => empty(&p, "", crate::i18n::t("選一個 commit"), None),
            Some(d) if tab == HistTab::Commit => self.render_commit_tab(d, cx),
            Some(d) if tab == HistTab::Tree => self.render_tree_tab(d, cx),
            Some(d) => self.render_changes_tab(d, cx),
        };
        let _ = nothing;
        let empty_msg = if count == 0 && !log_loading {
            Some(if query.is_empty() { crate::i18n::t("還沒有任何 commit") } else { crate::i18n::t("找不到符合的 commit") })
        } else {
            None
        };
        let hist_top = self.sizes.hist_top;
        // What is not committed yet sits above the newest commit, as in Fork; a click goes there.
        let pending = self.repo().and_then(|r| r.summary.as_ref()).map_or(0, |s| s.changes);
        let pending_row = (pending > 0 && graph.is_some()).then(|| {
            div()
                .id("uncommitted")
                .flex()
                .flex_none()
                .items_center()
                .gap(px(6.))
                .h(px(ROW))
                .pl(px(PAD - 6.))
                .pr(px(10.))
                .cursor_pointer()
                .border_b_1()
                .border_color(p.border)
                .text_color(p.muted)
                .hover(move |s| s.bg(p.hover))
                .on_click(cx.listener(|this, _, _, cx| this.set_view(View::Changes, cx)))
                .child(icon(Ico::FilePenLine, 14., p.warn))
                .child(crate::i18n::t("未提交的修改"))
                .child(div().px(px(6.)).rounded(px(8.)).bg(p.warn_bg).text_size(px(11.5)).text_color(p.warn).child(pending.to_string()))
        });
        on_split(div().id("hist").flex().flex_col().flex_1().min_h_0(), SplitKind::HistTop, cx)
            .child(cbar)
            .children(pending_row)
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
        // The stash this "commit" is, if it is one, and the repo it belongs to.
        let stash_ix = self.repo().and_then(|r| r.refs.as_ref()).and_then(|x| x.stashes.iter().find(|s| s.sha == d.sha)).map(|s| s.index as usize);
        let repo_path = self.repo().map(|r| r.path.clone()).unwrap_or_default();
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
                    .when(!d.refs.is_empty(), |c| c.child(kv("REFS", div().flex().flex_wrap().text_color(p.text).children({
                        let gh = self.on_github();
                        ref_views(&d.refs).into_iter().map(move |v| match v {
                            RefView::Branch { name, current, remotes } => ref_group(&p, name, current, remotes, gh),
                            RefView::Remote(name) => ref_pill(&p, name, "remote", gh),
                            RefView::Detached => ref_pill(&p, "", "detached", gh),
                            RefView::Tag(name) => ref_pill(&p, name, "tag", gh),
                        })
                    }).into_any_element())))
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
            .child(match stash_ix {
                // A stash: what can be done with it, instead of what can be done with a commit.
                Some(ix) => {
                    let (pa, pb) = (repo_path.clone(), repo_path.clone());
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(px(6.))
                        .child(act("s-ap", crate::i18n::t("套用")).on_click(cx.listener(move |this, _, _, cx| {
                            this.simple_op(&pa, "stash-apply", ix.to_string(), crate::i18n::t("套用 stash"), Some(crate::i18n::t("已套用 stash").into()), cx)
                        })))
                        .child(act("s-pop", "Pop").on_click(cx.listener(move |this, _, _, cx| {
                            this.simple_op(&pb, "stash-pop", ix.to_string(), "Pop stash", Some(crate::i18n::t("已套用並刪除 stash").into()), cx)
                        })))
                        .child(act("s-dr", crate::i18n::t("刪除")).on_click(cx.listener(move |this, _, _, cx| this.stash_drop(ix, cx))))
                }
                None => {
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
                        .child(act("a-rs", crate::i18n::t("Reset 到這裡…")).on_click(cx.listener(move |this, _, _, cx| this.reset_to(s6.clone(), cx))))
                }
            })
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

    /// The File Tree tab: every file of the commit, and the picked file as it was then.
    fn render_tree_tab(&mut self, d: gitcore::ops::CommitDetail, cx: &mut Context<Self>) -> AnyElement {
        let p = self.pal;
        let Some(r) = self.repo() else { return div().into_any_element() };
        let (rows, sel, shown) = (r.tree_rows.clone(), r.tree_sel.clone(), r.tree_file.clone());
        let listed = r.tree_of == d.sha;
        let open = Arc::new(r.tree_open.clone());
        let hf = self.sizes.hist_files;
        let owner = cx.entity().downgrade();
        let tree = uniform_list("ftree", if listed { rows.len() } else { 0 }, {
            let (rows, sel, open, owner) = (rows.clone(), sel.clone(), open.clone(), owner.clone());
            move |range, _, _| {
                range
                    .map(|ix| {
                        let row = &rows[ix];
                        let on = !row.dir && sel.as_deref() == Some(row.path.as_str());
                        let unfolded = row.dir && open.contains(&row.path);
                        let (o, path, is_dir) = (owner.clone(), row.path.clone(), row.dir);
                        div()
                            .id(SharedString::from(format!("ft-{}", row.path)))
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .h(px(24.))
                            .mx(px(6.))
                            .pl(px(8. + row.depth as f32 * 14.))
                            .pr(px(8.))
                            .rounded(px(5.))
                            .whitespace_nowrap()
                            .cursor_pointer()
                            .bg(if on { p.accent_bg } else { Hsla::transparent_black() })
                            .text_color(if on { p.accent_tx } else { p.text })
                            .hover(move |s| s.bg(if on { p.accent_bg } else { p.hover }))
                            .on_click(move |_, _, cx| {
                                let path = path.clone();
                                let _ = o.update(cx, |this, cx| if is_dir { this.toggle_tree_dir(path, cx) } else { this.pick_tree_file(path, cx) });
                            })
                            .child(if row.dir {
                                icon(if unfolded { Ico::ChevronDown } else { Ico::ChevronRight }, 12., p.muted).into_any_element()
                            } else {
                                div().w(px(12.)).flex_none().into_any_element()
                            })
                            .child(if row.dir { icon(if unfolded { Ico::FolderOpen } else { Ico::Folder }, 14., p.warn) } else { icon(Ico::File, 14., p.muted) })
                            .child(div().overflow_hidden().text_ellipsis().child(row.name.clone()))
                            .into_any_element()
                    })
                    .collect()
            }
        })
        .size_full();
        let right: AnyElement = match (&sel, shown) {
            (None, Some(FileShown::Note(why))) => empty(&p, "", &why, None),
            (None, _) => empty(&p, "", crate::i18n::t("選一個檔案"), None),
            (Some(file), Some(FileShown::Lines(lines))) => {
                // Room for the widest line number.
                let gutter = 22. + lines.len().max(1).to_string().len() as f32 * 7.5;
                let body = uniform_list(SharedString::from(format!("fc-{}-{file}", d.sha)), lines.len(), {
                    let lines = lines.clone();
                    move |range, _, _| {
                        range
                            .map(|ix| {
                                div()
                                    .flex()
                                    .items_center()
                                    .h(px(19.))
                                    .font_family("Cascadia Mono")
                                    .text_size(px(12.5))
                                    .whitespace_nowrap()
                                    .child(div().flex().flex_none().justify_end().w(px(gutter)).pr(px(12.)).text_color(p.faint).child((ix + 1).to_string()))
                                    .child(div().text_color(p.text).child(lines[ix].clone()))
                                    .into_any_element()
                            })
                            .collect()
                    }
                })
                .size_full();
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .min_h_0()
                    .child(
                        div()
                            .flex()
                            .flex_none()
                            .items_center()
                            .gap(px(8.))
                            .h(px(34.))
                            .px(px(12.))
                            .border_b_1()
                            .border_color(p.border)
                            .child(icon(Ico::File, 14., p.muted))
                            .child(div().overflow_hidden().text_ellipsis().whitespace_nowrap().child(file.clone()))
                            .child(sub(&p, crate::i18n::tr("{} 行", &[], &[lines.len().to_string()]))),
                    )
                    .child(div().flex_1().min_h_0().overflow_hidden().py(px(4.)).child(body))
                    .into_any_element()
            }
            (Some(_), Some(FileShown::Note(why))) => empty(&p, "", &why, None),
            (Some(_), _) => empty(&p, "", crate::i18n::t("載入中…"), None),
        };
        on_split(div().id("trtab").flex().flex_1().min_h_0(), SplitKind::HistFiles, cx)
            .child(div().id("hist-ftree").flex_none().w(px(hf)).h_full().py(px(6.)).child(tree))
            .child(splitter(&p, "split-ft", SplitKind::HistFiles, true))
            .child(right)
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
