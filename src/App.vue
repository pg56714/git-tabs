<script setup lang="ts">
import { computed, onMounted, watchEffect } from 'vue'
import { onRepoChanged } from './api'
import { S, R, init, startTimers, onRepoChangedOnDisk, switchTab, moveSel, moveCommit, toggleSeen, refresh, closeModal } from './store'
import { stageToggle } from './actions'
import TabBar from './components/TabBar.vue'
import Toolbar from './components/Toolbar.vue'
import Sidebar from './components/Sidebar.vue'
import ChangesView from './components/ChangesView.vue'
import HistoryView from './components/HistoryView.vue'
import RangeView from './components/RangeView.vue'
import ReflogView from './components/ReflogView.vue'
import LogPanel from './components/LogPanel.vue'
import ContextMenu from './components/ContextMenu.vue'
import Toasts from './components/Toasts.vue'
import Palette from './components/Palette.vue'
import ModalHost from './components/ModalHost.vue'
import Welcome from './components/Welcome.vue'

const repo = computed(() => R())
const dark = window.matchMedia('(prefers-color-scheme: dark)')
const sysDark = { v: dark.matches }

function applyTheme() {
  const t = S.cfg.theme === 'system' ? (sysDark.v ? 'dark' : 'light') : S.cfg.theme
  document.documentElement.dataset.theme = t
}
watchEffect(applyTheme)
dark.addEventListener('change', (e) => {
  sysDark.v = e.matches
  applyTheme()
})

function typing() {
  const el = document.activeElement as HTMLElement | null
  return !!el && (/INPUT|TEXTAREA|SELECT/.test(el.tagName) || el.isContentEditable)
}

function onKey(e: KeyboardEvent) {
  if (e.key === 'Escape') {
    if (S.menu) S.menu = null
    else if (S.palette) S.palette = false
    else if (S.modal) closeModal(null)
    // Leaving a text box hands the keys back to the shortcuts.
    else if (typing()) (document.activeElement as HTMLElement).blur()
    return
  }
  const ctrl = e.ctrlKey || e.metaKey
  if (ctrl && e.key.toLowerCase() === 'k') {
    e.preventDefault()
    S.palette = !S.palette
    return
  }
  if (ctrl && /^[1-9]$/.test(e.key)) {
    e.preventDefault()
    switchTab(e.key === '9' ? S.repos.length - 1 : +e.key - 1)
    return
  }
  if (ctrl && e.key === 'Tab') {
    e.preventDefault()
    const n = S.repos.length
    if (n) switchTab((S.active + (e.shiftKey ? n - 1 : 1)) % n)
    return
  }
  if (e.key === 'F5' || (ctrl && e.key.toLowerCase() === 'r')) {
    e.preventDefault()
    if (repo.value) refresh(repo.value)
    return
  }
  if (S.modal || S.palette || S.menu || typing() || ctrl || e.altKey) return
  const r = repo.value
  if (!r) return
  const review = r.view === 'changes' || r.view === 'range'
  const k = e.key
  if (k === 'ArrowDown' || k === 'ArrowUp') {
    const d = k === 'ArrowDown' ? 1 : -1
    if (review) moveSel(r, d)
    else if (r.view === 'history') moveCommit(r, d)
    else return
  } else if (k === 'ArrowRight' || k === 'ArrowLeft') window.dispatchEvent(new CustomEvent('git-tabs:hunk', { detail: k === 'ArrowRight' ? 1 : -1 }))
  // Enter would also "click" whatever button kept focus, so it is taken here first.
  else if (k === 'Enter' && review) toggleSeen(r)
  else if ((k === 's' || k === 'S') && r.view === 'changes') stageToggle(r)
  else return
  e.preventDefault()
  e.stopPropagation()
}

onMounted(async () => {
  window.addEventListener('keydown', onKey, true)
  window.addEventListener('focus', () => repo.value && refresh(repo.value))
  await onRepoChanged(onRepoChangedOnDisk)
  await init()
  startTimers()
})
</script>

<template>
  <div class="app" v-if="S.ready">
    <TabBar />
    <template v-if="repo">
      <Toolbar :r="repo" />
      <div class="main">
        <Sidebar :r="repo" />
        <section class="content">
          <div v-if="repo.error" class="banner err">
            <span>{{ repo.error }}</span><span class="grow" />
            <button class="btn sm" @click="refresh(repo)">重試</button>
          </div>
          <ChangesView v-else-if="repo.view === 'changes'" :r="repo" />
          <HistoryView v-else-if="repo.view === 'history'" :r="repo" />
          <RangeView v-else-if="repo.view === 'range'" :r="repo" />
          <ReflogView v-else :r="repo" />
        </section>
      </div>
    </template>
    <Welcome v-else />
    <LogPanel />
  </div>
  <ContextMenu />
  <Palette v-if="S.palette" />
  <ModalHost />
  <Toasts />
</template>

<style scoped>
.app { display: grid; grid-template-rows: 36px 42px minmax(0, 1fr) auto; height: 100vh; }
.app:has(> .welcome) { grid-template-rows: 36px minmax(0, 1fr) auto; }
.main { display: grid; grid-template-columns: 220px minmax(0, 1fr); min-height: 0; }
.content { min-width: 0; min-height: 0; display: flex; flex-direction: column; background: var(--panel); }
</style>
