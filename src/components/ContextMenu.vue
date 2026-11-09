<script setup lang="ts">
import { nextTick, ref, watch } from 'vue'
import { S, type MenuItem } from '../store'

const el = ref<HTMLElement | null>(null)
const pos = ref({ left: 0, top: 0 })

watch(
  () => S.menu,
  async (m) => {
    if (!m) return
    pos.value = { left: m.x, top: m.y }
    await nextTick()
    const r = el.value?.getBoundingClientRect()
    if (!r) return
    // Right-aligned menus (minWidth -1) open leftwards from the anchor.
    let left = m.minWidth === -1 ? m.x - r.width : m.x
    let top = m.y
    if (left + r.width > innerWidth - 8) left = innerWidth - r.width - 8
    if (top + r.height > innerHeight - 8) top = Math.max(8, innerHeight - r.height - 8)
    pos.value = { left: Math.max(8, left), top }
  },
)

function run(it: MenuItem) {
  if (it.disabled || it.header || it.sep) return
  S.menu = null
  it.on?.()
}
</script>

<template>
  <div v-if="S.menu" class="shade" @mousedown.self="S.menu = null" @contextmenu.prevent="S.menu = null">
    <div ref="el" class="menu" :style="{ left: pos.left + 'px', top: pos.top + 'px', minWidth: (S.menu.minWidth ?? 0) > 0 ? S.menu.minWidth + 'px' : undefined }">
      <template v-for="(it, i) in S.menu.items" :key="i">
        <div v-if="it.sep" class="sp" />
        <div v-else-if="it.header" class="mh">{{ it.label }}</div>
        <div v-else class="mi" :class="{ danger: it.danger, dis: it.disabled, on: it.checked }" @click="run(it)">
          <span class="ic">{{ it.checked ? '✓' : (it.icon ?? '') }}</span>
          <span>{{ it.label }}</span>
          <span v-if="it.hint" class="hint">{{ it.hint }}</span>
        </div>
      </template>
    </div>
  </div>
</template>

<style scoped>
.shade { position: fixed; inset: 0; z-index: 40; }
.menu { position: fixed; min-width: 220px; max-width: 460px; max-height: 70vh; overflow: auto; background: var(--panel); border: 1px solid var(--border); border-radius: 9px; box-shadow: var(--shadow); padding: 5px; }
.mi { display: flex; align-items: center; gap: 8px; padding: 6px 10px; border-radius: 6px; cursor: pointer; white-space: nowrap; }
.mi:hover { background: var(--hover); }
.mi.on { color: var(--accenttx); }
.mi.danger { color: var(--danger); }
.mi.dis { color: var(--faint); cursor: default; }
.mi.dis:hover { background: none; }
.ic { width: 16px; text-align: center; flex: none; }
.hint { margin-left: auto; padding-left: 16px; font-size: 11.5px; color: var(--faint); }
.mh { padding: 6px 10px 2px; font-size: 11.5px; color: var(--faint); }
.sp { border-top: 1px solid var(--border); margin: 4px 0; }
</style>
