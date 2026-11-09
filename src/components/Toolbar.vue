<script setup lang="ts">
import { computed } from 'vue'
import { S, menuAt, openModal, saveConfig, identityOf, setPin, type RepoState, type MenuItem } from '../store'
import { fetchRepo, pullRepo, pushRepo, syncRepo, stashPush, newBranch, switchBranch, openFolder, openTerminal } from '../actions'
import { tagOf } from '../lib/identity'
import IdChip from './IdChip.vue'

const p = defineProps<{ r: RepoState }>()
const s = computed(() => p.r.summary)
const who = computed(() => identityOf(p.r))

function branchMenu(e: MouseEvent) {
  const r = p.r
  const local = r.refs?.branches ?? []
  const localNames = new Set(local.map((b) => b.name))
  const remotes = (r.refs?.remotes ?? []).filter((b) => !localNames.has(b.name.replace(/^[^/]+\//, '')))
  const items: MenuItem[] = [
    { header: true, label: '本地分支' },
    ...local.map((b) => ({ label: b.name, checked: b.current, hint: b.ahead || b.behind ? `${b.ahead ? '↑' + b.ahead : ''} ${b.behind ? '↓' + b.behind : ''}` : '', on: () => !b.current && switchBranch(r, b.name) })),
  ]
  if (remotes.length) items.push({ header: true, label: '遠端（切換會建立本地分支）' }, ...remotes.slice(0, 30).map((b) => ({ label: b.name, on: () => switchBranch(r, b.name) })))
  items.push({ sep: true }, { label: '新增分支…', icon: '＋', on: () => newBranch(r) })
  menuAt(e.currentTarget as Element, items)
}

const pullMenu = (e: MouseEvent) =>
  menuAt(e.currentTarget as Element, [
    { label: 'Pull（只快轉）', on: () => pullRepo(p.r, 'ff') },
    { label: 'Pull --rebase', on: () => pullRepo(p.r, 'rebase') },
    { label: 'Pull（merge）', on: () => pullRepo(p.r, 'merge') },
  ])

const pushMenu = (e: MouseEvent) =>
  menuAt(e.currentTarget as Element, [
    { label: 'Push', on: () => pushRepo(p.r) },
    { label: '強制 Push（--force-with-lease）', danger: true, on: () => pushRepo(p.r, true) },
  ])

function idMenu(e: MouseEvent) {
  const r = p.r
  const w = who.value
  const items: MenuItem[] = [
    { header: true, label: `這個 repo（${r.name}）用哪個身分` },
    { label: '自動（依規則）', checked: !w.pinned, hint: w.auto ? '目前是 ' + w.auto.name : '沒有符合的規則', on: () => w.pinned && setPin(r, null) },
    ...S.cfg.identities.map((id) => ({ label: `${tagOf(id)}　${id.name}`, checked: w.pinned && w.id?.id === id.id, hint: id.account || id.email || '未設定', on: () => setPin(r, id.id) })),
    { sep: true },
    { label: '管理身分…', icon: '⚙', on: () => openModal('settings', { tab: 'ids' }) },
  ]
  menuAt(e.currentTarget as Element, items, 'right')
}

function cycleTheme() {
  S.cfg.theme = ({ system: 'light', light: 'dark', dark: 'system' } as const)[S.cfg.theme]
  saveConfig()
}
</script>

<template>
  <div class="bar">
    <button class="btn" title="切換分支" @click="branchMenu">
      ⑂ <b>{{ s?.branch ?? (s?.head ? '(detached ' + s.head.slice(0, 7) + ')' : '…') }}</b> ▾
    </button>
    <span class="sep" />
    <button class="btn primary" :disabled="!!r.busy" title="fetch → pull → push" @click="syncRepo(r)">
      ⟳ Sync<template v-if="s?.behind"> ↓{{ s.behind }}</template><template v-if="s?.ahead"> ↑{{ s.ahead }}</template>
    </button>
    <button class="btn" :disabled="!!r.busy" @click="fetchRepo(r)">Fetch</button>
    <span class="grp">
      <button class="btn" :disabled="!!r.busy" @click="pullRepo(r)">Pull<span v-if="s?.behind" class="down">&nbsp;↓{{ s.behind }}</span></button><button class="btn split" @click="pullMenu">▾</button>
    </span>
    <span class="grp">
      <button class="btn" :disabled="!!r.busy" @click="pushRepo(r)">Push<span v-if="s?.ahead" class="up">&nbsp;↑{{ s.ahead }}</span></button><button class="btn split" @click="pushMenu">▾</button>
    </span>
    <span class="sep" />
    <button class="btn" @click="stashPush(r)">Stash</button>
    <button class="btn" @click="newBranch(r)">＋ 分支</button>
    <span v-if="r.busy" class="sub busy">{{ r.busy }} 執行中…</span>
    <span class="grow" />
    <button v-if="S.cfg.identities.length" class="btn" title="切換這個 repo 用的身分" @click="idMenu">
      👤 <span v-if="who.id">{{ who.id.account || who.id.email || who.id.name }}</span><span v-else class="down">沒有對應的身分</span>
      <IdChip :r="r" /> ▾
    </button>
    <button v-else class="btn" title="設定 SSH key 和身分" @click="openModal('settings', { tab: 'ids' })">👤 設定身分</button>
    <button class="btn" title="搜尋指令、repo、分支、commit" @click="S.palette = true">🔍 <kbd>Ctrl K</kbd></button>
    <button class="btn icon" title="在檔案總管開啟" @click="openFolder(r.path)">📂</button>
    <button class="btn icon" title="開終端機" @click="openTerminal(r.path)">⌨</button>
    <button class="btn icon" :title="'外觀：' + { system: '跟系統', light: '淺色', dark: '深色' }[S.cfg.theme]" @click="cycleTheme">{{ { system: '◐', light: '☀', dark: '☾' }[S.cfg.theme] }}</button>
    <button class="btn icon" title="設定" @click="openModal('settings', { tab: 'ids' })">⚙</button>
  </div>
</template>

<style scoped>
.bar { display: flex; align-items: center; gap: 6px; padding: 0 10px; background: var(--panel); border-bottom: 1px solid var(--border); overflow: hidden; }
.grp { display: inline-flex; }
.grp .btn:first-child { border-top-right-radius: 0; border-bottom-right-radius: 0; }
.grp .split { margin-left: -1px; }
b { font-weight: 600; }
.busy { margin-left: 6px; }
</style>
