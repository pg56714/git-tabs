<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue'
import { S } from '../store'
import { baseName } from '../lib/format'

const last = computed(() => S.log[S.log.length - 1])
const box = ref<HTMLElement | null>(null)
const open = ref<number | null>(null)
const time = (ms: number) => new Date(ms).toTimeString().slice(0, 8)

watch(
  () => S.log.length,
  async () => {
    await nextTick()
    box.value?.scrollTo({ top: box.value.scrollHeight })
  },
)
</script>

<template>
  <div class="log">
    <div class="bar" @click="S.logOpen = !S.logOpen">
      {{ S.logOpen ? '▾' : '▸' }} 指令紀錄
      <template v-if="last">
        <span class="mono cmd">{{ last.cmd }}</span>
        <span :class="last.ok ? 'ok' : 'err'">{{ last.ok ? '✓' : '✕' }} {{ (last.ms / 1000).toFixed(1) }}s</span>
      </template>
      <span v-else>還沒有執行任何會改變東西的指令</span>
      <span class="grow" /><span>{{ S.log.length }} 筆</span><span v-if="S.gitVersion" class="sub">· {{ S.gitVersion }}</span>
    </div>
    <div v-if="S.logOpen" ref="box" class="list">
      <div v-for="l in S.log" :key="l.id" class="item" @click="open = open === l.id ? null : l.id">
        <span class="sub">{{ time(l.at) }}</span>
        <span :class="l.ok ? 'ok' : 'err'">{{ l.ok ? '✓' : '✕' }}</span>
        <span v-if="l.repo" class="sub">[{{ baseName(l.repo) }}]</span>
        {{ l.cmd }} <span class="sub">({{ (l.ms / 1000).toFixed(1) }}s)</span>
        <pre v-if="open === l.id || (!l.ok && l.output)" class="out">{{ l.output || '（沒有輸出）' }}</pre>
      </div>
      <div v-if="!S.log.length" class="sub">（空）</div>
    </div>
  </div>
</template>

<style scoped>
.log { background: var(--panel2); border-top: 1px solid var(--border); font-size: 12px; }
.bar { display: flex; align-items: center; gap: 8px; padding: 3px 10px; color: var(--muted); cursor: pointer; height: 26px; white-space: nowrap; overflow: hidden; }
.cmd { color: var(--text); overflow: hidden; text-overflow: ellipsis; }
.list { max-height: 200px; overflow: auto; padding: 4px 10px 8px; font: 12px/1.6 var(--mono); }
.item { cursor: pointer; }
.out { margin: 2px 0 6px 24px; white-space: pre-wrap; color: var(--muted); font: inherit; }
</style>
