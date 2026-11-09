// Shapes returned by gitcore (crates/gitcore). Keep in sync with the Rust structs.

export interface Summary {
  path: string
  name: string
  branch: string | null
  head: string | null
  upstream: string | null
  ahead: number
  behind: number
  changes: number
  conflicts: number
  remote_url: string | null
  state: 'merge' | 'rebase' | 'cherry-pick' | 'revert' | null
}

export interface StatusEntry {
  path: string
  orig_path: string | null
  x: string
  y: string
  untracked: boolean
  conflicted: boolean
}

export interface DiffLine {
  t: ' ' | '+' | '-'
  text: string
  no_eol?: boolean
}

export interface Hunk {
  header: string
  old_start: number
  old_lines: number
  new_start: number
  new_lines: number
  lines: DiffLine[]
}

export interface FileDiff {
  path: string
  old_path: string | null
  binary: boolean
  new_file: boolean
  deleted: boolean
  hunks: Hunk[]
}

export interface RefLabel {
  name: string
  kind: 'head' | 'branch' | 'remote' | 'tag' | 'detached'
}

export interface Commit {
  sha: string
  parents: string[]
  author: string
  email: string
  time: number
  refs: RefLabel[]
  subject: string
}

export interface ChangedFile {
  path: string
  old_path: string | null
  status: string
  add: number | null
  del: number | null
}

export interface CommitDetail {
  sha: string
  parents: string[]
  author: string
  email: string
  time: number
  committer: string
  commit_time: number
  refs: RefLabel[]
  message: string
  files: ChangedFile[]
}

export interface Branch {
  name: string
  sha: string
  upstream: string | null
  ahead: number
  behind: number
  gone: boolean
  current: boolean
  time: number
}

export interface Refs {
  branches: Branch[]
  remotes: Branch[]
  tags: { name: string; sha: string }[]
  stashes: { index: number; sha: string; message: string }[]
  worktrees: { path: string; head: string; branch: string | null; bare: boolean }[]
  default_branch: string | null
}

export interface ReflogEntry {
  selector: string
  sha: string
  message: string
  time: number
}

export interface BlameLine {
  sha: string
  author: string
  time: number
  summary: string
  line: number
  text: string
}

export interface LogEntry {
  id: number
  repo: string
  cmd: string
  ms: number
  ok: boolean
  output: string
  at: number
}

export interface Rule {
  t: 'remote' | 'dir'
  v: string
}

export interface Identity {
  id: string
  name: string
  short: string
  color: string
  key: string
  account: string
  verified: boolean
  user: string
  email: string
  host: string
  rules: Rule[]
}

export interface RepoEntry {
  path: string
  pin: string | null
}

export interface AppConfig {
  repos: RepoEntry[]
  active: number
  identities: Identity[]
  default_identity: string | null
  theme: 'system' | 'light' | 'dark'
  git_path: string
  fetch_minutes: number
  clone_dir: string
}

export interface SshTest {
  ok: boolean
  account: string | null
  output: string
}
