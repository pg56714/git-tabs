//! User-facing git actions (the Rust counterpart of src/actions.ts): confirm or ask where
//! needed, then run the operation and reload.

use std::collections::HashSet;
use std::path::{Path, PathBuf};


use gitcore::config::Identity;
use gitcore::ops::{self, LineAction};
use gpui_kit::{ClipboardItem, Context, Window};
use serde_json::json;

use crate::app::{GitTabs, HistTab, gated};
use crate::format::short;
use crate::model::*;

pub fn branch_name_check(v: &str) -> Option<String> {
    let t = v.trim();
    if t.is_empty() {
        return Some(crate::i18n::t("輸入分支名稱").into());
    }
    let bad = t.chars().any(|c| c.is_whitespace() || "~^:?*[\\".contains(c)) || t.contains("..") || t.starts_with('-') || t.ends_with('/') || t.ends_with(".lock");
    bad.then(|| crate::i18n::t("分支名稱不能有空白或 ~ ^ : ? * [ \\ 這些字元").to_string())
}

fn tag_name_check(v: &str) -> Option<String> {
    (v.trim().is_empty() || v.trim().chars().any(char::is_whitespace)).then(|| crate::i18n::t("輸入 tag 名稱（不能有空白）").to_string())
}

fn clean(e: &str) -> String {
    e.trim_start_matches("Error: ").to_string()
}

impl GitTabs {
    // ----- running operations -----

    /// Runs one action on a repo: marks it busy, reports errors, then reloads. `then` sees the
    /// result once the command has finished (the reload is still running).
    #[allow(clippy::too_many_arguments)]
    pub fn op_with(
        &mut self,
        path: &Path,
        label: &'static str,
        quiet: bool,
        ok_msg: Option<String>,
        job: impl FnOnce(&Path) -> Result<String, String> + Send + 'static,
        then: impl FnOnce(&mut GitTabs, &Result<String, String>, &mut Context<GitTabs>) + 'static,
        cx: &mut Context<Self>,
    ) {
        let Some(r) = self.repo_by_path(path) else { return };
        r.busy = Some(label.to_string());
        let (gate, ticket) = self.take_ticket();
        let p = path.to_path_buf();
        let p2 = p.clone();
        self.work(cx, move || gated(&gate, ticket, || job(&p2)), move |this, res, cx| {
            if let Some(r) = this.repo_by_path(&p) {
                r.busy = None;
            }
            this.pull_log();
            match &res {
                Ok(_) if !quiet => this.ok(ok_msg.unwrap_or_else(|| crate::i18n::tr("{label}完成", &[("label", (label).to_string())], &[])), cx),
                _ => {}
            }
            then(this, &res, cx);
            if let Err(e) = res {
                let e = clean(&e);
                // A merge/rebase/cherry-pick that stops on conflicts isn't an error to dismiss: go resolve them.
                let hook_path = p.clone();
                this.refresh_then(&p, cx, move |this, cx| {
                    let p = hook_path;
                    let Some(r) = this.repo_by_path(&p) else { return };
                    let conflicts: Vec<String> = r.status.iter().filter(|f| f.conflicted).map(|f| f.path.clone()).collect();
                    if r.summary.as_ref().is_some_and(|s| s.state.is_some()) && !conflicts.is_empty() {
                        let n = conflicts.len();
                        let was = r.view;
                        this.toast(crate::i18n::tr("{label}有 {n} 個衝突要解決", &[("label", (label).to_string()), ("n", (n).to_string())], &[]), ToastKind::Info, None, cx);
                        if was != View::Changes {
                            this.set_view(View::Changes, cx);
                        }
                        this.select(ListKind::U, conflicts[0].clone(), cx);
                    } else {
                        this.toast(crate::i18n::tr("{label}失敗", &[("label", (label).to_string())], &[]), ToastKind::Err, Some(e), cx);
                    }
                });
            } else {
                this.refresh(&p, cx);
            }
            cx.notify();
        });
        cx.notify();
    }

    pub fn op(
        &mut self,
        path: &Path,
        label: &'static str,
        ok_msg: Option<String>,
        job: impl FnOnce(&Path) -> Result<String, String> + Send + 'static,
        cx: &mut Context<Self>,
    ) {
        self.op_with(path, label, false, ok_msg, job, |_, _, _| {}, cx);
    }

    pub fn simple_op(&mut self, path: &Path, op: &'static str, arg: String, label: &'static str, ok: Option<String>, cx: &mut Context<Self>) {
        self.op(path, label, ok, move |repo| ops::simple(repo, op, &arg), cx);
    }

    fn dispatch_bg(&mut self, cmd: &'static str, args: serde_json::Value, fail: &'static str, cx: &mut Context<Self>) {
        let ctx = self.ctx.clone();
        self.work(cx, move || gitcore::dispatch(&ctx, cmd, args), move |this, res, cx| {
            if let Err(e) = res {
                this.toast(fail, ToastKind::Err, Some(clean(&e)), cx);
            }
        });
    }

    // ----- stage / unstage / discard -----

