// App state and every action the UI can take. Components read `S` and call these functions.

import { reactive } from 'vue'
import { call, isTauri, watchRepos } from './api'
import type { AppConfig, ChangedFile, Commit, CommitDetail, FileDiff, Identity, LogEntry, ReflogEntry, Refs, StatusEntry, Summary } from './types'
import { diffHash } from './lib/diff'
import { fromFound, matchIdentity, type Found } from './lib/identity'
import { baseName } from './lib/format'
import { treeSort } from './lib/tree'

export type View = 'changes' | 'history' | 'range' | 'reflog'
export type ListKind = 'u' | 's' | 'r' | 'c'

export interface RepoState {
  path: string
  name: string
  summary: Summary | null
  status: StatusEntry[]
  refs: Refs | null
  commits: Commit[]
  logDone: boolean
  logQuery: string
  logPath: string | null
  logLoading: boolean
  view: View
  sel: { list: ListKind; path: string } | null
  selCommit: string | null
  detail: CommitDetail | null
  rangeHead: string | null
  rangeFiles: ChangedFile[]
  rangeCommits: Commit[]
  reflog: ReflogEntry[]
  diff: FileDiff | null
  diffFor: string
  busy: string | null
  error: string | null
  collapsed: string[]
  /** git calls queued by quick actions (stage/unstage), run one at a time */
  queue: Promise<void>
  queued: number
  /** ignore file-watcher events caused by our own git calls until this time */
  quietUntil: number
}

export interface Toast {
  id: number
  text: string
  kind: 'ok' | 'err' | 'info'
  detail?: string
}

export interface MenuItem {
  label?: string
  hint?: string
  icon?: string
  danger?: boolean
  checked?: boolean
  header?: boolean
  sep?: boolean
  disabled?: boolean
  on?: () => void
}

interface ModalReq {
  kind: string
  props: Record<string, unknown>
  resolve: (v: unknown) => void
}

const DEFAULT_CFG: AppConfig = { repos: [], active: 0, identities: [], default_identity: null, theme: 'system', git_path: '', fetch_minutes: 10, clone_dir: '' }

const prefs = (() => {
  try {
    return JSON.parse(localStorage.getItem('git-tabs:prefs') ?? '{}')
  } catch {
    return {}
  }
})()

export const S = reactive({
  ready: false,
  cfg: { ...DEFAULT_CFG } as AppConfig,
  repos: [] as RepoState[],
  active: 0,
  side: !!prefs.side,
  wrap: !!prefs.wrap,
  logOpen: false,
  log: [] as LogEntry[],
  toasts: [] as Toast[],
  modal: null as ModalReq | null,
  menu: null as { x: number; y: number; items: MenuItem[]; minWidth?: number } | null,
  palette: false,
  gitVersion: '',
  seen: (() => {
    try {
      return JSON.parse(localStorage.getItem('git-tabs:seen') ?? '{}') as Record<string, string>
    } catch {
      return {} as Record<string, string>
    }
  })(),
})

export const R = (): RepoState | undefined => S.repos[S.active]

if (import.meta.env.DEV) (window as unknown as { __S: typeof S }).__S = S

export function savePrefs() {
  try {
    localStorage.setItem('git-tabs:prefs', JSON.stringify({ side: S.side, wrap: S.wrap }))
  } catch {
    /* private mode */
  }
}

// ---------- toasts, menus, modals ----------

let toastId = 0
export function toast(text: string, kind: Toast['kind'] = 'ok', detail?: string) {
  const id = ++toastId
  S.toasts.push({ id, text, kind, detail })
  setTimeout(() => dismissToast(id), kind === 'err' ? 9000 : 2800)
}
export const dismissToast = (id: number) => (S.toasts = S.toasts.filter((t) => t.id !== id))

export function openMenu(e: MouseEvent | { x: number; y: number }, items: MenuItem[], minWidth?: number) {
  const pos = 'clientX' in e ? { x: e.clientX, y: e.clientY } : e
  S.menu = { ...pos, items, minWidth }
}
export function menuAt(el: Element, items: MenuItem[], align: 'left' | 'right' = 'left') {
  const r = el.getBoundingClientRect()
  S.menu = { x: align === 'left' ? r.left : r.right, y: r.bottom + 4, items, minWidth: align === 'right' ? -1 : undefined }
}

