<script setup lang="ts">
import { nextTick, ref, watch } from 'vue'
import { S, switchTab, closeRepo, moveTab, openMenu, menuAt, openModal, refresh } from '../store'
import { openFolder, openTerminal, copy } from '../actions'
import IdChip from './IdChip.vue'

const drag = ref<number | null>(null)
const bar = ref<HTMLElement | null>(null)

// A plain mouse wheel scrolls the tab row sideways (there is nothing to scroll vertically).
function onWheel(e: WheelEvent) {
  if (!bar.value) return
  bar.value.scrollLeft += Math.abs(e.deltaY) > Math.abs(e.deltaX) ? e.deltaY : e.deltaX
}

// Keep the active tab in view, e.g. after Ctrl+Tab or adding a repo.
watch(
  // Branch labels arrive after the first render and widen the tabs, so re-check then too.
  () => [S.active, S.repos.length, S.repos.map((r) => r.summary?.branch ?? '').join()],
  async () => {
    await nextTick()
    bar.value?.querySelector('.tab.on')?.scrollIntoView({ inline: 'nearest', block: 'nearest' })
  },
  { immediate: true },
)

function allTabs(e: MouseEvent) {
  menuAt(
    e.currentTarget as Element,
    S.repos.map((r, i) => ({ label: r.name, checked: i === S.active, hint: r.summary?.branch ?? '', on: () => switchTab(i) })),
    'right',
  )
}

function menu(e: MouseEvent, i: number) {
  const r = S.repos[i]
  openMenu(e, [
    { label: '重新整理', icon: '⟳', on: () => refresh(r) },
    { label: '在檔案總管開啟', icon: '📂', on: () => openFolder(r.path) },
    { label: '開終端機', icon: '⌨', on: () => openTerminal(r.path) },
    { label: '複製路徑', icon: '⧉', on: () => copy(r.path, '路徑') },
    { sep: true },
    { label: '關閉分頁', icon: '×', on: () => closeRepo(i) },
  ])
}

function drop(i: number) {
  if (drag.value !== null && drag.value !== i) moveTab(drag.value, i)
  drag.value = null
}
</script>

<template>
  <div class="tabbar">
  <div ref="bar" class="tabs" @wheel.prevent="onWheel">
    <div
      v-for="(r, i) in S.repos"
      :key="r.path"
      class="tab"
      :class="{ on: i === S.active, dragging: drag === i }"
      :title="r.path"
      draggable="true"
      @click="switchTab(i)"
      @auxclick.middle.prevent="closeRepo(i)"
      @contextmenu.prevent="menu($event, i)"
      @dragstart="drag = i"
      @dragover.prevent
      @drop="drop(i)"
      @dragend="drag = null"
    >
      <span v-if="r.busy" class="spin" :title="r.busy" />
      <span v-else-if="r.error" class="warn" :title="r.error">!</span>
      <span v-else-if="r.summary?.changes" class="dirty" :title="`${r.summary.changes} 個未提交的修改`" />
      <span class="name">{{ r.name }}</span>
      <IdChip :r="r" />
      <span class="sub" v-if="r.summary">
        {{ r.summary.branch ?? '(detached)' }}<span v-if="r.summary.ahead" class="up"> ↑{{ r.summary.ahead }}</span><span v-if="r.summary.behind" class="down"> ↓{{ r.summary.behind }}</span>
      </span>
      <button class="x" title="關閉分頁" @click.stop="closeRepo(i)">×</button>
    </div>
  </div>
  <div class="tab add" title="加入 repo" @click="openModal('addrepo')">＋</div>
  <div class="tab add" title="所有分頁" @click="allTabs">⌄</div>
  </div>
</template>

<style scoped>
.tabbar { display: flex; align-items: flex-end; background: var(--bg); padding-right: 4px; min-width: 0; }
.tabs { display: flex; align-items: flex-end; gap: 2px; padding: 0 6px; overflow-x: auto; scrollbar-width: none; min-width: 0; height: 100%; }
.tab { display: flex; align-items: center; gap: 7px; height: 30px; padding: 0 8px 0 12px; border-radius: 8px 8px 0 0; color: var(--muted); cursor: pointer; white-space: nowrap; user-select: none; max-width: 300px; flex: none; }
.tab:hover { background: var(--hover); }
.tab.on { background: var(--panel); color: var(--text); }
.tab.dragging { opacity: 0.5; }
.tab.add { padding: 0 12px; }
.name { overflow: hidden; text-overflow: ellipsis; }
.x { opacity: 0; border: 0; background: none; cursor: pointer; color: var(--faint); padding: 0 3px; border-radius: 4px; font-size: 14px; line-height: 1; }
.x:hover { background: var(--hover); color: var(--text); }
.tab:hover .x, .tab.on .x { opacity: 1; }
.dirty { width: 7px; height: 7px; border-radius: 50%; background: #e8a33a; flex: none; }
.warn { color: var(--danger); font-weight: 700; }
.spin { width: 10px; height: 10px; border: 2px solid var(--border); border-top-color: var(--accent); border-radius: 50%; animation: spin 0.7s linear infinite; flex: none; }
@keyframes spin { to { transform: rotate(360deg); } }
</style>