    /// When the selected file leaves a list, select the next file in that list (or follow it).
    fn follow_selection(r: &mut RepoState, moved: &HashSet<String>, from: ListKind, to: ListKind) {
        let Some(sel) = r.sel.clone() else { return };
        if sel.list != from || !moved.contains(&sel.path) {
            return;
        }
        let list: Vec<Sel> = Self::ordered_files(r).into_iter().filter(|f| f.list == from).collect();
        let i = list.iter().position(|f| f.path == sel.path).unwrap_or(0);
        let next = list.iter().skip(i + 1).find(|f| !moved.contains(&f.path)).or_else(|| list[..i].iter().rev().find(|f| !moved.contains(&f.path))).cloned();
        r.sel = Some(match next {
            Some(n) => n,
            None => Sel { list: to, path: sel.path },
        });
    }

    pub fn stage_files(&mut self, paths: Vec<String>, cx: &mut Context<Self>) {
        let Some(path) = self.active_path() else { return };
        if paths.is_empty() {
            return;
        }
        let set: HashSet<String> = paths.iter().cloned().collect();
        self.quick(
            &path,
            "Stage",
            |r| {
                Self::follow_selection(r, &set, ListKind::U, ListKind::S);
                for f in r.status.iter_mut() {
                    if !set.contains(&f.path) || f.conflicted {
                        continue;
                    }
                    f.x = if f.untracked {
                        'A'
                    } else if f.y != '.' {
                        f.y
                    } else {
                        f.x
                    };
                    f.y = '.';
                    f.untracked = false;
                }
            },
            move |repo| ops::stage(repo, &paths).map(|_| String::new()),
            cx,
        );
        self.after_stage(cx);
    }

    pub fn unstage_files(&mut self, paths: Vec<String>, cx: &mut Context<Self>) {
        let Some(path) = self.active_path() else { return };
        if paths.is_empty() {
            return;
        }
        let set: HashSet<String> = paths.iter().cloned().collect();
        self.quick(
            &path,
            crate::i18n::t("取消 Stage"),
            |r| {
                Self::follow_selection(r, &set, ListKind::S, ListKind::U);
                for f in r.status.iter_mut() {
                    if !set.contains(&f.path) {
                        continue;
                    }
                    if f.x == 'A' {
                        f.untracked = true;
                        f.y = '?';
                    } else if f.y == '.' {
                        f.y = f.x;
                    }
                    f.x = '.';
                }
            },
            move |repo| ops::unstage(repo, &paths).map(|_| String::new()),
            cx,
        );
        self.after_stage(cx);
    }

    /// The diff of the file that was selected before the move is stale.
    fn after_stage(&mut self, cx: &mut Context<Self>) {
        self.load_diff(cx);
        cx.notify();
    }

    pub fn stage_all(&mut self, cx: &mut Context<Self>) {
        let Some(r) = self.repo() else { return };
        let paths: Vec<String> = r.unstaged().iter().filter(|f| !f.conflicted).map(|f| f.path.clone()).collect();
        self.stage_files(paths, cx);
    }

    pub fn unstage_all(&mut self, cx: &mut Context<Self>) {
        let Some(r) = self.repo() else { return };
        let paths: Vec<String> = r.staged().iter().map(|f| f.path.clone()).collect();
        self.unstage_files(paths, cx);
    }

    /// "S": stages the selected lines if there are any, else the whole file.
    pub fn stage_toggle(&mut self, cx: &mut Context<Self>) {
        if self.stored_sel.is_some() && self.apply_selection(false, cx) {
            return;
        }
        let Some(sel) = self.repo().and_then(|r| r.sel.clone()) else { return };
        match sel.list {
            ListKind::U => self.stage_files(vec![sel.path], cx),
            ListKind::S => self.unstage_files(vec![sel.path], cx),
            _ => {}
        }
    }

    pub fn discard_files(&mut self, paths: Vec<String>, cx: &mut Context<Self>) {
        let Some(r) = self.repo() else { return };
        if paths.is_empty() {
            return;
        }
        let repo = r.path.clone();
        let untracked = paths.iter().filter(|p| r.status.iter().any(|s| &s.path == *p && s.untracked)).count();
        let what = if paths.len() == 1 { crate::i18n::tr("`{}` 的修改", &[], &[(paths[0]).to_string()]) } else { crate::i18n::tr("{} 個檔案的修改", &[], &[(paths.len()).to_string()]) };
        let extra = if untracked > 0 { crate::i18n::tr("\n其中 {untracked} 個是新檔案，會直接**刪除**。", &[("untracked", (untracked).to_string())], &[]) } else { String::new() };
        self.confirm(crate::i18n::t("捨棄修改？"), crate::i18n::tr("{what}會被丟掉，**無法復原**。{extra}", &[("what", (what).to_string()), ("extra", (extra).to_string())], &[]), crate::i18n::t("捨棄"), true, cx, move |this, _, cx| {
            let paths = paths.clone();
            this.op(&repo, crate::i18n::t("捨棄"), Some(crate::i18n::t("已捨棄").into()), move |r| ops::discard(r, &paths).map(|_| String::new()), cx);
        });
    }

    /// Applies the lines picked in the diff view. `discard` asks first.
    pub fn apply_selection(&mut self, discard: bool, cx: &mut Context<Self>) -> bool {
        let Some(r) = self.repo() else { return false };
        let (Some(sel), Some(fd)) = (r.sel.clone(), r.diff.clone()) else { return false };
        let Some(range) = self.stored_sel.clone() else { return false };
        let pairs = crate::diffmap::selected_pairs(&fd, (range.side(), range.start()), (range.end_side(), range.end()));
        if pairs.is_empty() {
            self.toast(crate::i18n::t("選取的範圍沒有異動的行"), ToastKind::Info, None, cx);
            return false;
        }
        let action = if discard {
            LineAction::Discard
        } else if sel.list == ListKind::S {
            LineAction::Unstage
        } else {
            LineAction::Stage
        };
        self.apply_lines(sel.path, action, pairs, cx);
        true
    }