export function openModal<T = unknown>(kind: string, props: Record<string, unknown> = {}): Promise<T | null> {
  return new Promise((resolve) => {
    S.modal?.resolve(null)
    S.modal = { kind, props, resolve: resolve as (v: unknown) => void }
  })
}
export function closeModal(v: unknown = null) {
  const m = S.modal
  S.modal = null
  m?.resolve(v)
}

export const confirm = (title: string, body: string, ok = '確定', danger = false) => openModal<boolean>('confirm', { title, body, ok, danger }).then((v) => !!v)

export interface Field {
  key: string
  label: string
  value?: string
  placeholder?: string
  type?: 'text' | 'textarea' | 'checkbox' | 'select' | 'password'
  options?: { value: string; label: string }[]
  check?: (v: string, all: Record<string, string>) => string | null
}
export const prompt = (title: string, fields: Field[], ok = '確定') => openModal<Record<string, string>>('prompt', { title, fields, ok })

// ---------- config ----------

export async function saveConfig() {
  S.cfg.repos = S.repos.map((r) => ({ path: r.path, pin: S.cfg.repos.find((x) => x.path === r.path)?.pin ?? null }))
  S.cfg.active = S.active
  await call('config_save', { config: S.cfg }).catch((e) => toast('設定存不進去', 'err', String(e)))
}

function newRepo(path: string): RepoState {
  return {
    path,
    name: baseName(path),
    summary: null,
    status: [],
    refs: null,
    commits: [],
    logDone: false,
    logQuery: '',
    logPath: null,
    logLoading: false,
    view: 'changes',
    sel: null,
    selCommit: null,
    detail: null,
    rangeHead: null,
    rangeFiles: [],
    rangeCommits: [],
    reflog: [],
    diff: null,
    diffFor: '',
    busy: null,
    error: null,
    collapsed: [],
    queue: Promise.resolve(),
    queued: 0,
    quietUntil: 0,
  }
}

export async function init() {
  try {
    S.cfg = { ...DEFAULT_CFG, ...(await call<AppConfig>('config_get')) }
  } catch (e) {
    toast('讀不到設定', 'err', String(e))
  }
  S.repos = S.cfg.repos.map((r) => newRepo(r.path))
  S.active = Math.min(S.cfg.active, Math.max(0, S.repos.length - 1))
  S.ready = true
  call<string>('git_version')
    .then((v) => (S.gitVersion = v))
    .catch((e) => toast('找不到 git，請到設定填 git 路徑', 'err', String(e)))
  await Promise.all(S.repos.map((r) => refreshSummary(r)))
  const r = R()
  if (r) await openRepo(r)
  watchRepos(S.repos.map((r) => r.path))
  offerDetectedIdentities()
}

/** First launch only: fill the identity settings from SSH keys and gitconfig already on this computer. */
async function offerDetectedIdentities() {
  const FLAG = 'git-tabs:identities-offered'
  try {
    if (S.cfg.identities.length || localStorage.getItem(FLAG)) return
    localStorage.setItem(FLAG, '1')
  } catch {
    return
  }
  const found = await call<Found>('identities_detect').catch(() => null)
  if (!found) return
  const detected = fromFound(found)
  if (detected.ids.length) openModal('settings', { tab: 'ids', detected })
}

// ---------- loading ----------

const inflight = new Map<string, Promise<void>>()
const again = new Set<string>()

export async function refreshSummary(r: RepoState) {
  try {
    r.summary = await call<Summary>('summary', { repo: r.path })
    r.error = null
  } catch (e) {
    r.error = String(e).replace(/^Error: /, '')
  }
}

/** Reloads everything the current view of a repo shows. Calls made while one runs are merged. */
export function refresh(r: RepoState): Promise<void> {
  if (inflight.has(r.path)) {
    again.add(r.path)
    return inflight.get(r.path)!
  }
  const p = (async () => {
    do {
      again.delete(r.path)
      await doRefresh(r)
    } while (again.has(r.path))
  })().finally(() => inflight.delete(r.path))
  inflight.set(r.path, p)
  return p
}

