<script setup lang="ts">
import { computed } from 'vue'
import type { FileDiff } from '../types'
import { S, isSeen, toggleSeen, openMenu, type RepoState, type ListKind } from '../store'
import { stats } from '../lib/diff'
import { openFile, copy, fileHistory, blame, openFolder } from '../actions'

const p = defineProps<{ r: RepoState; diff: FileDiff | null; list: ListKind; path: string; status: string }>()
const st = computed(() => stats(p.diff))
const seen = computed(() => isSeen(p.r, p.list, p.path))
const canSeen = computed(() => p.list !== 'c')

function more(e: MouseEvent) {
  const r = p.r
  const dir = p.path.includes('/') ? p.path.slice(0, p.path.lastIndexOf('/')) : ''
  openMenu(e, [
    { label: '用編輯器開啟', icon: '✎', on: () => openFile(r, p.path) },
    { label: '在檔案總管顯示資料夾', icon: '📂', on: () => openFolder(`${r.path}/${dir}`.replace(/\//g, '\\')) },
    { label: '這個檔案的歷史', icon: '◷', on: () => fileHistory(r, p.path) },
    { label: 'Blame（逐行看是誰改的）', icon: '☰', on: () => blame(r, p.path, p.list === 'c' ? r.detail?.sha : undefined) },
    { label: '複製路徑', icon: '⧉', on: () => copy(p.path, '路徑') },
  ])
}
</script>

<template>
  <div class="dhead">
    <span class="st" :class="status">{{ status }}</span>
    <span class="path mono" :title="path"><span v-if="diff?.old_path" class="sub">{{ diff.old_path }} → </span>{{ path }}</span>
    <span class="add-n">+{{ st[0] }}</span><span class="del-n">−{{ st[1] }}</span>
    <span class="grow" />
    <slot />
    <label v-if="canSeen" class="btn sm" :class="{ on: seen }" title="看完勾起來，會跳到下一個沒看的檔案（V）">
      <input type="checkbox" :checked="seen" style="margin: 0" @click.prevent="toggleSeen(r)" /> 已看
    </label>
    <div class="seg">
      <button :class="{ on: !S.side }" @click="S.side = false">單欄</button>
      <button :class="{ on: S.side }" @click="S.side = true">並排</button>
    </div>
    <button class="btn sm" :class="{ on: S.wrap }" title="自動換行" @click="S.wrap = !S.wrap">↩</button>
    <button class="btn sm" title="更多" @click="more">⋯</button>
  </div>
</template>

<style scoped>
.dhead { display: flex; align-items: center; gap: 8px; padding: 6px 10px; border-bottom: 1px solid var(--border); min-height: 41px; flex-wrap: wrap; }
.path { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; min-width: 80px; flex: 0 1 auto; }
.add-n { color: var(--ok); font-size: 12px; }
.del-n { color: var(--danger); font-size: 12px; }
</style>