    pub fn apply_hunk(&mut self, hunk: usize, action: LineAction, cx: &mut Context<Self>) {
        let Some(r) = self.repo() else { return };
        let (Some(sel), Some(fd)) = (r.sel.clone(), r.diff.as_ref()) else { return };
        let pairs = crate::diffmap::hunk_pairs(fd, hunk);
        self.apply_lines(sel.path, action, pairs, cx);
    }

    pub fn apply_lines(&mut self, file: String, action: LineAction, pairs: Vec<(usize, usize)>, cx: &mut Context<Self>) {
        let Some(path) = self.active_path() else { return };
        if pairs.is_empty() {
            return;
        }
        let n = pairs.len();
        let run = move |this: &mut GitTabs, cx: &mut Context<GitTabs>| {
            let label = match action {
                LineAction::Stage => crate::i18n::t("Stage 選取的行"),
                LineAction::Unstage => crate::i18n::t("取消 Stage 選取的行"),
                LineAction::Discard => crate::i18n::t("捨棄選取的行"),
            };
            let (p, f, pr) = (path.clone(), file.clone(), pairs.clone());
            this.quick(&p, label, |_| {}, move |repo| ops::apply_lines(repo, &f, action, &pr).map(|_| String::new()), cx);
            this.stored_sel = None;
            this.diff.update(cx, |d, cx| d.set_selected_lines(None, cx));
        };
        if action == LineAction::Discard {
            let file = self.repo().and_then(|r| r.sel.as_ref().map(|s| s.path.clone())).unwrap_or_default();
            self.confirm(crate::i18n::t("捨棄選取的行？"), crate::i18n::tr("`{file}` 選取的 {n} 行修改會被丟掉，**無法復原**。", &[("file", (file).to_string()), ("n", (n).to_string())], &[]), crate::i18n::t("捨棄"), true, cx, move |this, _, cx| run(this, cx));
        } else {
            run(self, cx);
        }
    }

    // ----- commit -----

    pub fn do_commit(&mut self, push: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.commit_err.clear();
        let Some(r) = self.repo() else { return };
        let conflicts = r.unstaged().iter().filter(|f| f.conflicted).count();
        let staged = r.staged().len();
        let subject = self.subject.read(cx).value().trim().to_string();
        let desc = self.desc.read(cx).value().trim().to_string();
        let err = if conflicts > 0 {
            crate::i18n::t("先解決衝突的檔案")
        } else if staged == 0 && !self.amend {
            crate::i18n::t("先 Stage 要提交的檔案")
        } else if subject.is_empty() {
            crate::i18n::t("輸入 commit 標題")
        } else {
            ""
        };
        if !err.is_empty() {
            self.commit_err = err.into();
            cx.notify();
            return;
        }
        let msg = if desc.is_empty() { subject } else { format!("{subject}\n\n{desc}") };
        let amend = self.amend;
        let repo = r.path.clone();
        let who = self.identity_of(r);
        let handle = window.window_handle();
        let go = move |this: &mut GitTabs, cx: &mut Context<GitTabs>| {
            let (repo, msg) = (repo.clone(), msg.clone());
            this.committing = true;
            let (m2, subj) = (msg.clone(), msg.lines().next().unwrap_or("").to_string());
            this.op_with(
                &repo.clone(),
                if amend { "Amend" } else { "Commit" },
                true,
                None,
                move |r| ops::commit(r, &m2, amend),
                move |this, res, cx| {
                    this.committing = false;
                    if res.is_ok() {
                        this.ok(crate::i18n::tr("{}：{subj}", &[("subj", (subj).to_string())], &[(if amend { crate::i18n::t("已 amend") } else { crate::i18n::t("已 commit") }).to_string()]), cx);
                        let (subject, desc) = (this.subject.clone(), this.desc.clone());
                        let _ = handle.update(cx, |_, window, cx| {
                            subject.update(cx, |s, cx| s.set_value("", window, cx));
                            desc.update(cx, |s, cx| s.set_value("", window, cx));
                        });
                        this.amend = false;
                        if push {
                            this.push_repo(&repo, false, cx);
                        }
                    }
                },
                cx,
            );
        };
        if !self.cfg.identities.is_empty() && who.id.is_none() {
            let r = self.repo().unwrap();
            let def = self.cfg.identities.iter().find(|i| Some(&i.id) == self.cfg.default_identity.as_ref()).map(|i| i.name.clone());
            let body = crate::i18n::tr("**{}**（{}）沒有符合任何身分的規則，{}提交。\n\n可以點工具列的 👤 按鈕把它指定給某個身分。", &[], &[(r.name).to_string(), (r.summary.as_ref().and_then(|s| s.remote_url.clone()).unwrap_or_else(|| crate::i18n::t("沒有 remote").into())).to_string(), (def.map(|d| crate::i18n::tr("會用預設的「{d}」", &[("d", (d).to_string())], &[])).unwrap_or_else(|| crate::i18n::t("會用 git 的全域設定").into())).to_string()]);
            let go = std::rc::Rc::new(go);
            self.confirm(crate::i18n::t("這個 repo 沒有對應的身分"), body, crate::i18n::t("照樣提交"), false, cx, move |this, _, cx| go(this, cx));
        } else {
            go(self, cx);
        }
    }