async function doRefresh(r: RepoState) {
  const oldHead = r.summary?.head
  const oldRefs = JSON.stringify(r.refs?.branches.map((b) => b.sha + b.name)) + JSON.stringify(r.refs?.remotes.map((b) => b.sha + b.name))
  try {
    const snap = await call<{ summary: Summary; status: StatusEntry[]; refs: Refs }>('snapshot', { repo: r.path })
    r.summary = snap.summary
    r.status = snap.status
    r.refs = snap.refs
    r.error = null
    pruneSeen(r)
  } catch (e) {
    r.error = String(e).replace(/^Error: /, '')
    return
  }
  const newRefs = JSON.stringify(r.refs?.branches.map((b) => b.sha + b.name)) + JSON.stringify(r.refs?.remotes.map((b) => b.sha + b.name))
  if (r.commits.length && (oldHead !== r.summary?.head || oldRefs !== newRefs)) await loadLog(r, true)
  if (r.view === 'range') await loadRange(r)
  if (r.view === 'reflog') await loadReflog(r)
  if (r.view === 'changes') ensureSel(r)
  await loadDiff(r)
}

export async function openRepo(r: RepoState) {
  await refresh(r)
  if (r.view === 'history' && !r.commits.length) await loadLog(r, true)
  if (r.view === 'changes' && !r.status.length && !r.summary?.state) {
    r.view = 'history'
    await loadLog(r, true)
  }
}

export async function loadLog(r: RepoState, reset = false) {
  if (r.logLoading && !reset) return
  r.logLoading = true
  try {
    const skip = reset ? 0 : r.commits.length
    const page = await call<Commit[]>('log', { repo: r.path, skip, limit: 400, all: !r.logPath, query: r.logQuery, path: r.logPath })
    r.commits = reset ? page : [...r.commits, ...page]
    r.logDone = page.length < 400
    if (reset && (!r.selCommit || !r.commits.some((c) => c.sha === r.selCommit))) r.selCommit = r.commits[0]?.sha ?? null
    if (reset) await loadDetail(r)
  } catch (e) {
    toast('讀不到歷史', 'err', String(e))
  } finally {
    r.logLoading = false
  }
}

export async function loadDetail(r: RepoState) {
  if (!r.selCommit) {
    r.detail = null
    return
  }
  if (r.detail?.sha === r.selCommit) return loadDiff(r)
  const sha = r.selCommit
  try {
    const d = await call<CommitDetail>('commit_detail', { repo: r.path, sha })
    // Holding ↓ fires many loads; only the latest selection may land.
    if (r.selCommit !== sha) return
    r.detail = d
    const want = r.logPath && r.detail.files.find((f) => f.path === r.logPath)
    r.sel = r.detail.files.length ? { list: 'c', path: want ? want.path : r.detail.files[0].path } : null
    await loadDiff(r)
  } catch (e) {
    r.detail = null
    toast('讀不到 commit', 'err', String(e))
  }
}

export function rangeBase(r: RepoState): string | null {
  return r.refs?.default_branch ?? null
}

export function rangeCandidates(r: RepoState): string[] {
  const base = rangeBase(r)
  return (r.refs?.branches ?? []).filter((b) => b.name !== base).sort((a, b) => Number(b.current) - Number(a.current) || b.time - a.time).map((b) => b.name)
}

export async function loadRange(r: RepoState) {
  const base = rangeBase(r)
  const cands = rangeCandidates(r)
  if (!r.rangeHead || !cands.includes(r.rangeHead)) r.rangeHead = cands[0] ?? null
  if (!base || !r.rangeHead) {
    r.rangeFiles = []
    r.rangeCommits = []
    return
  }
  try {
    const v = await call<{ files: ChangedFile[]; commits: Commit[] }>('range_files', { repo: r.path, base, head: r.rangeHead })
    r.rangeFiles = v.files
    r.rangeCommits = v.commits
    if (!r.sel || r.sel.list !== 'r' || !v.files.some((f) => f.path === r.sel!.path)) r.sel = v.files[0] ? { list: 'r', path: v.files[0].path } : null
  } catch (e) {
    toast('比較失敗', 'err', String(e))
  }
}

export async function loadReflog(r: RepoState) {
  r.reflog = await call<ReflogEntry[]>('reflog', { repo: r.path, limit: 300 }).catch(() => [])
}

export const unstaged = (r: RepoState) => r.status.filter((f) => f.untracked || f.conflicted || f.y !== '.')
export const staged = (r: RepoState) => r.status.filter((f) => !f.untracked && !f.conflicted && f.x !== '.')

