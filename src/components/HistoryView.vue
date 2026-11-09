<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue'
import { useVirtualizer } from '@tanstack/vue-virtual'
import { loadLog, loadDetail, select, openMenu, type RepoState } from '../store'
import { layout, LANE_COLORS } from '../lib/graph'
import { ago, dateTime, short } from '../lib/format'
import { cherryPick, revert, resetTo, checkoutCommit, createTag, newBranch, copy, clearFileHistory } from '../actions'
import DiffView from './DiffView.vue'
import DiffHead from './DiffHead.vue'

defineOptions({ name: 'HistoryView' })
const p = defineProps<{ r: RepoState }>()
const ROW = 28
const LANE = 14
const list = ref<HTMLElement | null>(null)
const q = ref(p.r.logQuery)
const graph = computed(() => (p.r.logQuery || p.r.logPath ? null : layout(p.r.commits)))
const gw = computed(() => (graph.value ? Math.min(10, Math.max(1, ...graph.value.map((g) => g.width))) * LANE + 10 : 14))

const virt = useVirtualizer(
  computed(() => ({ count: p.r.commits.length, getScrollElement: () => list.value, estimateSize: () => ROW, overscan: 20 })),
)

let timer: ReturnType<typeof setTimeout> | undefined
watch(q, (v) => {
  clearTimeout(timer)
  timer = setTimeout(() => {
    p.r.logQuery = v
    loadLog(p.r, true)
  }, 300)
})
watch(
  () => p.r.path,
  () => (q.value = p.r.logQuery),
)

function onScroll() {
  const el = list.value
  if (el && !p.r.logDone && !p.r.logLoading && el.scrollTop + el.clientHeight > el.scrollHeight - ROW * 30) loadLog(p.r)
}

async function pick(sha: string) {
  p.r.selCommit = sha
  await loadDetail(p.r)
}

watch(
  () => p.r.selCommit,
  async (sha) => {
    const i = p.r.commits.findIndex((c) => c.sha === sha)
    if (i < 0 || !list.value) return
    await nextTick()
    const top = i * ROW
    const el = list.value
    if (top < el.scrollTop || top > el.scrollTop + el.clientHeight - ROW) virt.value.scrollToIndex(i, { align: 'center' })
  },
)

function onKey(e: KeyboardEvent) {
  if (e.key !== 'ArrowDown' && e.key !== 'ArrowUp') return
  e.preventDefault()
  const i = p.r.commits.findIndex((c) => c.sha === p.r.selCommit)
  const n = Math.max(0, Math.min(p.r.commits.length - 1, i + (e.key === 'ArrowDown' ? 1 : -1)))
  if (p.r.commits[n]) pick(p.r.commits[n].sha)
}

function menu(e: MouseEvent, sha: string) {
  const r = p.r
  pick(sha)
  openMenu(e, [
    { label: 'Checkout 這個 commit…', on: () => checkoutCommit(r, sha) },
    { label: '從這裡開新分支…', on: () => newBranch(r, sha) },
    { label: 'Cherry-pick 到目前分支', on: () => cherryPick(r, sha) },
    { label: 'Revert…', on: () => revert(r, sha) },
    { label: `Reset ${r.summary?.branch ?? 'HEAD'} 到這裡…`, danger: true, on: () => resetTo(r, sha) },
    { label: '加 Tag…', on: () => createTag(r, sha) },
    { sep: true },
    { label: '複製 SHA', on: () => copy(sha, short(sha)) },
    { label: '複製標題', on: () => copy(r.commits.find((c) => c.sha === sha)?.subject ?? '', '標題') },
  ])
}

const refClass = (k: string) => ({ head: 'head', branch: '', remote: 'remote', tag: 'tag', detached: 'detached' })[k] ?? ''
const d = computed(() => p.r.detail)
const body = computed(() => d.value?.message.split('\n').slice(1).join('\n').trim() ?? '')
const selFile = computed(() => d.value?.files.find((f) => f.path === p.r.sel?.path))