    pub fn toggle_amend(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.amend = !self.amend;
        if self.amend && self.subject.read(cx).value().is_empty() && self.desc.read(cx).value().is_empty() {
            let Some(path) = self.active_path() else { return };
            let handle = window.window_handle();
            self.work(cx, move || ops::last_message(&path).unwrap_or_default(), move |this, m, cx| {
                let mut it = m.splitn(2, '\n');
                let first = it.next().unwrap_or("").to_string();
                let rest = it.next().unwrap_or("").trim().to_string();
                let (s, d) = (this.subject.clone(), this.desc.clone());
                let _ = handle.update(cx, |_, window, cx| {
                    s.update(cx, |s, cx| s.set_value(first.clone(), window, cx));
                    d.update(cx, |s, cx| s.set_value(rest.clone(), window, cx));
                });
            });
        }
        cx.notify();
    }

    // ----- remotes -----

    pub fn fetch_repo(&mut self, path: &Path, cx: &mut Context<Self>) {
        let name = self.repos.iter().find(|r| r.path == path).map(|r| r.name.clone()).unwrap_or_default();
        self.op(path, "Fetch", Some(crate::i18n::tr("已 fetch {name}", &[("name", (name).to_string())], &[])), |r| ops::fetch(r), cx);
    }

    pub fn pull_repo(&mut self, path: &Path, mode: &'static str, cx: &mut Context<Self>) {
        let name = self.repos.iter().find(|r| r.path == path).map(|r| r.name.clone()).unwrap_or_default();
        let p = path.to_path_buf();
        let label = "Pull";
        self.op_with(
            path,
            label,
            false,
            Some(crate::i18n::tr("已 pull {name}", &[("name", (name).to_string())], &[])),
            move |r| ops::pull(r, mode),
            move |this, res, cx| {
                // The op already toasted a failure; a diverged branch gets the better question.
                if let Err(e) = res
                    && mode == "ff"
                    && ["fast-forward", "diverg", "Not possible"].iter().any(|k| e.to_lowercase().contains(&k.to_lowercase()))
                {
                    this.toasts.pop();
                    let p = p.clone();
                    this.choose(
                        crate::i18n::t("本地和遠端各有新的 commit"),
                        crate::i18n::t("沒辦法直接快轉。要用哪種方式合併遠端的修改？"),
                        vec![
                            Choice { value: "rebase", label: crate::i18n::t("Rebase（把我的 commit 接在遠端後面）").into(), primary: true, danger: false },
                            Choice { value: "merge", label: crate::i18n::t("Merge（多一個 merge commit）").into(), primary: false, danger: false },
                        ],
                        cx,
                        move |this, pick, _, cx| {
                            let m: &'static str = if pick == "rebase" { "rebase" } else { "merge" };
                            this.pull_repo(&p, m, cx);
                        },
                    );
                }
            },
            cx,
        );
    }

    pub fn push_repo(&mut self, path: &Path, force: bool, cx: &mut Context<Self>) {
        let (name, branch) = self.repos.iter().find(|r| r.path == path).map(|r| (r.name.clone(), r.summary.as_ref().and_then(|s| s.branch.clone()).unwrap_or_default())).unwrap_or_default();
        let p = path.to_path_buf();
        let run = move |this: &mut GitTabs, cx: &mut Context<GitTabs>| {
            this.op(&p, "Push", Some(crate::i18n::tr("已 push {name}", &[("name", (name).to_string())], &[])), move |r| ops::push(r, force), cx);
        };
        if force {
            self.confirm(
                crate::i18n::t("強制 Push？"),
                crate::i18n::tr("會用 `--force-with-lease` 覆蓋遠端的 **{branch}**。如果遠端有你還沒拿到的 commit，git 會拒絕，不會蓋掉別人的東西。", &[("branch", (branch).to_string())], &[]),
                crate::i18n::t("強制 Push"),
                true,
                cx,
                move |this, _, cx| run(this, cx),
            );
        } else {
            run(self, cx);
        }
    }

    pub fn sync_repo(&mut self, path: &Path, cx: &mut Context<Self>) {
        let name = self.repos.iter().find(|r| r.path == path).map(|r| r.name.clone()).unwrap_or_default();
        self.op_with(
            path,
            "Sync",
            true,
            None,
            |r| {
                ops::fetch(r)?;
                let s = ops::summary(r)?;
                if s.behind > 0 {
                    ops::pull(r, "ff")?;
                }
                if s.ahead > 0 || s.upstream.is_none() {
                    ops::push(r, false)?;
                }
                Ok(format!("{}\u{1f}{}", s.behind, s.ahead))
            },
            move |this, res, cx| match res {
                Ok(v) => {
                    let mut it = v.split('\u{1f}');
                    let (b, a): (u32, u32) = (it.next().and_then(|x| x.parse().ok()).unwrap_or(0), it.next().and_then(|x| x.parse().ok()).unwrap_or(0));
                    this.ok(crate::i18n::tr("{name} 已同步{}{}", &[("name", (name).to_string())], &[(if b > 0 { crate::i18n::tr("，拉下 {b}", &[("b", (b).to_string())], &[]) } else { String::new() }).to_string(), (if a > 0 { crate::i18n::tr("，推上 {a}", &[("a", (a).to_string())], &[]) } else { String::new() }).to_string()]), cx);
                }
                Err(_) => {
                    this.toasts.pop();
                }
            },
            cx,
        );
    }

