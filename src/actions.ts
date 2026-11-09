// User-facing actions: asks for confirmation or input where needed, then runs the git op.

import { call, copyText } from './api'
import { S, op, quick, git, confirm, prompt, openModal, toast, refresh, pullLog, loadLog, setView, select, orderedFiles, addRepos, identityOf, staged, unstaged, type RepoState } from './store'
import { short } from './lib/format'
import { lineSel } from './lineSel'

const branchName = (v: string) => (!v.trim() ? '輸入分支名稱' : /\s|\.\.|[~^:?*[\\]|^-|\/$|\.lock$/.test(v.trim()) ? '分支名稱不能有空白或 ~ ^ : ? * [ \\ 這些字元' : null)

/** When the selected file leaves a list, select the next file in that list (or follow it). */
function followSelection(r: RepoState, moved: Set<string>, from: 'u' | 's', to: 'u' | 's') {
  if (!r.sel || r.sel.list !== from || !moved.has(r.sel.path)) return
  const list = orderedFiles(r).filter((f) => f.list === from)
  const i = list.findIndex((f) => f.path === r.sel!.path)
  const next = list.slice(i + 1).find((f) => !moved.has(f.path)) ?? list.slice(0, i).reverse().find((f) => !moved.has(f.path))
  if (next) select(r, from, next.path)
  else r.sel = { list: to, path: r.sel.path }
}

export function stageFiles(r: RepoState, paths: string[]) {
  if (!paths.length) return
  const set = new Set(paths)
  return quick(
    r,
    'Stage',
    () => {
      followSelection(r, set, 'u', 's')
      for (const f of r.status) {
        if (!set.has(f.path) || f.conflicted) continue
        f.x = f.untracked ? 'A' : f.y !== '.' ? f.y : f.x
        f.y = '.'
        f.untracked = false
      }
    },
    () => git(r, 'stage', { paths }),
  )
}

export function unstageFiles(r: RepoState, paths: string[]) {
  if (!paths.length) return
  const set = new Set(paths)
  return quick(
    r,
    '取消 Stage',
    () => {
      followSelection(r, set, 's', 'u')
      for (const f of r.status) {
        if (!set.has(f.path)) continue
        if (f.x === 'A') {
          f.untracked = true
          f.y = '?'
        } else if (f.y === '.') f.y = f.x
        f.x = '.'
      }
    },
    () => git(r, 'unstage', { paths }),
  )
}
export const stageAll = (r: RepoState) => stageFiles(r, unstaged(r).filter((f) => !f.conflicted).map((f) => f.path))
export const unstageAll = (r: RepoState) => unstageFiles(r, staged(r).map((f) => f.path))

export function stageToggle(r: RepoState) {
  if (lineSel.handler?.()) return
  if (!r.sel) return
  if (r.sel.list === 'u') return stageFiles(r, [r.sel.path])
  if (r.sel.list === 's') return unstageFiles(r, [r.sel.path])
}

export async function discardFiles(r: RepoState, paths: string[]) {
  if (!paths.length) return
  const untracked = paths.filter((p) => r.status.some((s) => s.path === p && s.untracked))
  const what = paths.length === 1 ? `<span class="mono">${esc(paths[0])}</span> 的修改` : `${paths.length} 個檔案的修改`
  const extra = untracked.length ? `<br>其中 ${untracked.length} 個是新檔案，會直接<b>刪除</b>。` : ''
  if (!(await confirm('捨棄修改？', `${what}會被丟掉，<b>無法復原</b>。${extra}`, '捨棄', true))) return
  await op(r, '捨棄', () => git(r, 'discard', { paths }), { ok: '已捨棄' })
}

export async function applyLines(r: RepoState, path: string, action: 'stage' | 'unstage' | 'discard', lines: [number, number][]) {
  if (!lines.length) return
  if (action === 'discard' && !(await confirm('捨棄選取的行？', `<span class="mono">${esc(path)}</span> 選取的 ${lines.length} 行修改會被丟掉，<b>無法復原</b>。`, '捨棄', true))) return
  const label = { stage: 'Stage 選取的行', unstage: '取消 Stage 選取的行', discard: '捨棄選取的行' }[action]
  await quick(r, label, () => {}, () => git(r, 'apply_lines', { path, action, lines }))
}

export async function commit(r: RepoState, message: string, amend: boolean, push: boolean): Promise<boolean> {
  // Let queued stage/unstage calls land first.
  await r.queue
  const who = identityOf(r)
  if (S.cfg.identities.length && !who.id) {
    const def = S.cfg.identities.find((i) => i.id === S.cfg.default_identity)
    const body = `<b>${esc(r.name)}</b>（${esc(r.summary?.remote_url ?? '沒有 remote')}）沒有符合任何身分的規則，${def ? `會用預設的「${esc(def.name)}」` : '會用 git 的全域設定'}提交。<br><br>可以點工具列的 👤 按鈕把它指定給某個身分。`
    if (!(await confirm('這個 repo 沒有對應的身分', body, '照樣提交'))) return false
  }
  const sha = await op(r, amend ? 'Amend' : 'Commit', () => git(r, 'commit', { message, amend }) as Promise<string>, { quiet: true })
  if (!sha) return false
  toast(`${amend ? '已 amend' : '已 commit'}：${message.split('\n')[0]}`)
  if (push) await pushRepo(r)
  return true
}

export const fetchRepo = (r: RepoState) => op(r, 'Fetch', () => git(r, 'fetch'), { ok: `已 fetch ${r.name}` })

export async function pullRepo(r: RepoState, mode: 'ff' | 'rebase' | 'merge' = 'ff') {
  r.busy = 'Pull'
  try {
    await git(r, 'pull', { mode })
    toast(`已 pull ${r.name}`)
  } catch (e) {
    const msg = String(e).replace(/^Error: /, '')
    if (mode === 'ff' && /fast-forward|diverg|Not possible/i.test(msg)) {
      const pick = await openModal<string>('choice', {
        title: '本地和遠端各有新的 commit',
        body: '沒辦法直接快轉。要用哪種方式合併遠端的修改？',
        choices: [
          { value: 'rebase', label: 'Rebase（把我的 commit 接在遠端後面）', primary: true },
          { value: 'merge', label: 'Merge（多一個 merge commit）' },
        ],
      })
      r.busy = null
      if (pick === 'rebase' || pick === 'merge') return pullRepo(r, pick)
    } else toast('Pull 失敗', 'err', msg)
  } finally {
    r.busy = null
    await pullLog()
    await refresh(r)
  }
}

export async function pushRepo(r: RepoState, force = false) {
  if (force && !(await confirm('強制 Push？', `會用 <span class="mono">--force-with-lease</span> 覆蓋遠端的 <b>${esc(r.summary?.branch ?? '')}</b>。如果遠端有你還沒拿到的 commit，git 會拒絕，不會蓋掉別人的東西。`, '強制 Push', true))) return
  await op(r, 'Push', () => git(r, 'push', { force }), { ok: `已 push ${r.name}` })
}

export async function syncRepo(r: RepoState) {
  r.busy = 'Sync'
  try {
    await git(r, 'fetch')
    const s = await call<{ ahead: number; behind: number; upstream: string | null }>('summary', { repo: r.path })
    if (s.behind) await git(r, 'pull', { mode: 'ff' })
    if (s.ahead || !s.upstream) await git(r, 'push', { force: false })
    toast(`${r.name} 已同步` + (s.behind ? `，拉下 ${s.behind}` : '') + (s.ahead ? `，推上 ${s.ahead}` : ''))
  } catch (e) {
    const msg = String(e).replace(/^Error: /, '')
    toast('Sync 沒有完成', 'err', msg)
  } finally {
    r.busy = null
    await pullLog()
    await refresh(r)
  }
}

export const switchBranch = (r: RepoState, name: string) => op(r, '切換分支', () => git(r, 'switch', { name }), { ok: `已切換到 ${name}` })

export async function newBranch(r: RepoState, from?: string) {
  const v = await prompt(
    '新增分支',
    [
      { key: 'name', label: '名稱', placeholder: 'feat/my-change', check: branchName },
      { key: 'from', label: '從', value: from ?? r.summary?.branch ?? 'HEAD' },
      { key: 'checkout', label: '建好就切換過去', type: 'checkbox', value: '1' },
    ],
    '建立',
  )
  if (!v) return
  await op(r, '建立分支', () => git(r, 'create_branch', { name: v.name.trim(), from: v.from.trim(), checkout: v.checkout === '1' }), { ok: `已建立 ${v.name.trim()}` })
}

export async function deleteBranch(r: RepoState, name: string) {
  const b = r.refs?.branches.find((x) => x.name === name)
  if (!(await confirm('刪除分支？', `刪除本地分支 <b>${esc(name)}</b>${b?.upstream ? `（遠端的 ${esc(b.upstream)} 不會被刪）` : ''}。`, '刪除', true))) return
  const res = await op(r, '刪除分支', () => git(r, 'delete_branch', { name, force: false }), { quiet: true })
  if (res !== undefined) return toast(`已刪除 ${name}`)
  if (await confirm('分支還沒合併', `<b>${esc(name)}</b> 有還沒合併的 commit，強制刪除後只能從 Reflog 找回。`, '強制刪除', true))
    await op(r, '刪除分支', () => git(r, 'delete_branch', { name, force: true }), { ok: `已刪除 ${name}` })
}

export async function simpleOp(r: RepoState, opName: string, arg: string, label: string, ok?: string) {
  return op(r, label, () => git(r, 'op', { op: opName, arg }), { ok })
}

export async function mergeInto(r: RepoState, name: string) {
  const cur = r.summary?.branch ?? 'HEAD'
  if (await confirm('Merge', `把 <b>${esc(name)}</b> merge 進目前的 <b>${esc(cur)}</b>？`, 'Merge')) await simpleOp(r, 'merge', name, 'Merge', `已把 ${name} merge 進 ${cur}`)
}

export async function rebaseOnto(r: RepoState, name: string) {
  const cur = r.summary?.branch ?? 'HEAD'
  if (await confirm('Rebase', `把目前的 <b>${esc(cur)}</b> rebase 到 <b>${esc(name)}</b> 上？已經 push 過的 commit 會被改寫。`, 'Rebase')) await simpleOp(r, 'rebase', name, 'Rebase')
}

export async function cherryPick(r: RepoState, sha: string) {
  await simpleOp(r, 'cherry-pick', sha, 'Cherry-pick', `已 cherry-pick ${short(sha)}`)
}

export async function revert(r: RepoState, sha: string) {
  if (await confirm('Revert', `新增一個 commit 來抵銷 <span class="mono">${short(sha)}</span> 的修改？`, 'Revert')) await simpleOp(r, 'revert', sha, 'Revert', `已 revert ${short(sha)}`)
}

export async function resetTo(r: RepoState, sha: string) {
  const mode = await openModal<string>('choice', {
    title: `Reset ${r.summary?.branch ?? 'HEAD'} 到 ${short(sha)}`,
    body: '選擇怎麼處理目前的修改：',
    choices: [
      { value: 'soft', label: 'soft：保留修改，而且保持已 stage' },
      { value: 'mixed', label: 'mixed：保留修改，取消 stage', primary: true },
      { value: 'hard', label: 'hard：丟掉所有修改（無法復原）', danger: true },
    ],
  })
  if (!mode) return
  if (mode === 'hard' && !(await confirm('確定 hard reset？', '所有還沒 commit 的修改都會消失，<b>無法復原</b>。', 'Hard reset', true))) return
  await simpleOp(r, `reset-${mode}`, sha, 'Reset', `已 reset --${mode} 到 ${short(sha)}`)
}

export async function checkoutCommit(r: RepoState, sha: string) {
  if (await confirm('Checkout 這個 commit？', `會進入 detached HEAD 狀態，停在 <span class="mono">${short(sha)}</span>。之後要繼續工作，請切回分支或從這裡開新分支。`, 'Checkout'))
    await simpleOp(r, 'checkout', sha, 'Checkout')
}

export async function createTag(r: RepoState, sha: string) {
  const v = await prompt(`在 ${short(sha)} 加 Tag`, [
    { key: 'name', label: '名稱', placeholder: 'v1.0.0', check: (x) => (x.trim() && !/\s/.test(x) ? null : '輸入 tag 名稱（不能有空白）') },
    { key: 'message', label: '說明（可留空）', type: 'textarea' },
  ])
  if (v) await op(r, '加 Tag', () => git(r, 'tag', { name: v.name.trim(), sha, message: v.message }), { ok: `已加上 ${v.name.trim()}` })
}

export async function stashPush(r: RepoState) {
  if (!r.status.length) return toast('沒有修改可以 stash', 'info')
  const v = await prompt('Stash 目前的修改', [
    { key: 'message', label: '說明（可留空）', placeholder: '做到一半的 xxx' },
    { key: 'untracked', label: '包含新檔案', type: 'checkbox', value: '1' },
  ], 'Stash')
  if (v) await op(r, 'Stash', () => git(r, 'stash_push', { message: v.message, untracked: v.untracked === '1' }), { ok: '已 stash' })
}

export async function stashDrop(r: RepoState, index: number) {
  if (await confirm('刪除 stash？', '刪除後無法復原。', '刪除', true)) await simpleOp(r, 'stash-drop', String(index), '刪除 stash', '已刪除 stash')
}

export async function abortOrContinue(r: RepoState, what: 'abort' | 'continue') {
  if (what === 'abort' && !(await confirm(`中止 ${r.summary?.state}？`, '會回到開始之前的狀態，解到一半的衝突會被丟掉。', '中止', true))) return
  const name = r.summary?.state ?? ''
  await simpleOp(r, what, '', what === 'abort' ? '中止' : '繼續', what === 'abort' ? `已中止 ${name}` : `已完成 ${name}`)
}

export async function openFolder(path: string) {
  await call('open_folder', { path }).catch((e) => toast('打不開資料夾', 'err', String(e)))
}
export async function openTerminal(path: string) {
  await call('open_terminal', { path }).catch((e) => toast('打不開終端機', 'err', String(e)))
}
export async function openFile(r: RepoState, rel: string) {
  await call('open_file', { path: `${r.path}/${rel}`.replace(/\//g, '\\') }).catch((e) => toast('打不開檔案', 'err', String(e)))
}
export async function copy(text: string, what = text) {
  await copyText(text)
  toast(`已複製 ${what}`)
}

export async function fileHistory(r: RepoState, path: string) {
  r.logPath = path
  r.logQuery = ''
  r.commits = []
  r.detail = null
  await setView(r, 'history')
}
export async function clearFileHistory(r: RepoState) {
  r.logPath = null
  r.commits = []
  r.detail = null
  await loadLog(r, true)
}

export const blame = (r: RepoState, path: string, rev?: string) => openModal('blame', { repo: r.path, path, rev })

export async function openWorktree(path: string) {
  await addRepos([path])
}

export async function removeWorktree(r: RepoState, path: string) {
  if (await confirm('移除 worktree？', `移除 <span class="mono">${esc(path)}</span>。分支本身會保留。有未提交的修改時 git 會拒絕。`, '移除', true))
    await simpleOp(r, 'worktree-remove', path, '移除 worktree')
}

export async function resolveConflict(r: RepoState, path: string, how: 'use-ours' | 'use-theirs' | 'mark-resolved') {
  await simpleOp(r, how, path, { 'use-ours': '用我的版本', 'use-theirs': '用對方的版本', 'mark-resolved': '標記已解決' }[how], '已解決 ' + path.split('/').pop())
  const next = unstaged(r).find((f) => f.conflicted)
  if (next) select(r, 'u', next.path)
}

export function esc(s: string) {
  return s.replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' })[c]!)
}

