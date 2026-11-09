<script setup lang="ts">
import { computed } from 'vue'
import { S, select, isSeen, loadRange, loadDiff, rangeBase, rangeCandidates, setView, loadDetail, type RepoState } from '../store'
import { mergeInto, switchBranch } from '../actions'
import { short, ago } from '../lib/format'
import FileTree from './FileTree.vue'
import DiffView from './DiffView.vue'
import DiffHead from './DiffHead.vue'

defineOptions({ name: 'RangeView' })
const p = defineProps<{ r: RepoState }>()
const base = computed(() => rangeBase(p.r))
const cands = computed(() => rangeCandidates(p.r))
const files = computed(() => p.r.rangeFiles.map((f) => ({ path: f.path, status: f.status, key: 'r:' + f.path })))
const seenN = computed(() => p.r.rangeFiles.filter((f) => isSeen(p.r, 'r', f.path)).length)
const selFile = computed(() => p.r.rangeFiles.find((f) => f.path === p.r.sel?.path))

async function pickBranch(e: Event) {
  p.r.rangeHead = (e.target as HTMLSelectElement).value
  p.r.sel = null
  await loadRange(p.r)
  await loadDiff(p.r)
}

async function openCommit(sha: string) {
  await setView(p.r, 'history')
  p.r.selCommit = sha
  await loadDetail(p.r)
}

const isCurrent = computed(() => p.r.summary?.branch === p.r.rangeHead)
</script>

<template>
  <div v-if="!base" class="empty"><div><span class="big">⇄</span>找不到 main / master 分支可以比較</div></div>
  <div v-else-if="!cands.length" class="empty"><div><span class="big">⇄</span>{{ r.name }} 只有 {{ base }}，沒有其他分支可以比較</div></div>
  <template v-else>
    <div class="cbar">
      <select class="field" style="width: auto" :value="r.rangeHead ?? ''" @change="pickBranch">
        <option v-for="b in cands" :key="b" :value="b">{{ b }}</option>
      </select>
      <span class="sub">vs {{ base }} · {{ r.rangeCommits.length }} 個 commit · {{ r.rangeFiles.length }} 個檔案 · 已看 {{ seenN }}</span>
      <span class="grow" />
      <button v-if="!isCurrent && r.rangeHead" class="btn sm" @click="switchBranch(r, r.rangeHead!)">切換到 {{ r.rangeHead }}</button>
      <button v-if="r.summary?.branch === base && r.rangeHead && r.rangeCommits.length" class="btn sm primary" @click="mergeInto(r, r.rangeHead!)">把 {{ r.rangeHead }} merge 進 {{ base }}</button>
      <span v-else-if="r.rangeHead && r.rangeCommits.length && r.summary?.branch !== base" class="sub" title="要 merge 請先切到 main">（切到 {{ base }} 之後可以直接 merge）</span>
    </div>
    <div v-if="!r.rangeFiles.length" class="empty"><div>{{ r.rangeHead }} 跟 {{ base }} 沒有差異</div></div>
    <div v-else class="review">
      <div class="fp">
        <div class="prog"><div :style="{ width: Math.round((seenN / Math.max(1, r.rangeFiles.length)) * 100) + '%' }" /></div>
        <div class="scroll">
          <FileTree :files="files" :selected="r.sel?.list === 'r' ? r.sel.path : null" :seen="(x) => isSeen(r, 'r', x)" @select="(x) => select(r, 'r', x)" />
        </div>
        <div class="commits">
          <div class="fsec">這個分支的 commit</div>
          <div v-for="c in r.rangeCommits" :key="c.sha" class="cm" :title="c.subject" @click="openCommit(c.sha)">
            <span class="mono sub">{{ short(c.sha) }}</span> <span class="nm">{{ c.subject }}</span><span class="grow" /><span class="sub">{{ ago(c.time) }}</span>
          </div>
        </div>
      </div>
      <div class="dv">
        <template v-if="r.sel && selFile">
          <DiffHead :r="r" :diff="r.diff" list="r" :path="r.sel.path" :status="selFile.status" />
          <DiffView :diff="r.diffFor === 'r:' + r.sel.path ? r.diff : null" mode="ro" />
        </template>
        <div class="hint"><span><kbd>↑</kbd>/<kbd>↓</kbd> 上/下一個檔案</span><span><kbd>←</kbd>/<kbd>→</kbd> 上/下一段改動</span><span><kbd>Enter</kbd> 已看</span><span class="grow" /><span v-if="S.side">並排</span></div>
      </div>
    </div>
  </template>
</template>

<style scoped>
.review { display: grid; grid-template-columns: 300px minmax(0, 1fr); flex: 1; min-height: 0; }
.fp { border-right: 1px solid var(--border); display: flex; flex-direction: column; min-height: 0; }
.prog { height: 3px; background: var(--border); margin: 8px 10px 6px; border-radius: 2px; }
.prog div { height: 3px; background: var(--accent); border-radius: 2px; transition: width 0.2s; }
.scroll { flex: 1; overflow: auto; }
.commits { border-top: 1px solid var(--border); max-height: 35%; overflow: auto; padding-bottom: 6px; }
.fsec { padding: 8px 10px 4px; font-size: 12px; color: var(--muted); }
.cm { display: flex; gap: 6px; align-items: center; padding: 2px 10px; cursor: pointer; white-space: nowrap; }
.cm:hover { background: var(--hover); }
.nm { overflow: hidden; text-overflow: ellipsis; }
.dv { display: flex; flex-direction: column; min-width: 0; min-height: 0; }
.hint { font-size: 11.5px; color: var(--faint); padding: 4px 10px; border-top: 1px solid var(--border); display: flex; gap: 12px; margin-top: auto; }
</style>