    pub fn fetch_all(&mut self, quiet: bool, cx: &mut Context<Self>) {
        let paths: Vec<PathBuf> = self.repos.iter().map(|r| r.path.clone()).collect();
        let n = paths.len();
        let p2 = paths.clone();
        self.work(cx, move || p2.iter().for_each(|p| drop(ops::fetch(p))), move |this, _, cx| {
            this.pull_log();
            let active = this.active_path();
            for p in paths {
                if Some(&p) == active.as_ref() {
                    this.refresh(&p, cx);
                } else {
                    this.refresh_summary(&p, cx);
                }
            }
            if !quiet {
                this.ok(crate::i18n::tr("已 fetch {n} 個 repo", &[("n", (n).to_string())], &[]), cx);
            }
        });
    }

    // ----- branches, tags, stash -----

    pub fn switch_branch(&mut self, path: &Path, name: &str, cx: &mut Context<Self>) {
        let n = name.to_string();
        let ok = crate::i18n::tr("已切換到 {name}", &[("name", (name).to_string())], &[]);
        self.op(path, crate::i18n::t("切換分支"), Some(ok), move |r| ops::switch(r, &n), cx);
    }

    pub fn new_branch(&mut self, from: Option<String>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(r) = self.repo() else { return };
        let path = r.path.clone();
        let from = from.or_else(|| r.summary.as_ref().and_then(|s| s.branch.clone())).unwrap_or_else(|| "HEAD".into());
        self.prompt(
            crate::i18n::t("新增分支"),
            vec![FieldSpec::text("name", crate::i18n::t("名稱")).placeholder("feat/my-change").check(branch_name_check), FieldSpec::text("from", crate::i18n::t("從")).value(from), FieldSpec::checkbox("checkout", crate::i18n::t("建好就切換過去"), true)],
            crate::i18n::t("建立"),
            window,
            cx,
            move |this, v, _, cx| {
                let name = v["name"].trim().to_string();
                let from = v["from"].trim().to_string();
                let checkout = v.get("checkout").is_some_and(|x| x == "1");
                let ok = crate::i18n::tr("已建立 {name}", &[("name", (name).to_string())], &[]);
                this.op(&path, crate::i18n::t("建立分支"), Some(ok), move |r| ops::create_branch(r, &name, Some(&from).filter(|f| !f.is_empty()).map(|f| f.as_str()), checkout), cx);
            },
        );
    }

    pub fn delete_branch(&mut self, name: String, cx: &mut Context<Self>) {
        let Some(r) = self.repo() else { return };
        let path = r.path.clone();
        let upstream = r.refs.as_ref().and_then(|x| x.refs.branches.iter().find(|b| b.name == name)).and_then(|b| b.upstream.clone());
        let body = crate::i18n::tr("刪除本地分支 **{name}**{}。", &[("name", (name).to_string())], &[(upstream.map(|u| crate::i18n::tr("（遠端的 {u} 不會被刪）", &[("u", (u).to_string())], &[])).unwrap_or_default()).to_string()]);
        self.confirm(crate::i18n::t("刪除分支？"), body, crate::i18n::t("刪除"), true, cx, move |this, _, cx| {
            let (n1, p1, n2) = (name.clone(), path.clone(), name.clone());
            let p_op = p1.clone();
            this.op_with(
                &p_op,
                crate::i18n::t("刪除分支"),
                true,
                None,
                move |r| ops::delete_branch(r, &n1, false),
                move |this, res, cx| {
                    if res.is_ok() {
                        this.ok(crate::i18n::tr("已刪除 {n2}", &[("n2", (n2).to_string())], &[]), cx);
                        return;
                    }
                    this.toasts.pop();
                    let (n3, p3) = (n2.clone(), p1.clone());
                    this.confirm(crate::i18n::t("分支還沒合併"), crate::i18n::tr("**{n2}** 有還沒合併的 commit，強制刪除後只能從 Reflog 找回。", &[("n2", (n2).to_string())], &[]), crate::i18n::t("強制刪除"), true, cx, move |this, _, cx| {
                        let n4 = n3.clone();
                        this.op(&p3, crate::i18n::t("刪除分支"), Some(crate::i18n::tr("已刪除 {n3}", &[("n3", (n3).to_string())], &[])), move |r| ops::delete_branch(r, &n4, true), cx);
                    });
                },
                cx,
            );
        });
    }

    pub fn merge_into(&mut self, name: String, cx: &mut Context<Self>) {
        let Some(r) = self.repo() else { return };
        let (path, cur) = (r.path.clone(), r.summary.as_ref().and_then(|s| s.branch.clone()).unwrap_or_else(|| "HEAD".into()));
        self.confirm("Merge", crate::i18n::tr("把 **{name}** merge 進目前的 **{cur}**？", &[("name", (name).to_string()), ("cur", (cur).to_string())], &[]), "Merge", false, cx, move |this, _, cx| {
            this.simple_op(&path, "merge", name.clone(), "Merge", Some(crate::i18n::tr("已把 {name} merge 進 {cur}", &[("name", (name).to_string()), ("cur", (cur).to_string())], &[])), cx);
        });
    }

