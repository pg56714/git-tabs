<script setup lang="ts">
import { computed, reactive } from 'vue'
import { setView, openMenu, loadDetail, toast, type RepoState, type View } from '../store'
import { switchBranch, deleteBranch, mergeInto, rebaseOnto, newBranch, simpleOp, stashDrop, openWorktree, removeWorktree, copy } from '../actions'

defineOptions({ name: 'SideBar' })
const p = defineProps<{ r: RepoState }>()
const fold = reactive<Record<string, boolean>>({ Tags: true })
const remotes = computed(() => {
  const g = new Map<string, string[]>()
  for (const b of p.r.refs?.remotes ?? []) {
    const [remote, ...rest] = b.name.split('/')
    if (!g.has(remote)) g.set(remote, [])
    g.get(remote)!.push(rest.join('/'))
  }
  return [...g]
})
const count = computed(() => p.r.summary?.changes ?? 0)

function nav(v: View) {
  setView(p.r, v)
}

/** Shows the commit a branch/tag points at in the history. */
async function show(sha: string) {
  const r = p.r
  if (r.view !== 'history') await setView(r, 'history')
  if (!r.commits.some((c) => c.sha === sha)) return toast('這個 commit 不在目前載入的歷史裡', 'info')
  r.selCommit = sha
  await loadDetail(r)
}

function branchMenu(e: MouseEvent, name: string, current: boolean) {
  const r = p.r
  openMenu(e, [
    { label: '切換到這個分支', disabled: current, on: () => switchBranch(r, name) },
    { label: `Merge 進目前的分支`, disabled: current, on: () => mergeInto(r, name) },
    { label: `把目前的分支 rebase 到這裡`, disabled: current, on: () => rebaseOnto(r, name) },
    { label: '從這裡開新分支…', on: () => newBranch(r, name) },
    { label: '跟 main 比較', on: () => ((r.rangeHead = name), setView(r, 'range')) },
    { label: '複製名稱', on: () => copy(name, '分支名稱') },
    { sep: true },
    { label: '刪除分支…', danger: true, disabled: current, on: () => deleteBranch(r, name) },
  ])
}

function remoteMenu(e: MouseEvent, full: string) {
  const r = p.r
  openMenu(e, [
    { label: '切換（建立本地分支）', on: () => switchBranch(r, full) },
    { label: 'Merge 進目前的分支', on: () => mergeInto(r, full) },
    { label: '從這裡開新分支…', on: () => newBranch(r, full) },
  ])
}

function tagMenu(e: MouseEvent, name: string) {
  const r = p.r
  openMenu(e, [
    { label: 'Push 這個 tag', on: () => simpleOp(r, 'push-tag', name, 'Push tag', `已 push ${name}`) },
    { label: '從這裡開新分支…', on: () => newBranch(r, name) },
    { label: '複製名稱', on: () => copy(name, 'tag') },
    { sep: true },
    { label: '刪除 tag', danger: true, on: () => simpleOp(r, 'tag-delete', name, '刪除 tag', `已刪除 ${name}`) },
  ])
}
</script>