function ensureSel(r: RepoState) {
  const ok = r.sel && ((r.sel.list === 'u' && unstaged(r).some((f) => f.path === r.sel!.path)) || (r.sel.list === 's' && staged(r).some((f) => f.path === r.sel!.path)))
  if (ok) return
  // Keep the same file selected when it moves between the two lists.
  const path = r.sel?.path
  if (path && unstaged(r).some((f) => f.path === path)) r.sel = { list: 'u', path }
  else if (path && staged(r).some((f) => f.path === path)) r.sel = { list: 's', path }
  else {
    const first = orderedFiles(r)[0]
    r.sel = first ?? null
  }
}

let diffSeq = 0
export async function loadDiff(r: RepoState) {
  const sel = r.sel
  if (!sel) {
    r.diff = null
    r.diffFor = ''
    return
  }
  const seq = ++diffSeq
  let d: FileDiff
  try {
    if (sel.list === 'c') {
      if (!r.detail) return
      const f = r.detail.files.find((x) => x.path === sel.path)
      d = await call<FileDiff>('commit_file_diff', { repo: r.path, sha: r.detail.sha, path: sel.path, old_path: f?.old_path })
    } else if (sel.list === 'r') {
      const f = r.rangeFiles.find((x) => x.path === sel.path)
      d = await call<FileDiff>('range_file_diff', { repo: r.path, base: rangeBase(r), head: r.rangeHead, path: sel.path, old_path: f?.old_path })
    } else {
      const st = r.status.find((x) => x.path === sel.path)
      d = await call<FileDiff>('diff_file', { repo: r.path, path: sel.path, staged: sel.list === 's', untracked: !!st?.untracked && sel.list === 'u', conflicted: !!st?.conflicted && sel.list === 'u' })
    }
  } catch (e) {
    if (seq === diffSeq) toast('讀不到 diff', 'err', String(e))
    return
  }
  if (seq !== diffSeq || r.sel !== sel) return
  r.diff = d
  r.diffFor = sel.list + ':' + sel.path
  // A file marked as reviewed that changed since gets unmarked.
  const k = seenKey(r, sel.list, sel.path)
  if (k && S.seen[k] && S.seen[k] !== diffHash(d)) {
    delete S.seen[k]
    persistSeen()
  }
}

// ---------- reviewed ("已看") ----------

export function seenKey(r: RepoState, list: ListKind, path: string): string | null {
  if (list === 'u' || list === 's') return `${r.path}|w|${path}`
  if (list === 'r') return `${r.path}|r:${rangeBase(r)}...${r.rangeHead}|${path}`
  return null
}
export const isSeen = (r: RepoState, list: ListKind, path: string) => {
  const k = seenKey(r, list, path)
  return !!k && !!S.seen[k]
}
/** Drops "reviewed" marks of files that are no longer changed, so the saved map doesn't grow forever. */
function pruneSeen(r: RepoState) {
  const prefix = `${r.path}|w|`
  const live = new Set(r.status.map((f) => f.path))
  let changed = false
  for (const k of Object.keys(S.seen))
    if (k.startsWith(prefix) && !live.has(k.slice(prefix.length))) {
      delete S.seen[k]
      changed = true
    }
  if (changed) persistSeen()
}

function persistSeen() {
  try {
    localStorage.setItem('git-tabs:seen', JSON.stringify(S.seen))
  } catch {
    /* ignore */
  }
}

export function orderedFiles(r: RepoState): { list: ListKind; path: string }[] {
  const sort = treeSort
  if (r.view === 'range') return sort(r.rangeFiles).map((f) => ({ list: 'r' as const, path: f.path }))
  if (r.view === 'history') return (r.detail?.files ?? []).map((f) => ({ list: 'c' as const, path: f.path }))
  return [...sort(unstaged(r)).map((f) => ({ list: 'u' as const, path: f.path })), ...sort(staged(r)).map((f) => ({ list: 's' as const, path: f.path }))]
}

export function toggleSeen(r: RepoState, advance = true) {
  const sel = r.sel
  if (!sel || !r.diff) return
  const k = seenKey(r, sel.list, sel.path)
  if (!k) return
  if (S.seen[k]) delete S.seen[k]
  else {
    S.seen[k] = diffHash(r.diff)
    if (advance) {
      const next = orderedFiles(r).find((f) => !isSeen(r, f.list, f.path))
      if (next) select(r, next.list, next.path)
      else toast('全部檔案都看完了')
    }
  }
  persistSeen()
}

export function select(r: RepoState, list: ListKind, path: string) {
  r.sel = { list, path }
  loadDiff(r)
}

