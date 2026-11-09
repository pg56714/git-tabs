<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { call } from '../api'
import { closeModal } from '../store'
import type { BlameLine } from '../types'
import { ago, dateTime, short } from '../lib/format'

const p = defineProps<{ repo: string; path: string; rev?: string }>()
const lines = ref<BlameLine[]>([])
const err = ref('')
const loading = ref(true)

onMounted(async () => {
  try {
    lines.value = await call<BlameLine[]>('blame', { repo: p.repo, path: p.path, rev: p.rev })
  } catch (e) {
    err.value = String(e).replace(/^Error: /, '')
  }
  loading.value = false
})

// Only label the first line of each run from the same commit.
const rows = computed(() => lines.value.map((l, i) => ({ ...l, first: i === 0 || lines.value[i - 1].sha !== l.sha })))
const shade = (sha: string) => (parseInt(sha.slice(0, 2), 16) % 2 ? 'a' : 'b')
</script>

<template>
  <div class="modal wide blame">
    <h2>Blame：<span class="mono">{{ path }}</span><span v-if="rev" class="sub"> @ {{ short(rev) }}</span></h2>
    <div class="body">
      <div v-if="loading" class="sub">載入中…</div>
      <div v-else-if="err" class="err">{{ err }}</div>
      <div v-else class="code">
        <div v-for="l in rows" :key="l.line" class="bl" :class="[shade(l.sha), { first: l.first }]">
          <span class="who" :title="`${l.summary}\n${dateTime(l.time)}`"><template v-if="l.first"><span class="mono">{{ short(l.sha) }}</span> {{ l.author }} · {{ ago(l.time) }}</template></span>
          <span class="n">{{ l.line }}</span>
          <span class="t">{{ l.text }}</span>
        </div>
      </div>
    </div>
    <div class="foot"><button class="btn" @click="closeModal(null)">關閉</button></div>
  </div>
</template>

<style scoped>
.blame { width: min(1100px, 96vw); }
.code { font: 12.5px/1.6 var(--mono); border: 1px solid var(--border); border-radius: 8px; overflow: auto; }
.bl { display: grid; grid-template-columns: 300px 48px minmax(0, 1fr); }
.bl.a { background: var(--panel2); }
.bl.first { border-top: 1px solid var(--border); }
.who { color: var(--muted); padding: 0 8px; overflow: hidden; white-space: nowrap; text-overflow: ellipsis; font-family: 'Segoe UI', system-ui, sans-serif; font-size: 12px; }
.n { color: var(--faint); text-align: right; padding-right: 8px; border-right: 1px solid var(--border); }
.t { white-space: pre; padding-left: 10px; }
</style>
