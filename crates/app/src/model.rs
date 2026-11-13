//! State shapes: per-repo state, overlays (menu, modals, toasts) and their callbacks.

use std::collections::HashSet;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;

use gitcore::config::Identity;
use gitcore::ops::{BlameLine, CommitDetail, RefsAll, Summary};
use gitcore::parse::{Commit, FileDiff, ReflogEntry, StatusEntry};
use gpui_kit::component::input::{InputState, TextareaState};
use gpui_kit::{Context, Entity, Point, Pixels, Subscription, Window};

use crate::app::GitTabs;
use crate::graph::GraphRow;

/// A callback the UI keeps: menu items, modal buttons.
pub type Act = Rc<dyn Fn(&mut GitTabs, &mut Window, &mut Context<GitTabs>)>;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum View {
    Changes,
    History,
    Reflog,
}

/// Which list the selected file is in: unstaged, staged, or a commit.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum ListKind {
    U,
    S,
    C,
}

impl ListKind {
    pub fn tag(self) -> &'static str {
        match self {
            ListKind::U => "u",
            ListKind::S => "s",
            ListKind::C => "c",
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Sel {
    pub list: ListKind,
    pub path: String,
}

/// What the File Tree tab shows of the picked file.
#[derive(Clone)]
pub enum FileShown {
    Loading,
    Lines(Arc<Vec<gpui_kit::SharedString>>),
    /// Why there is nothing to read: binary, too large, or what git said.
    Note(String),
}

pub struct RepoState {
    pub path: PathBuf,
    pub name: String,
    pub summary: Option<Summary>,
    pub status: Vec<StatusEntry>,
    pub refs: Option<RefsAll>,
    pub commits: Arc<Vec<Commit>>,
    pub graph: Option<Arc<Vec<GraphRow>>>,
    /// The graph's lanes below the rows laid out so far, to append the next page to.
    pub layout: crate::graph::Layout,
    /// Merges whose merged-in commits are folded away, and then which commits are left: for each
    /// row of the list its place in `commits` (None while nothing is folded).
    pub collapsed: HashSet<String>,
    pub shown: Option<Arc<Vec<usize>>>,
    /// Which commits no remote has yet, by their place in `commits` (drawn with a dot, as in Fork).
    pub unpushed: Arc<Vec<bool>>,
    /// The File Tree tab: the commit whose files are listed, the files, the folders unfolded,
    /// the lines on screen, the file picked and what it held.
    pub tree_of: String,
    pub tree_paths: Arc<Vec<String>>,
    pub tree_open: HashSet<String>,
    pub tree_rows: Arc<Vec<crate::tree::FtRow>>,
    pub tree_sel: Option<String>,
    pub tree_file: Option<FileShown>,
    pub log_done: bool,
    pub log_query: String,
    pub log_path: Option<String>,
    pub log_loading: bool,
    pub view: View,
    pub sel: Option<Sel>,
    pub sel_commit: Option<String>,
    pub detail: Option<CommitDetail>,
    pub reflog: Vec<ReflogEntry>,
    pub diff: Option<FileDiff>,
    pub diff_for: String,
    pub busy: Option<String>,
    pub error: Option<String>,
    /// stage/unstage calls waiting in the serial queue
    pub queued: usize,
    /// ignore file-watcher events caused by our own git calls until then
    pub quiet_until: Option<Instant>,
    pub log_seq: u64,
    pub detail_seq: u64,
}

impl RepoState {
    pub fn new(path: PathBuf) -> Self {
        let name = crate::format::base_name(&path.display().to_string());
        Self {
            path,
            name,
            summary: None,
            status: Vec::new(),
            refs: None,
            commits: Arc::default(),
            graph: None,
            layout: Default::default(),
            collapsed: HashSet::new(),
            shown: None,
            unpushed: Arc::default(),
            tree_of: String::new(),
            tree_paths: Arc::default(),
            tree_open: HashSet::new(),
            tree_rows: Arc::default(),
            tree_sel: None,
            tree_file: None,
            log_done: false,
            log_query: String::new(),
            log_path: None,
            log_loading: false,
            view: View::History,
            sel: None,
            sel_commit: None,
            detail: None,
            reflog: Vec::new(),
            diff: None,
            diff_for: String::new(),
            busy: None,
            error: None,
            queued: 0,
            quiet_until: None,
            log_seq: 0,
            detail_seq: 0,
        }
    }

    pub fn unstaged(&self) -> Vec<&StatusEntry> {
        self.status.iter().filter(|f| f.untracked || f.conflicted || f.y != '.').collect()
    }

    pub fn staged(&self) -> Vec<&StatusEntry> {
        self.status.iter().filter(|f| !f.untracked && !f.conflicted && f.x != '.').collect()
    }

    pub fn changes(&self) -> usize {
        self.summary.as_ref().map_or(0, |s| s.changes)
    }
}

// ---------- overlays ----------

#[derive(Clone)]
pub struct MenuItem {
    pub label: String,
    pub hint: String,
    pub icon: Option<crate::ui::Ico>,
    pub danger: bool,
    pub checked: bool,
    pub header: bool,
    pub sep: bool,
    pub disabled: bool,
    pub on: Option<Act>,
}

impl MenuItem {
    fn blank() -> Self {
        Self { label: String::new(), hint: String::new(), icon: None, danger: false, checked: false, header: false, sep: false, disabled: false, on: None }
    }

    pub fn new(label: impl Into<String>, on: impl Fn(&mut GitTabs, &mut Window, &mut Context<GitTabs>) + 'static) -> Self {
        Self { label: label.into(), on: Some(Rc::new(on)), ..Self::blank() }
    }

    pub fn sep() -> Self {
        Self { sep: true, ..Self::blank() }
    }

    pub fn header(label: impl Into<String>) -> Self {
        Self { header: true, label: label.into(), ..Self::blank() }
    }

    pub fn danger(mut self) -> Self {
        self.danger = true;
        self
    }

    pub fn checked(mut self, v: bool) -> Self {
        self.checked = v;
        self
    }

    pub fn disabled(mut self, v: bool) -> Self {
        self.disabled = v;
        self
    }

    pub fn hint(mut self, h: impl Into<String>) -> Self {
        self.hint = h.into();
        self
    }

    pub fn icon(mut self, i: crate::ui::Ico) -> Self {
        self.icon = Some(i);
        self
    }
}

pub struct MenuState {
    pub pos: Point<Pixels>,
    pub items: Vec<MenuItem>,
    /// Anchored to a button's right edge instead of the click point.
    pub right: bool,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ToastKind {
    Ok,
    Err,
    Info,
}

pub struct Toast {
    pub id: u64,
    pub text: String,
    pub kind: ToastKind,
    pub detail: Option<String>,
}

#[derive(Clone, Copy, PartialEq)]
pub enum FieldKind {
    Text,
    Textarea,
    Checkbox,
}

pub struct FieldSpec {
    pub key: &'static str,
    pub label: String,
    pub kind: FieldKind,
    pub value: String,
    pub placeholder: String,
    pub check: Option<fn(&str) -> Option<String>>,
}

impl FieldSpec {
    pub fn text(key: &'static str, label: &str) -> Self {
        Self { key, label: label.into(), kind: FieldKind::Text, value: String::new(), placeholder: String::new(), check: None }
    }

    pub fn textarea(key: &'static str, label: &str) -> Self {
        Self { kind: FieldKind::Textarea, ..Self::text(key, label) }
    }

    pub fn checkbox(key: &'static str, label: &str, on: bool) -> Self {
        Self { kind: FieldKind::Checkbox, value: if on { "1".into() } else { String::new() }, ..Self::text(key, label) }
    }

    pub fn value(mut self, v: impl Into<String>) -> Self {
        self.value = v.into();
        self
    }

    pub fn placeholder(mut self, v: &str) -> Self {
        self.placeholder = v.into();
        self
    }

    pub fn check(mut self, f: fn(&str) -> Option<String>) -> Self {
        self.check = Some(f);
        self
    }
}

pub struct PromptField {
    pub spec: FieldSpec,
    pub input: Option<Entity<InputState>>,
    pub textarea: Option<Entity<TextareaState>>,
    pub checked: bool,
    pub err: Option<String>,
}

pub type PromptDone = Rc<dyn Fn(&mut GitTabs, std::collections::HashMap<String, String>, &mut Window, &mut Context<GitTabs>)>;
pub type ChoiceDone = Rc<dyn Fn(&mut GitTabs, String, &mut Window, &mut Context<GitTabs>)>;

pub struct Choice {
    pub value: &'static str,
    pub label: String,
    pub primary: bool,
    pub danger: bool,
}

pub struct SshResult {
    pub ok: bool,
    pub output: String,
}

#[derive(Default)]
pub struct KeyGen {
    pub open: bool,
    pub path: String,
    pub public: Option<String>,
    pub err: Option<String>,
    pub busy: bool,
}

/// Editable copy of one identity while the settings dialog is open.
pub struct IdDraft {
    pub id: Identity,
    pub name: Entity<InputState>,
    pub key: Entity<InputState>,
    pub host: Entity<InputState>,
    pub user: Entity<InputState>,
    pub email: Entity<InputState>,
    pub rules: Vec<RuleDraft>,
    pub test: Option<Result<SshResult, ()>>,
    pub testing: bool,
    pub kg: KeyGen,
    pub gen_path: Entity<InputState>,
    pub gen_pass: Entity<InputState>,
}

pub struct RuleDraft {
    /// "remote" or "dir"
    pub t: String,
    pub v: Entity<InputState>,
}

pub struct SettingsModal {
    pub tab: SettingsTab,
    pub ids: Vec<IdDraft>,
    pub theme: String,
    pub git: Entity<InputState>,
    pub fetch: String,
    pub clone: Entity<InputState>,
    pub err: Option<String>,
    pub saving: bool,
    pub scanning: bool,
    pub notice: Option<String>,
    pub _subs: Vec<Subscription>,
}

#[derive(Clone, Copy, PartialEq)]
pub enum SettingsTab {
    Ids,
    General,
}

pub struct AddRepoModal {
    pub scan_dir: Entity<InputState>,
    pub found: Vec<String>,
    pub picked: HashSet<String>,
    pub pin: String,
    pub url: Entity<InputState>,
    pub clone_to: Entity<InputState>,
    pub busy: bool,
    pub err: Option<String>,
    pub _subs: Vec<Subscription>,
}

pub struct BlameModal {
    pub path: String,
    pub rev: Option<String>,
    pub lines: Vec<BlameLine>,
    pub err: Option<String>,
    pub loading: bool,
}

pub struct ConfirmModal {
    pub title: String,
    pub body: String,
    pub ok: String,
    pub danger: bool,
    pub on_ok: Act,
}

pub struct PromptModal {
    pub title: String,
    pub ok: String,
    pub fields: Vec<PromptField>,
    pub on_ok: PromptDone,
    pub _subs: Vec<Subscription>,
}

pub struct ChoiceModal {
    pub title: String,
    pub body: String,
    pub choices: Vec<Choice>,
    pub on_pick: ChoiceDone,
}

pub enum Modal {
    Confirm(ConfirmModal),
    Prompt(PromptModal),
    Choice(ChoiceModal),
    AddRepo(Box<AddRepoModal>),
    Settings(Box<SettingsModal>),
    Blame(BlameModal),
}

pub struct Palette {
    pub input: Entity<InputState>,
    pub sel: usize,
    pub query: String,
    pub _sub: Subscription,
}

/// One entry of the command palette.
pub struct PaletteItem {
    pub title: String,
    pub group: String,
    pub run: Act,
}
