<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from 'vue'
import { S, R, switchTab, setView, setPin, openModal, saveConfig, fetchAll, loadDetail, refresh } from '../store'
import { fetchRepo, pullRepo, pushRepo, syncRepo, switchBranch, newBranch, stashPush, openFolder, openTerminal } from '../actions'
import { fuzzy, short } from '../lib/format'
import { tagOf } from '../lib/identity'

interface Item {
  t: string
  g: string
  f: () => void
}

const q = ref('')
const sel = ref(0)
const input = ref<HTMLInputElement | null>(null)
const box = ref<HTMLElement | null>(null)

const items = computed<Item[]>(() => {
  const r = R()
  const v: Item[] = S.repos.map((x, i) => ({ t: `切換到 ${x.name}`, g: 'repo', f: () => switchTab(i) }))
  if (r) {
    v.push(
      { t: 'Sync', g: r.name, f: () => syncRepo(r) },
      { t: 'Fetch', g: r.name, f: () => fetchRepo(r) },
      { t: 'Pull', g: r.name, f: () => pullRepo(r) },
      { t: 'Pull --rebase', g: r.name, f: () => pullRepo(r, 'rebase') },
      { t: 'Push', g: r.name, f: () => pushRepo(r) },
      { t: 'Stash', g: r.name, f: () => stashPush(r) },
      { t: '新增分支', g: r.name, f: () => newBranch(r) },
      { t: '看未提交的修改', g: '檢視', f: () => setView(r, 'changes') },
      { t: '看歷史', g: '檢視', f: () => setView(r, 'history') },
      { t: '看分支 vs main', g: '檢視', f: () => setView(r, 'range') },
      { t: '看 Reflog', g: '檢視', f: () => setView(r, 'reflog') },
      { t: '在檔案總管開啟', g: r.name, f: () => openFolder(r.path) },
      { t: '開終端機', g: r.name, f: () => openTerminal(r.path) },
      { t: '重新整理', g: r.name, f: () => refresh(r) },
      ...(r.refs?.branches ?? []).filter((b) => !b.current).map((b) => ({ t: `切換分支：${b.name}`, g: r.name, f: () => switchBranch(r, b.name) })),
    )
    if (S.cfg.identities.length) {
      v.push({ t: '身分：自動（依規則）', g: r.name, f: () => setPin(r, null) })
      v.push(...S.cfg.identities.map((id) => ({ t: `身分：${id.name} ${tagOf(id)} ${id.account}`, g: r.name, f: () => setPin(r, id.id) })))
    }
    v.push(
      ...S.repos.map((x, i) => ({ t: `Push ${x.name}` + (x.summary?.ahead ? ` (↑${x.summary.ahead})` : ''), g: '遠端', f: () => switchTab(i).then(() => pushRepo(x)) })),
    )
    // Commits are searchable too: "fix depth" jumps to that commit.
    v.push(
      ...r.commits.slice(0, 400).map((c) => ({
        t: c.subject,
        g: short(c.sha),
        f: async () => {
          await setView(r, 'history')
          r.selCommit = c.sha
          await loadDetail(r)
        },
      })),
    )
  }
  v.push(
    { t: '全部 repo 一起 Fetch', g: '遠端', f: () => fetchAll() },
    { t: '加入 repo', g: 'repo', f: () => openModal('addrepo') },
    { t: '開啟設定', g: '設定', f: () => openModal('settings', { tab: 'general' }) },
    { t: '身分與 SSH key', g: '設定', f: () => openModal('settings', { tab: 'ids' }) },
    {
      t: '切換深淺色',
      g: '外觀',
      f: () => {
        S.cfg.theme = ({ system: 'light', light: 'dark', dark: 'system' } as const)[S.cfg.theme]
        saveConfig()
      },
    },
  )
  return v
})

const shown = computed(() => (q.value.trim() ? items.value.filter((x) => fuzzy(q.value, x.t + ' ' + x.g)) : items.value).slice(0, 60))
watch(q, () => (sel.value = 0))
watch(sel, async () => {
  await nextTick()
  box.value?.querySelector('.on')?.scrollIntoView({ block: 'nearest' })
})

function go(i: number) {
  const it = shown.value[i]
  if (!it) return
  S.palette = false
  it.f()
}

function onKey(e: KeyboardEvent) {
  if (e.key === 'ArrowDown') sel.value = Math.min(shown.value.length - 1, sel.value + 1)
  else if (e.key === 'ArrowUp') sel.value = Math.max(0, sel.value - 1)
  else if (e.key === 'Enter') go(sel.value)
  else return
  e.preventDefault()
}

onMounted(() => input.value?.focus())
</script>

<template>
  <div class="shade" @mousedown.self="S.palette = false">
    <div class="pal">
      <input ref="input" v-model="q" placeholder="輸入指令、repo、分支或 commit…（例如：push my-repo）" @keydown="onKey" />
      <div ref="box" class="items">
        <div v-for="(it, i) in shown" :key="i" class="pi" :class="{ on: i === sel }" @mouseenter="sel = i" @click="go(i)">
          <span class="t">{{ it.t }}</span><span class="g">{{ it.g }}</span>
        </div>
        <div v-if="!shown.length" class="pi">沒有符合的項目</div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.shade { position: fixed; inset: 0; background: rgba(10, 12, 16, 0.35); z-index: 50; }
.pal { position: absolute; left: 50%; top: 12vh; transform: translateX(-50%); width: min(620px, 92vw); background: var(--panel); border: 1px solid var(--border); border-radius: 12px; box-shadow: var(--shadow); overflow: hidden; }
input { width: 100%; border: 0; border-bottom: 1px solid var(--border); padding: 14px 16px; font-size: 15px; background: none; outline: none; }
.items { max-height: 50vh; overflow: auto; padding: 6px; }
.pi { display: flex; align-items: center; gap: 10px; padding: 7px 10px; border-radius: 7px; cursor: pointer; }
.pi.on { background: var(--accentbg); color: var(--accenttx); }
.t { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.g { margin-left: auto; font-size: 11.5px; color: var(--faint); white-space: nowrap; }
</style>
