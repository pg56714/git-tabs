<script setup lang="ts">
import { ref } from 'vue'
import { S, switchTab, closeRepo, moveTab, openMenu, openModal, refresh } from '../store'
import { openFolder, openTerminal, copy } from '../actions'
import IdChip from './IdChip.vue'

const drag = ref<number | null>(null)

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
  <div class="tabs">
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
    <div class="tab add" title="加入 repo" @click="openModal('addrepo')">＋</div>
  </div>
</template>

<style scoped>
.tabs { display: flex; align-items: flex-end; gap: 2px; padding: 0 6px; background: var(--bg); overflow-x: auto; scrollbar-width: none; }
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
