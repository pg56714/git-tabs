<script setup lang="ts">
import { computed, ref } from 'vue'
import { treeRows, type TreeFile } from '../lib/tree'

const p = defineProps<{ files: TreeFile[]; selected: string | null; seen?: (path: string) => boolean; flat?: boolean }>()
const emit = defineEmits<{ select: [path: string]; menu: [e: MouseEvent, path: string]; dbl: [path: string] }>()
const collapsed = ref(new Set<string>())
const rows = computed(() =>
  p.flat ? p.files.map((f) => ({ kind: 'file' as const, file: f, name: f.path, depth: 0 })) : treeRows(p.files, collapsed.value),
)

function toggle(dir: string) {
  const s = new Set(collapsed.value)
  if (s.has(dir)) s.delete(dir)
  else s.add(dir)
  collapsed.value = s
}
</script>

<template>
  <div class="tree">
    <template v-for="row in rows" :key="row.kind === 'dir' ? 'd:' + row.dir : row.file.key">
      <div v-if="row.kind === 'dir'" class="tr dir" :style="{ paddingLeft: 10 + row.depth * 14 + 'px' }" @click="toggle(row.dir)">
        <span class="car">{{ collapsed.has(row.dir) ? '▸' : '▾' }}</span><span class="nm">{{ row.name }}</span>
      </div>
      <div
        v-else
        class="tr"
        :class="{ on: selected === row.file.path, seen: seen?.(row.file.path) }"
        :style="{ paddingLeft: 10 + row.depth * 14 + 'px' }"
        :title="row.file.path"
        :data-path="row.file.path"
        @click="emit('select', row.file.path)"
        @dblclick="emit('dbl', row.file.path)"
        @contextmenu.prevent="emit('menu', $event, row.file.path)"
      >
        <span class="st" :class="row.file.status">{{ row.file.status }}</span>
        <span class="nm">{{ row.name }}</span>
        <span v-if="seen?.(row.file.path)" class="ck">✓</span>
      </div>
    </template>
  </div>
</template>

<style scoped>
.tr { display: flex; align-items: center; gap: 6px; padding: 2px 10px; cursor: pointer; white-space: nowrap; border-radius: 5px; margin: 0 6px; min-height: 24px; }
.tr:hover { background: var(--hover); }
.tr.on { background: var(--accentbg); color: var(--accenttx); }
.tr.dir { color: var(--muted); }
.car { width: 10px; font-size: 10px; flex: none; }
.nm { overflow: hidden; text-overflow: ellipsis; }
.tr.seen .nm { color: var(--faint); text-decoration: line-through; }
.ck { margin-left: auto; color: var(--ok); font-size: 12px; }
</style>
