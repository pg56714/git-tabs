<script setup lang="ts">
import { S, dismissToast } from '../store'
</script>

<template>
  <div class="toasts">
    <div v-for="t in S.toasts" :key="t.id" class="t" :class="t.kind" @click="dismissToast(t.id)">
      <div>{{ t.kind === 'err' ? '✕ ' : t.kind === 'ok' ? '✓ ' : '' }}{{ t.text }}</div>
      <pre v-if="t.detail" class="d">{{ t.detail }}</pre>
    </div>
  </div>
</template>

<style scoped>
.toasts { position: fixed; right: 16px; bottom: 40px; display: flex; flex-direction: column; gap: 8px; z-index: 60; max-width: min(520px, 90vw); }
.t { background: var(--text); color: var(--panel); padding: 8px 14px; border-radius: 8px; box-shadow: var(--shadow); animation: in 0.18s ease-out; cursor: pointer; }
.t.err { background: var(--danger); color: #fff; }
.d { margin: 6px 0 0; white-space: pre-wrap; word-break: break-word; font: 12px/1.5 var(--mono); opacity: 0.92; max-height: 160px; overflow: auto; }
@keyframes in { from { transform: translateY(8px); opacity: 0; } }
</style>