function path(g: ReturnType<typeof layout>[number]) {
  const x = (c: number) => 7 + c * LANE
  const y = (v: number) => (v * ROW) / 2
  return g.segs.map((s) => {
    const [x1, y1, x2, y2] = [x(s.x1), y(s.y1), x(s.x2), y(s.y2)]
    const dpath = x1 === x2 ? `M${x1} ${y1}L${x2} ${y2}` : `M${x1} ${y1}C${x1} ${(y1 + y2) / 2} ${x2} ${(y1 + y2) / 2} ${x2} ${y2}`
    return { d: dpath, c: LANE_COLORS[s.color % LANE_COLORS.length] }
  })
}
</script>

<template>
  <div class="cbar">
    <input v-model="q" class="field search" placeholder="搜尋 commit 訊息，或貼上 SHA" />
    <span v-if="r.logPath" class="pill">只看 <span class="mono">{{ r.logPath }}</span> <button class="x" title="看全部" @click="clearFileHistory(r)">×</button></span>
    <span class="sub">{{ r.commits.length }}{{ r.logDone ? '' : '+' }} 個 commit</span>
    <span v-if="r.logLoading" class="sub">載入中…</span>
  </div>
  <div class="hist">
    <div ref="list" class="clist" tabindex="0" @scroll="onScroll" @keydown="onKey">
      <div v-if="!r.commits.length && !r.logLoading" class="empty">{{ r.logQuery ? '找不到符合的 commit' : '還沒有任何 commit' }}</div>
      <div :style="{ height: virt.getTotalSize() + 'px', position: 'relative' }">
        <div
          v-for="it in virt.getVirtualItems()"
          :key="r.commits[it.index].sha"
          class="cr"
          :class="{ on: r.commits[it.index].sha === r.selCommit }"
          :style="{ transform: `translateY(${it.start}px)`, gridTemplateColumns: `${gw}px minmax(0,1fr) 120px 64px 92px` }"
          @click="pick(r.commits[it.index].sha)"
          @contextmenu.prevent="menu($event, r.commits[it.index].sha)"
        >
          <svg v-if="graph" :width="gw" :height="ROW" aria-hidden="true">
            <path v-for="(s, k) in path(graph[it.index])" :key="k" :d="s.d" :stroke="s.c" stroke-width="2" fill="none" />
            <circle :cx="7 + graph[it.index].col * LANE" :cy="ROW / 2" r="4.5" fill="var(--panel)" :stroke="LANE_COLORS[graph[it.index].color % LANE_COLORS.length]" stroke-width="2.5" />
          </svg>
          <span v-else class="dot">●</span>
          <span class="msg">
            <span v-for="x in r.commits[it.index].refs" :key="x.kind + x.name" class="ref" :class="refClass(x.kind)">{{ x.name }}</span>{{ r.commits[it.index].subject }}
          </span>
          <span class="meta">{{ r.commits[it.index].author }}</span>
          <span class="meta mono">{{ short(r.commits[it.index].sha) }}</span>
          <span class="meta" :title="dateTime(r.commits[it.index].time)">{{ ago(r.commits[it.index].time) }}</span>
        </div>
      </div>
    </div>
    <div class="cdet">
      <template v-if="d">
        <div class="cmeta">
          <h3>{{ d.message.split('\n')[0] }}</h3>
          <pre v-if="body" class="body">{{ body }}</pre>
          <div class="kv">
            <span>作者</span><b>{{ d.author }} &lt;{{ d.email }}&gt; · {{ dateTime(d.time) }}</b>
            <span>SHA</span><b class="mono">{{ d.sha.slice(0, 12) }} <a class="link" @click="copy(d.sha, short(d.sha))">複製</a></b>
            <span>上一個</span><b class="mono"><a v-for="pa in d.parents" :key="pa" class="link" style="margin-right: 8px" @click="pick(pa)">{{ short(pa) }}</a><span v-if="!d.parents.length">（第一個 commit）</span></b>
            <span v-if="d.refs.length">Refs</span><b v-if="d.refs.length"><span v-for="x in d.refs" :key="x.kind + x.name" class="ref" :class="refClass(x.kind)">{{ x.name }}</span></b>
          </div>
          <div class="acts">
            <button class="btn sm" @click="cherryPick(r, d.sha)">Cherry-pick</button>
            <button class="btn sm" @click="revert(r, d.sha)">Revert</button>
            <button class="btn sm" @click="checkoutCommit(r, d.sha)">Checkout</button>
            <button class="btn sm" @click="newBranch(r, d.sha)">開分支</button>
            <button class="btn sm" @click="createTag(r, d.sha)">加 Tag</button>
            <button class="btn sm" @click="resetTo(r, d.sha)">Reset 到這裡…</button>
          </div>
        </div>
        <div class="cfiles">
          <div v-for="f in d.files" :key="f.path" class="fr" :class="{ on: r.sel?.path === f.path }" :title="f.path" @click="select(r, 'c', f.path)">
            <span class="st" :class="f.status">{{ f.status }}</span><span class="nm mono">{{ f.path }}</span><span class="grow" />
            <span v-if="f.add !== null" class="add-n">+{{ f.add }}</span><span v-if="f.del !== null" class="del-n">−{{ f.del }}</span>
          </div>
          <div v-if="!d.files.length" class="sub" style="padding: 6px 12px">沒有檔案變更</div>
        </div>
        <template v-if="r.sel && selFile">
          <DiffHead :r="r" :diff="r.diff" list="c" :path="r.sel.path" :status="selFile.status" />
          <DiffView :diff="r.diffFor === 'c:' + r.sel.path ? r.diff : null" mode="ro" />
        </template>
      </template>
      <div v-else class="empty">選一個 commit</div>
    </div>
  </div>