    pub fn rebase_onto(&mut self, name: String, cx: &mut Context<Self>) {
        let Some(r) = self.repo() else { return };
        let (path, cur) = (r.path.clone(), r.summary.as_ref().and_then(|s| s.branch.clone()).unwrap_or_else(|| "HEAD".into()));
        self.confirm("Rebase", crate::i18n::tr("把目前的 **{cur}** rebase 到 **{name}** 上？已經 push 過的 commit 會被改寫。", &[("cur", (cur).to_string()), ("name", (name).to_string())], &[]), "Rebase", false, cx, move |this, _, cx| {
            this.simple_op(&path, "rebase", name.clone(), "Rebase", None, cx);
        });
    }

    pub fn cherry_pick(&mut self, sha: String, cx: &mut Context<Self>) {
        let Some(path) = self.active_path() else { return };
        let ok = crate::i18n::tr("已 cherry-pick {}", &[], &[(short(&sha)).to_string()]);
        self.simple_op(&path, "cherry-pick", sha, "Cherry-pick", Some(ok), cx);
    }

    pub fn revert(&mut self, sha: String, cx: &mut Context<Self>) {
        let Some(path) = self.active_path() else { return };
        self.confirm("Revert", crate::i18n::tr("新增一個 commit 來抵銷 `{}` 的修改？", &[], &[(short(&sha)).to_string()]), "Revert", false, cx, move |this, _, cx| {
            let ok = crate::i18n::tr("已 revert {}", &[], &[(short(&sha)).to_string()]);
            this.simple_op(&path, "revert", sha.clone(), "Revert", Some(ok), cx);
        });
    }

