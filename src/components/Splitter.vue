<script setup lang="ts">
// A drag handle between two panes. `side` says which pane's size the value holds:
// 'left'/'top' grows when dragged right/down, 'right'/'bottom' when dragged left/up.
const p = defineProps<{ modelValue: number; min: number; max: number; side: 'left' | 'right' | 'top' | 'bottom' }>()
const emit = defineEmits<{ 'update:modelValue': [v: number]; done: [] }>()
const vertical = p.side === 'top' || p.side === 'bottom'

function down(e: PointerEvent) {
  const el = e.currentTarget as HTMLElement
  el.setPointerCapture(e.pointerId)
  const start = vertical ? e.clientY : e.clientX
  const w0 = p.modelValue
  const sign = p.side === 'left' || p.side === 'top' ? 1 : -1
  const move = (ev: PointerEvent) => {
    const d = ((vertical ? ev.clientY : ev.clientX) - start) * sign
    emit('update:modelValue', Math.round(Math.max(p.min, Math.min(p.max, w0 + d))))
  }
  const up = () => {
    el.removeEventListener('pointermove', move)
    el.removeEventListener('pointerup', up)
    document.body.classList.remove('resizing', 'resizing-y')
    emit('done')
  }
  document.body.classList.add(vertical ? 'resizing-y' : 'resizing')
  el.addEventListener('pointermove', move)
  el.addEventListener('pointerup', up)
}
</script>

<template>
  <div class="split" :class="{ y: vertical }" title="拖曳調整大小" @pointerdown.prevent="down" />
</template>

<style scoped>
.split { width: 5px; cursor: col-resize; background: var(--border); position: relative; z-index: 2; touch-action: none; flex: none; }
.split.y { width: auto; height: 5px; cursor: row-resize; }
.split::after { content: ''; position: absolute; inset: 0 -3px; }
.split.y::after { inset: -3px 0; }
.split:hover { background: var(--accent); }
:global(body.resizing) { cursor: col-resize; user-select: none; }
:global(body.resizing-y) { cursor: row-resize; user-select: none; }
</style>