export function moveSel(r: RepoState, d: number) {
  const files = orderedFiles(r)
  if (!files.length) return
  let i = files.findIndex((f) => r.sel && f.list === r.sel.list && f.path === r.sel.path)
  i = Math.max(0, Math.min(files.length - 1, i + d))
  select(r, files[i].list, files[i].path)
}

export function moveCommit(r: RepoState, d: number) {
  const i = r.commits.findIndex((c) => c.sha === r.selCommit)
  const c = r.commits[Math.max(0, Math.min(r.commits.length - 1, i + d))]
  if (!c || c.sha === r.selCommit) return
  r.selCommit = c.sha
  loadDetail(r)
}

// ---------- identity ----------

export function pinOf(r: RepoState): string | null {
  return S.cfg.repos.find((x) => x.path === r.path)?.pin ?? null
}

export function identityOf(r: RepoState): { id: Identity | null; pinned: boolean; auto: Identity | null } {
  const auto = matchIdentity(S.cfg.identities, r.path, r.summary?.remote_url ?? null)
  const pin = pinOf(r)
  const pinned = pin ? S.cfg.identities.find((i) => i.id === pin) ?? null : null
  return { id: pinned ?? auto, pinned: !!pinned, auto }
}

export async function setPin(r: RepoState, id: string | null) {
  const ident = id ? S.cfg.identities.find((i) => i.id === id) ?? null : null
  await op(r, ident ? `改用 ${ident.name}` : '改回自動', () => call('identity_pin', { repo: r.path, identity: ident }), { quiet: true })
  const e = S.cfg.repos.find((x) => x.path === r.path)
  if (e) e.pin = id
  await saveConfig()
  const now = identityOf(r)
  toast(id ? `${r.name} 改用 ${ident?.name}（${ident?.account || ident?.email}）` : `${r.name} 改回自動` + (now.auto ? `：${now.auto.name}` : '（沒有符合的規則）'))
}

// ---------- running operations ----------

let logSince = 0
export async function pullLog() {
  const items = await call<LogEntry[]>('command_log', { since: logSince }).catch(() => [])
  if (items.length) {
    logSince = items[items.length - 1].id
    S.log.push(...items)
    if (S.log.length > 500) S.log.splice(0, S.log.length - 500)
  }
}

/** Runs one action on a repo: shows it as busy, reports errors, then reloads. */
export async function op<T>(r: RepoState, label: string, fn: () => Promise<T>, o: { ok?: string; quiet?: boolean; all?: boolean } = {}): Promise<T | undefined> {
  r.busy = label
  let failed: string | null = null
  let v: T | undefined
  try {
    v = await fn()
    if (!o.quiet) toast(o.ok ?? `${label}完成`)
  } catch (e) {
    failed = String(e).replace(/^Error: /, '')
  } finally {
    r.busy = null
    await pullLog()
    await refresh(r)
  }
  if (failed !== null) {
    // A merge/rebase/cherry-pick that stops on conflicts isn't an error to dismiss: go resolve them.
    const conflicts = r.status.filter((f) => f.conflicted)
    if (r.summary?.state && conflicts.length) {
      toast(`${label}有 ${conflicts.length} 個衝突要解決`, 'info')
      if (r.view !== 'changes') await setView(r, 'changes')
      select(r, 'u', conflicts[0].path)
    } else toast(`${label}失敗`, 'err', failed)
  }
  return v
}

export const git = (r: RepoState, cmd: string, args: Record<string, unknown> = {}) => call(cmd, { repo: r.path, ...args })

// ---------- quick actions ----------

/** Just the file list (one git call), for after stage/unstage where nothing else changed. */
export async function refreshStatus(r: RepoState) {
  const st = await call<StatusEntry[]>('status', { repo: r.path }).catch(() => null)
  if (!st) return refresh(r)
  r.status = st
  pruneSeen(r)
  if (r.summary) {
    r.summary.changes = st.length
    r.summary.conflicts = st.filter((f) => f.conflicted).length
  }
  if (r.view === 'changes') {
    ensureSel(r)
    await loadDiff(r)
  }
}

/**
 * Updates the screen first (`apply`), then runs the git call in the background, queued so
 * two quick key presses never fight over .git/index.lock. Only the last call in a burst
 * reloads the status.
 */