    pub fn reset_to(&mut self, sha: String, cx: &mut Context<Self>) {
        let Some(r) = self.repo() else { return };
        let (path, branch) = (r.path.clone(), r.summary.as_ref().and_then(|s| s.branch.clone()).unwrap_or_else(|| "HEAD".into()));
        self.choose(
            &crate::i18n::tr("Reset {branch} 到 {}", &[("branch", (branch).to_string())], &[(short(&sha)).to_string()]),
            crate::i18n::t("選擇怎麼處理目前的修改："),
            vec![
                Choice { value: "soft", label: crate::i18n::t("soft：保留修改，而且保持已 stage").into(), primary: false, danger: false },
                Choice { value: "mixed", label: crate::i18n::t("mixed：保留修改，取消 stage").into(), primary: true, danger: false },
                Choice { value: "hard", label: crate::i18n::t("hard：丟掉所有修改（無法復原）").into(), primary: false, danger: true },
            ],
            cx,
            move |this, mode, _, cx| {
                let run = {
                    let (path, sha, mode) = (path.clone(), sha.clone(), mode.clone());
                    move |this: &mut GitTabs, cx: &mut Context<GitTabs>| {
                        let op: &'static str = match mode.as_str() {
                            "soft" => "reset-soft",
                            "hard" => "reset-hard",
                            _ => "reset-mixed",
                        };
                        let ok = crate::i18n::tr("已 reset --{mode} 到 {}", &[("mode", (mode).to_string())], &[(short(&sha)).to_string()]);
                        this.simple_op(&path, op, sha.clone(), "Reset", Some(ok), cx);
                    }
                };
                if mode == "hard" {
                    this.confirm(crate::i18n::t("確定 hard reset？"), crate::i18n::t("所有還沒 commit 的修改都會消失，**無法復原**。").into(), "Hard reset", true, cx, move |this, _, cx| run(this, cx));
                } else {
                    run(this, cx);
                }
            },
        );
    }

    pub fn checkout_commit(&mut self, sha: String, cx: &mut Context<Self>) {
        let Some(path) = self.active_path() else { return };
        self.confirm(
            crate::i18n::t("Checkout 這個 commit？"),
            crate::i18n::tr("會進入 detached HEAD 狀態，停在 `{}`。之後要繼續工作，請切回分支或從這裡開新分支。", &[], &[(short(&sha)).to_string()]),
            "Checkout",
            false,
            cx,
            move |this, _, cx| this.simple_op(&path, "checkout", sha.clone(), "Checkout", None, cx),
        );
    }

    pub fn create_tag(&mut self, sha: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(path) = self.active_path() else { return };
        self.prompt(
            &crate::i18n::tr("在 {} 加 Tag", &[], &[(short(&sha)).to_string()]),
            vec![FieldSpec::text("name", crate::i18n::t("名稱")).placeholder("v1.0.0").check(tag_name_check), FieldSpec::textarea("message", crate::i18n::t("說明（可留空）"))],
            crate::i18n::t("確定"),
            window,
            cx,
            move |this, v, _, cx| {
                let (name, msg, sha) = (v["name"].trim().to_string(), v["message"].clone(), sha.clone());
                let ok = crate::i18n::tr("已加上 {name}", &[("name", (name).to_string())], &[]);
                this.op(&path, crate::i18n::t("加 Tag"), Some(ok), move |r| ops::tag(r, &name, &sha, &msg), cx);
            },
        );
    }

    pub fn stash_push(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(r) = self.repo() else { return };
        if r.status.is_empty() {
            self.toast(crate::i18n::t("沒有修改可以 stash"), ToastKind::Info, None, cx);
            return;
        }
        let path = r.path.clone();
        self.prompt(
            crate::i18n::t("Stash 目前的修改"),
            vec![FieldSpec::text("message", crate::i18n::t("說明（可留空）")).placeholder(crate::i18n::t("做到一半的 xxx")), FieldSpec::checkbox("untracked", crate::i18n::t("包含新檔案"), true)],
            "Stash",
            window,
            cx,
            move |this, v, _, cx| {
                let (msg, untracked) = (v["message"].clone(), v.get("untracked").is_some_and(|x| x == "1"));
                this.op(&path, "Stash", Some(crate::i18n::t("已 stash").into()), move |r| ops::stash_push(r, &msg, untracked), cx);
            },
        );
    }

    pub fn stash_drop(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(path) = self.active_path() else { return };
        self.confirm(crate::i18n::t("刪除 stash？"), crate::i18n::t("刪除後無法復原。").into(), crate::i18n::t("刪除"), true, cx, move |this, _, cx| {
            this.simple_op(&path, "stash-drop", index.to_string(), crate::i18n::t("刪除 stash"), Some(crate::i18n::t("已刪除 stash").into()), cx);
        });
    }

    pub fn abort_or_continue(&mut self, what: &'static str, cx: &mut Context<Self>) {
        let Some(r) = self.repo() else { return };
        let (path, state) = (r.path.clone(), r.summary.as_ref().and_then(|s| s.state).unwrap_or("").to_string());
        if what == "abort" {
            self.confirm(&crate::i18n::tr("中止 {state}？", &[("state", (state).to_string())], &[]), crate::i18n::t("會回到開始之前的狀態，解到一半的衝突會被丟掉。").into(), crate::i18n::t("中止"), true, cx, move |this, _, cx| {
                this.simple_op(&path, "abort", String::new(), crate::i18n::t("中止"), Some(crate::i18n::tr("已中止 {state}", &[("state", (state).to_string())], &[])), cx);
            });
        } else {
            self.simple_op(&path, "continue", String::new(), crate::i18n::t("繼續"), Some(crate::i18n::tr("已完成 {state}", &[("state", (state).to_string())], &[])), cx);
        }
    }

    pub fn resolve_conflict(&mut self, file: String, how: &'static str, cx: &mut Context<Self>) {
        let Some(path) = self.active_path() else { return };
        let (label, name) = (
            match how {
                "use-ours" => crate::i18n::t("用我的版本"),
                "use-theirs" => crate::i18n::t("用對方的版本"),
                _ => crate::i18n::t("標記已解決"),
            },
            file.rsplit('/').next().unwrap_or(&file).to_string(),
        );
        let p = path.clone();
        self.op_with(&path, label, false, Some(crate::i18n::tr("已解決 {name}", &[("name", (name).to_string())], &[])), move |r| ops::simple(r, how, &file), move |_, _, _| {}, cx);
        self.refresh_then(&p, cx, |this, cx| {
            let next = this.repo().and_then(|r| r.unstaged().iter().find(|f| f.conflicted).map(|f| f.path.clone()));
            if let Some(n) = next {
                this.select(ListKind::U, n, cx);
            }
        });
    }

    pub fn set_pin(&mut self, id: Option<String>, cx: &mut Context<Self>) {
        let Some(r) = self.repo() else { return };
        let (path, name) = (r.path.clone(), r.name.clone());
        let ident: Option<Identity> = id.as_ref().and_then(|i| self.cfg.identities.iter().find(|x| &x.id == i).cloned());
        let label = crate::i18n::t("改用身分");
        let ctx = self.ctx.clone();
        let v = json!({ "repo": path.display().to_string(), "identity": ident });
        let msg = match &ident {
            Some(i) => crate::i18n::tr("{name} 改用 {}（{}）", &[("name", (name).to_string())], &[(i.name).to_string(), (if i.account.is_empty() { &i.email } else { &i.account }).to_string()]),
            None => crate::i18n::tr("{name} 改回自動", &[("name", (name).to_string())], &[]),
        };
        self.op_with(&path, label, true, None, move |_| gitcore::dispatch(&ctx, "identity_pin", v).map(|_| String::new()), move |this, res, cx| {
            if res.is_ok() {
                let p = this.repo().map(|r| r.path.display().to_string()).unwrap_or_default();
                if let Some(e) = this.cfg.repos.iter_mut().find(|x| x.path == p) {
                    e.pin = id.clone();
                }
                this.save_config();
                this.ok(msg, cx);
            }
        }, cx);
    }

    // ----- opening things -----

    pub fn open_folder(&mut self, path: String, cx: &mut Context<Self>) {
        self.dispatch_bg("open_folder", json!({ "path": path }), crate::i18n::t("打不開資料夾"), cx);
    }

    pub fn open_terminal(&mut self, path: String, cx: &mut Context<Self>) {
        self.dispatch_bg("open_terminal", json!({ "path": path }), crate::i18n::t("打不開終端機"), cx);
    }

    pub fn open_file(&mut self, rel: &str, cx: &mut Context<Self>) {
        let Some(r) = self.repo() else { return };
        let full = format!("{}/{}", r.path.display(), rel).replace('/', "\\");
        self.dispatch_bg("open_file", json!({ "path": full }), crate::i18n::t("打不開檔案"), cx);
    }

    pub fn copy(&mut self, text: String, what: &str, cx: &mut Context<Self>) {
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        self.ok(crate::i18n::tr("已複製 {what}", &[("what", (what).to_string())], &[]), cx);
    }

    pub fn open_url(&mut self, url: &str, cx: &mut Context<Self>) {
        cx.open_url(url);
    }

    // ----- history views -----

    pub fn file_history(&mut self, path: String, cx: &mut Context<Self>) {
        if let Some(r) = self.repo_mut() {
            r.log_path = Some(path);
            r.log_query.clear();
            r.commits = Default::default();
            r.detail = None;
        }
        self.set_view(View::History, cx);
    }

    pub fn clear_file_history(&mut self, cx: &mut Context<Self>) {
        let Some(r) = self.repo_mut() else { return };
        r.log_path = None;
        r.commits = Default::default();
        r.detail = None;
        let p = r.path.clone();
        self.load_log(&p, true, cx);
    }

    pub fn show_commit(&mut self, sha: String, cx: &mut Context<Self>) {
        let was = self.repo().map(|r| r.view);
        if was != Some(View::History) {
            self.set_view(View::History, cx);
        }
        let Some(r) = self.repo_mut() else { return };
        if !r.commits.iter().any(|c| c.sha == sha) {
            self.toast(crate::i18n::t("這個 commit 不在目前載入的歷史裡"), ToastKind::Info, None, cx);
            return;
        }
        r.sel_commit = Some(sha);
        let p = r.path.clone();
        self.load_detail(&p, cx);
        cx.notify();
    }

    pub fn open_worktree(&mut self, path: String, cx: &mut Context<Self>) {
        self.add_repos(vec![PathBuf::from(path)], None, cx);
    }

    pub fn remove_worktree(&mut self, wt: String, cx: &mut Context<Self>) {
        let Some(path) = self.active_path() else { return };
        self.confirm(crate::i18n::t("移除 worktree？"), crate::i18n::tr("移除 `{wt}`。分支本身會保留。有未提交的修改時 git 會拒絕。", &[("wt", (wt).to_string())], &[]), crate::i18n::t("移除"), true, cx, move |this, _, cx| {
            this.simple_op(&path, "worktree-remove", wt.clone(), crate::i18n::t("移除 worktree"), None, cx);
        });
    }

    pub fn set_hist_tab(&mut self, t: HistTab, cx: &mut Context<Self>) {
        self.hist_tab = t;
        self.save_prefs();
        cx.notify();
    }

    // ----- repos -----

    pub fn add_repos(&mut self, paths: Vec<PathBuf>, pin: Option<String>, cx: &mut Context<Self>) {
        let ctx = self.ctx.clone();
        self.work(cx, move || paths.iter().map(|p| ops::toplevel(p)).collect::<Vec<_>>(), move |this, tops, cx| {
            let mut added = 0usize;
            let mut last = None;
            for t in tops {
                match t {
                    Ok(top) => {
                        let same = |a: &Path, b: &Path| a.display().to_string().replace('/', "\\").to_lowercase() == b.display().to_string().replace('/', "\\").to_lowercase();
                        if this.repos.iter().any(|r| same(&r.path, &top)) {
                            continue;
                        }
                        this.repos.push(RepoState::new(top.clone()));
                        this.cfg.repos.push(gitcore::config::RepoEntry { path: top.display().to_string(), pin: pin.clone() });
                        if let Some(ident) = pin.as_ref().and_then(|p| this.cfg.identities.iter().find(|i| &i.id == p)).cloned() {
                            let (c, v) = (ctx.clone(), json!({ "repo": top.display().to_string(), "identity": ident }));
                            this.work(cx, move || drop(gitcore::dispatch(&c, "identity_pin", v)), |_, _, _| {});
                        }
                        last = Some(top);
                        added += 1;
                    }
                    Err(e) => this.toast(crate::i18n::t("不是 git repo"), ToastKind::Err, Some(clean(&e)), cx),
                }
            }
            if added > 0 {
                this.active = this.repos.len() - 1;
                this.save_config();
                this.watch(cx);
                if let Some(p) = last {
                    this.set_view(View::History, cx);
                    this.open_repo(&p, cx);
                }
                this.ok(crate::i18n::tr("已加入 {added} 個 repo", &[("added", (added).to_string())], &[]), cx);
            }
            cx.notify();
        });
    }

    pub fn close_repo(&mut self, i: usize, cx: &mut Context<Self>) {
        if i >= self.repos.len() {
            return;
        }
        let r = self.repos.remove(i);
        self.cfg.repos.retain(|x| Path::new(&x.path) != r.path);
        if self.active >= self.repos.len() {
            self.active = self.repos.len().saturating_sub(1);
        } else if i < self.active {
            self.active -= 1;
        }
        self.save_config();
        self.watch(cx);
        self.load_diff(cx);
        if let Some(p) = self.active_path() {
            self.open_repo(&p, cx);
        }
        cx.notify();
    }

    pub fn move_tab(&mut self, from: usize, to: usize, cx: &mut Context<Self>) {
        if from >= self.repos.len() || to >= self.repos.len() || from == to {
            return;
        }
        let cur = self.repos[self.active].path.clone();
        let r = self.repos.remove(from);
        self.repos.insert(to, r);
        self.active = self.repos.iter().position(|r| r.path == cur).unwrap_or(0);
        self.save_config();
        cx.notify();
    }
}