<template>
  <nav class="side">
    <div class="nav" :class="{ on: r.view === 'changes' }" @click="nav('changes')">✎ 未提交的修改 <span class="n">{{ count || '' }}</span></div>
    <div class="nav" :class="{ on: r.view === 'history' }" @click="nav('history')">◷ 歷史</div>
    <div class="nav" :class="{ on: r.view === 'range' }" @click="nav('range')">⇄ 分支 vs {{ r.refs?.default_branch ?? 'main' }}</div>

    <div class="sh" @click="fold.b = !fold.b">{{ fold.b ? '▸' : '▾' }} 分支 <span class="n">{{ r.refs?.branches.length ?? '' }}</span></div>
    <template v-if="!fold.b">
      <div v-for="b in r.refs?.branches" :key="b.name" class="si" :class="{ cur: b.current }" :title="b.upstream ? `追蹤 ${b.upstream}${b.gone ? '（遠端已刪除）' : ''}` : '沒有追蹤遠端'" @click="show(b.sha)" @dblclick="!b.current && switchBranch(r, b.name)" @contextmenu.prevent="branchMenu($event, b.name, b.current)">
        <span class="ck">{{ b.current ? '✓' : '' }}</span><span class="nm">{{ b.name }}</span>
        <span class="n"><span v-if="b.ahead" class="up">↑{{ b.ahead }}</span><span v-if="b.behind" class="down"> ↓{{ b.behind }}</span><span v-if="b.gone" class="down"> 已刪</span></span>
      </div>
    </template>

    <div class="sh" @click="fold.r = !fold.r">{{ fold.r ? '▸' : '▾' }} 遠端</div>
    <template v-if="!fold.r">
      <template v-for="[remote, names] in remotes" :key="remote">
        <div class="si sub2">{{ remote }}</div>
        <div v-for="n in names" :key="n" class="si deep" @contextmenu.prevent="remoteMenu($event, remote + '/' + n)" @click="show(r.refs!.remotes.find((x) => x.name === remote + '/' + n)!.sha)" @dblclick="switchBranch(r, remote + '/' + n)"><span class="nm">{{ n }}</span></div>
      </template>
      <div v-if="!remotes.length" class="si none">沒有遠端分支</div>
    </template>

    <div class="sh" @click="fold.Tags = !fold.Tags">{{ fold.Tags ? '▸' : '▾' }} Tags <span class="n">{{ r.refs?.tags.length || '' }}</span></div>
    <template v-if="!fold.Tags">
      <div v-for="t in r.refs?.tags" :key="t.name" class="si" @click="show(t.sha)" @contextmenu.prevent="tagMenu($event, t.name)">🏷 <span class="nm">{{ t.name }}</span></div>
      <div v-if="!r.refs?.tags.length" class="si none">沒有 tag</div>
    </template>

    <div class="sh" @click="fold.s = !fold.s">{{ fold.s ? '▸' : '▾' }} Stashes <span class="n">{{ r.refs?.stashes.length || '' }}</span></div>
    <template v-if="!fold.s">
      <div v-for="s in r.refs?.stashes" :key="s.sha" class="si" :title="s.message">
        <span class="nm">{{ s.message }}</span>
        <span class="act">
          <button class="btn sm" title="套用但保留這個 stash" @click="simpleOp(r, 'stash-apply', String(s.index), '套用 stash', '已套用 stash')">套用</button>
          <button class="btn sm" title="套用並刪除" @click="simpleOp(r, 'stash-pop', String(s.index), 'Pop stash', '已套用並刪除 stash')">Pop</button>
          <button class="btn sm" title="刪除" @click="stashDrop(r, s.index)">×</button>
        </span>
      </div>
      <div v-if="!r.refs?.stashes.length" class="si none">沒有 stash</div>
    </template>

    <div class="sh" @click="fold.w = !fold.w">{{ fold.w ? '▸' : '▾' }} Worktrees <span class="n">{{ (r.refs?.worktrees.length ?? 1) - 1 || '' }}</span></div>
    <template v-if="!fold.w">
      <div v-for="w in r.refs?.worktrees.slice(1)" :key="w.path" class="si" :title="w.path + '（點一下開成新分頁）'" @click="openWorktree(w.path)" @contextmenu.prevent="openMenu($event, [{ label: '開成新分頁', on: () => openWorktree(w.path) }, { label: '複製路徑', on: () => copy(w.path, '路徑') }, { sep: true }, { label: '移除 worktree…', danger: true, on: () => removeWorktree(r, w.path) }])">
        🌿 <span class="nm">{{ w.branch ?? w.head.slice(0, 7) }}</span>
      </div>
      <div v-if="(r.refs?.worktrees.length ?? 0) <= 1" class="si none">沒有其他 worktree</div>
    </template>

    <div class="nav" style="margin-top: 8px" :class="{ on: r.view === 'reflog' }" @click="nav('reflog')">↺ Reflog</div>
  </nav>
</template>

<style scoped>
.side { background: var(--panel2); border-right: 1px solid var(--border); overflow: auto; padding: 8px 0; user-select: none; }
.nav { display: flex; align-items: center; gap: 8px; margin: 1px 8px; padding: 5px 8px; border-radius: 6px; cursor: pointer; color: var(--muted); }
.nav:hover { background: var(--hover); }
.nav.on { background: var(--accentbg); color: var(--accenttx); }
.n { margin-left: auto; font-size: 11.5px; flex: none; }
.sh { display: flex; align-items: center; gap: 4px; padding: 12px 14px 3px; font-size: 12px; font-weight: 600; color: var(--muted); cursor: pointer; }
.sh .n { font-weight: 400; color: var(--faint); }
.sh:hover { color: var(--text); }
.si { display: flex; align-items: center; gap: 6px; margin: 0 8px; padding: 3px 8px 3px 22px; border-radius: 6px; cursor: pointer; color: var(--text); white-space: nowrap; min-height: 26px; }
.si:hover { background: var(--hover); }
.si.cur { color: var(--text); font-weight: 600; }
.si.deep { padding-left: 38px; }
.si.sub2 { cursor: default; color: var(--muted); }
.si.sub2:hover { background: none; }
.si.none { color: var(--faint); cursor: default; font-size: 12px; }
.si.none:hover { background: none; }
.ck { width: 10px; flex: none; color: var(--ok); }
.nm { overflow: hidden; text-overflow: ellipsis; }
.act { display: none; margin-left: auto; gap: 2px; }
.si:hover .act { display: flex; }
</style>