export function quick(r: RepoState, label: string, apply: () => void, fn: () => Promise<unknown>): Promise<void> {
  apply()
  r.queued++
  r.queue = r.queue.then(async () => {
    try {
      await fn()
    } catch (e) {
      toast(`${label}失敗`, 'err', String(e).replace(/^Error: /, ''))
    }
    r.queued--
    await pullLog()
    if (!r.queued) {
      await refreshStatus(r)
      r.quietUntil = Date.now() + 1500
    }
  })
  return r.queue
}

// ---------- repos ----------

export async function addRepos(paths: string[], pin: string | null = null) {
  let added = 0
  for (const p of paths) {
    let top: string
    try {
      top = await call<string>('repo_resolve', { path: p })
    } catch (e) {
      toast('不是 git repo', 'err', String(e))
      continue
    }
    const norm = top.replace(/\//g, '\\')
    if (S.repos.some((r) => r.path.replace(/\//g, '\\').toLowerCase() === norm.toLowerCase())) continue
    const r = newRepo(top)
    S.repos.push(r)
    S.cfg.repos.push({ path: top, pin })
    added++
    await refreshSummary(r)
    if (pin) {
      const ident = S.cfg.identities.find((i) => i.id === pin)
      if (ident) await call('identity_pin', { repo: top, identity: ident }).catch(() => {})
    }
  }
  if (added) {
    S.active = S.repos.length - 1
    await saveConfig()
    await openRepo(S.repos[S.active])
    watchRepos(S.repos.map((r) => r.path))
  }
  return added
}

export async function closeRepo(i: number) {
  const r = S.repos[i]
  S.repos.splice(i, 1)
  S.cfg.repos = S.cfg.repos.filter((x) => x.path !== r.path)
  if (S.active >= S.repos.length) S.active = Math.max(0, S.repos.length - 1)
  await saveConfig()
  watchRepos(S.repos.map((r) => r.path))
  const cur = R()
  if (cur) openRepo(cur)
}

export async function switchTab(i: number) {
  if (i < 0 || i >= S.repos.length) return
  S.active = i
  saveConfig()
  await openRepo(S.repos[i])
}

export function moveTab(from: number, to: number) {
  const cur = R()
  const [r] = S.repos.splice(from, 1)
  S.repos.splice(to, 0, r)
  S.active = cur ? S.repos.indexOf(cur) : 0
  saveConfig()
}

export async function setView(r: RepoState, v: View) {
  r.view = v
  r.sel = null
  r.diff = null
  if (v === 'history') {
    r.detail = null
    if (!r.commits.length) await loadLog(r, true)
    else await loadDetail(r)
  } else if (v === 'range') {
    await loadRange(r)
    await loadDiff(r)
  } else if (v === 'reflog') await loadReflog(r)
  else {
    ensureSel(r)
    await loadDiff(r)
  }
}

// ---------- background work ----------

let pollTimer: ReturnType<typeof setInterval> | undefined
let fetchTimer: ReturnType<typeof setInterval> | undefined

export function startTimers() {
  clearInterval(pollTimer)
  // The desktop app hears about changes from the file watcher; a browser has to poll.
  let tick = 0
  pollTimer = setInterval(
    () => {
      tick++
      const r = R()
      if (r && !r.busy && !r.queued && document.visibilityState === 'visible') refresh(r)
      if (tick % 6 === 0) S.repos.forEach((x) => x !== r && refreshSummary(x))
    },
    isTauri ? 30000 : 4000,
  )
  setFetchTimer()
}

export function setFetchTimer() {
  clearInterval(fetchTimer)
  const m = S.cfg.fetch_minutes
  if (m > 0) fetchTimer = setInterval(() => fetchAll(true), m * 60000)
}

export async function fetchAll(quiet = false) {
  await Promise.all(S.repos.map((r) => call('fetch', { repo: r.path }).catch(() => null)))
  await pullLog()
  await Promise.all(S.repos.map((r) => (r === R() ? refresh(r) : refreshSummary(r))))
  if (!quiet) toast(`已 fetch ${S.repos.length} 個 repo`)
}

export function onRepoChangedOnDisk(path: string) {
  const r = S.repos.find((x) => x.path.replace(/\\/g, '/').toLowerCase() === path.replace(/\\/g, '/').toLowerCase())
  if (!r || r.busy || r.queued || Date.now() < r.quietUntil) return
  if (r === R()) refresh(r)
  else refreshSummary(r)
}