</template>

<style scoped>
.search { max-width: 340px; }
.pill { display: inline-flex; align-items: center; gap: 4px; background: var(--accentbg); color: var(--accenttx); padding: 2px 4px 2px 10px; border-radius: 12px; font-size: 12px; }
.pill .x { border: 0; background: none; cursor: pointer; color: inherit; font-size: 14px; }
.hist { display: grid; grid-template-columns: minmax(0, 1fr) minmax(380px, 46%); flex: 1; min-height: 0; }
.clist { overflow: auto; border-right: 1px solid var(--border); outline: none; position: relative; }
.cr { position: absolute; left: 0; right: 0; top: 0; display: grid; align-items: center; height: 28px; padding-right: 10px; cursor: pointer; gap: 6px; }
.cr:hover { background: var(--hover); }
.cr.on { background: var(--accentbg); }
.dot { color: var(--accent); text-align: center; font-size: 10px; }
.msg { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.meta { color: var(--muted); font-size: 12px; overflow: hidden; white-space: nowrap; text-overflow: ellipsis; }
.cdet { display: flex; flex-direction: column; min-height: 0; min-width: 0; }
.cmeta { padding: 12px 14px; border-bottom: 1px solid var(--border); }
.cmeta h3 { margin: 0 0 6px; font-size: 14.5px; font-weight: 600; }
.body { white-space: pre-wrap; font: 12.5px/1.5 inherit; color: var(--muted); margin: 0 0 8px; max-height: 140px; overflow: auto; }
.kv { display: grid; grid-template-columns: 52px 1fr; gap: 2px 8px; font-size: 12px; color: var(--muted); }
.kv b { font-weight: 400; color: var(--text); }
.link { color: var(--accent); cursor: pointer; }
.acts { display: flex; gap: 6px; margin-top: 10px; flex-wrap: wrap; }
.cfiles { max-height: 28%; overflow: auto; border-bottom: 1px solid var(--border); padding: 4px 0; flex: none; }
.fr { display: flex; align-items: center; gap: 6px; padding: 2px 12px; cursor: pointer; white-space: nowrap; }
.fr:hover { background: var(--hover); }
.fr.on { background: var(--accentbg); color: var(--accenttx); }
.nm { font-size: 12px; overflow: hidden; text-overflow: ellipsis; }
.add-n { color: var(--ok); font-size: 12px; }
.del-n { color: var(--danger); font-size: 12px; }
</style>
