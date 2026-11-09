<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useVirtualizer } from '@tanstack/vue-virtual'
import { S, savePrefs, loadLog, loadDetail, select, openMenu, type RepoState } from '../store'
import { layout, LANE_COLORS } from '../lib/graph'
import { ago, dateTime, short } from '../lib/format'
import { cherryPick, revert, resetTo, checkoutCommit, createTag, newBranch, copy, clearFileHistory } from '../actions'
import DiffView from './DiffView.vue'
import DiffHead from './DiffHead.vue'
import FileTree from './FileTree.vue'
import Splitter from './Splitter.vue'

defineOptions({ name: 'HistoryView' })
const p = defineProps<{ r: RepoState }>()
const ROW = 28
const LANE = 16
// Center of the first lane; the same margin is kept on the right of the last lane.
const PAD = 9
// Room after the last lane: the dot sits close to the message, like Fork.
const TAIL = 7
const list = ref<HTMLElement | null>(null)
const q = ref(p.r.logQuery)
const graph = computed(() => (p.r.logQuery || p.r.logPath ? null : layout(p.r.commits)))
const gw = computed(() => (graph.value ? (Math.min(10, Math.max(1, ...graph.value.map((g) => g.width))) - 1) * LANE + PAD + TAIL : 16))

// The message column always gets the room; author and date drop out when the list is narrow.
const listWidth = ref(1000)
const showAuthor = computed(() => listWidth.value >= 720)
const showDate = computed(() => listWidth.value >= 520)
const cols = computed(() => [`${gw.value}px`, 'minmax(120px,1fr)', showAuthor.value ? '110px' : '', '64px', showDate.value ? '88px' : ''].filter(Boolean).join(' '))
let ro: ResizeObserver | undefined
onMounted(() => {
  ro = new ResizeObserver(([e]) => (listWidth.value = e.contentRect.width))
  if (list.value) ro.observe(list.value)
})
onBeforeUnmount(() => ro?.disconnect())

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
const treeFiles = computed(() => (d.value?.files ?? []).map((f) => ({ path: f.path, status: f.status, key: f.path })))
const initials = (name: string) => name.trim().split(/\s+/).map((w) => w[0]).join('').slice(0, 2).toUpperCase()

/** Like Fork: a Commit tab (details and file list) and a Changes tab (files + diff). */
function openFile(path: string) {
  select(p.r, 'c', path)
  S.histTab = 'changes'
}

// With Windows display scaling (125%, 150%…) a 2px line at x=9 falls between screen pixels
// and blurs unevenly. Snap line width and lane centers to whole device pixels.
const dpr = window.devicePixelRatio || 1
const lineDev = Math.max(2, Math.round(2 * dpr))
const lineW = lineDev / dpr
const laneX = (c: number) => (Math.round((PAD + c * LANE) * dpr) + (lineDev % 2 ? 0.5 : 0)) / dpr
const color = (n: number) => LANE_COLORS[n % LANE_COLORS.length]

/** One SVG for all visible rows, so vertical lines run through without a seam at every row. */
const visible = computed(() => virt.value.getVirtualItems())
const graphPaths = computed(() => {
  if (!graph.value) return []
  const out: { d: string; c: string }[] = []
  visible.value.forEach((it, k) => {
    const g = graph.value![it.index]
    const top = k * ROW
    for (const sg of g.segs) {
      const [x1, x2] = [laneX(sg.x1), laneX(sg.x2)]
      const [y1, y2] = [top + (sg.y1 * ROW) / 2, top + (sg.y2 * ROW) / 2]
      const mid = (y1 + y2) / 2
      out.push({ d: x1 === x2 ? `M${x1} ${y1}V${y2}` : `M${x1} ${y1}C${x1} ${mid} ${x2} ${mid} ${x2} ${y2}`, c: color(sg.color) })
    }
  })
  return out
})
const graphDots = computed(() =>
  graph.value ? visible.value.map((it, k) => ({ x: laneX(graph.value![it.index].col), y: k * ROW + ROW / 2, c: color(graph.value![it.index].color) })) : [],
)
</script>

<template>
  <div class="cbar">
    <input v-model="q" class="field search" placeholder="搜尋 commit 訊息，或貼上 SHA" />
    <span v-if="r.logPath" class="pill">只看 <span class="mono">{{ r.logPath }}</span> <button class="x" title="看全部" @click="clearFileHistory(r)">×</button></span>
    <span class="sub">{{ r.commits.length }}{{ r.logDone ? '' : '+' }} 個 commit</span>
    <span v-if="r.logLoading" class="sub">載入中…</span>
  </div>
  <div class="hist">
    <div ref="list" class="clist" :style="{ height: S.sizes.histTop + 'px' }" @scroll="onScroll">
      <div v-if="!r.commits.length && !r.logLoading" class="empty">{{ r.logQuery ? '找不到符合的 commit' : '還沒有任何 commit' }}</div>
      <div :style="{ height: virt.getTotalSize() + 'px', position: 'relative' }">
        <svg v-if="graph && visible.length" class="gsvg" :width="gw" :height="visible.length * ROW" :style="{ transform: `translateY(${visible[0].start}px)` }" aria-hidden="true">
          <path v-for="(p, k) in graphPaths" :key="k" :d="p.d" :stroke="p.c" :stroke-width="lineW" fill="none" />
          <circle v-for="(c, k) in graphDots" :key="'d' + k" :cx="c.x" :cy="c.y" r="4.5" :fill="c.c" stroke="var(--panel)" stroke-width="1.5" />
        </svg>
        <div
          v-for="it in virt.getVirtualItems()"
          :key="r.commits[it.index].sha"
          class="cr"
          :class="{ on: r.commits[it.index].sha === r.selCommit }"
          :style="{ transform: `translateY(${it.start}px)`, gridTemplateColumns: cols }"
          @click="pick(r.commits[it.index].sha)"
          @contextmenu.prevent="menu($event, r.commits[it.index].sha)"
        >
          <span v-if="graph" />
          <span v-else class="dot">●</span>
          <span class="msg">
            <span v-for="x in r.commits[it.index].refs" :key="x.kind + x.name" class="ref" :class="refClass(x.kind)">{{ x.name }}</span>{{ r.commits[it.index].subject }}
          </span>
          <span v-if="showAuthor" class="meta">{{ r.commits[it.index].author }}</span>
          <span class="meta mono">{{ short(r.commits[it.index].sha) }}</span>
          <span v-if="showDate" class="meta" :title="dateTime(r.commits[it.index].time)">{{ ago(r.commits[it.index].time) }}</span>
        </div>
      </div>
    </div>
    <Splitter v-model="S.sizes.histTop" :min="120" :max="900" side="top" @done="savePrefs" />
    <div class="cdet">
      <div class="dtabs">
        <button :class="{ on: S.histTab === 'commit' }" @click="S.histTab = 'commit'">Commit</button>
        <button :class="{ on: S.histTab === 'changes' }" @click="S.histTab = 'changes'">Changes <span v-if="d" class="cnt">{{ d.files.length }}</span></button>
      </div>
      <template v-if="d">
        <div v-if="S.histTab === 'commit'" class="ctab">
          <div class="who">
            <div class="avatar">{{ initials(d.author) }}</div>
            <div>
              <div class="author">{{ d.author }} <span class="sub">{{ d.email }}</span></div>
              <div class="sub">{{ dateTime(d.time) }}<template v-if="d.committer !== d.author"> · commit 者 {{ d.committer }}</template></div>
            </div>
          </div>
          <div class="kv">
            <span v-if="d.refs.length">REFS</span><b v-if="d.refs.length"><span v-for="x in d.refs" :key="x.kind + x.name" class="ref" :class="refClass(x.kind)">{{ x.name }}</span></b>
            <span>SHA</span><b class="mono">{{ d.sha }} <a class="link" @click="copy(d.sha, short(d.sha))">複製</a></b>
            <span>PARENTS</span><b class="mono"><a v-for="pa in d.parents" :key="pa" class="link" style="margin-right: 8px" @click="pick(pa)">{{ short(pa) }}</a><span v-if="!d.parents.length">（第一個 commit）</span></b>
          </div>
          <h3>{{ d.message.split('\n')[0] }}</h3>
          <pre v-if="body" class="body">{{ body }}</pre>
          <div class="acts">
            <button class="btn sm" @click="cherryPick(r, d.sha)">Cherry-pick</button>
            <button class="btn sm" @click="revert(r, d.sha)">Revert</button>
            <button class="btn sm" @click="checkoutCommit(r, d.sha)">Checkout</button>
            <button class="btn sm" @click="newBranch(r, d.sha)">開分支</button>
            <button class="btn sm" @click="createTag(r, d.sha)">加 Tag</button>
            <button class="btn sm" @click="resetTo(r, d.sha)">Reset 到這裡…</button>
          </div>
          <div class="flist">
            <div v-for="f in d.files" :key="f.path" class="fr" :title="f.path" @click="openFile(f.path)">
              <span class="st" :class="f.status">{{ f.status }}</span><span class="nm mono">{{ f.path }}</span><span class="grow" />
              <span v-if="f.add !== null" class="add-n">+{{ f.add }}</span><span v-if="f.del !== null" class="del-n">−{{ f.del }}</span>
            </div>
            <div v-if="!d.files.length" class="sub" style="padding: 6px 12px">沒有檔案變更</div>
          </div>
        </div>
        <div v-else class="chtab" :style="{ gridTemplateColumns: `${S.sizes.histFiles}px 5px minmax(0, 1fr)` }">
          <div class="ftree">
            <FileTree :files="treeFiles" :selected="r.sel?.list === 'c' ? r.sel.path : null" @select="(x) => select(r, 'c', x)" />
          </div>
          <Splitter v-model="S.sizes.histFiles" :min="160" :max="700" side="left" @done="savePrefs" />
          <div class="dv">
            <template v-if="r.sel && selFile">
              <DiffHead :r="r" :diff="r.diff" list="c" :path="r.sel.path" :status="selFile.status" />
              <DiffView :diff="r.diffFor === 'c:' + r.sel.path ? r.diff : null" mode="ro" />
            </template>
            <div v-else class="empty">選一個檔案</div>
          </div>
        </div>
      </template>
      <div v-else class="empty">選一個 commit</div>
    </div>
  </div>
</template>

<style scoped>
.search { max-width: 340px; }
.pill { display: inline-flex; align-items: center; gap: 4px; background: var(--accentbg); color: var(--accenttx); padding: 2px 4px 2px 10px; border-radius: 12px; font-size: 12px; }
.pill .x { border: 0; background: none; cursor: pointer; color: inherit; font-size: 14px; }
.hist { display: flex; flex-direction: column; flex: 1; min-height: 0; }
.clist { overflow: auto; outline: none; position: relative; flex: none; max-height: calc(100% - 120px); }
.gsvg { position: absolute; left: 0; top: 0; z-index: 1; pointer-events: none; overflow: visible; }
.cr { position: absolute; left: 0; right: 0; top: 0; display: grid; align-items: center; height: 28px; padding-right: 10px; cursor: pointer; gap: 6px; }
.cr:hover { background: var(--hover); }
.cr.on { background: var(--accentbg); }
.dot { color: var(--accent); text-align: center; font-size: 10px; }
.msg { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.meta { color: var(--muted); font-size: 12px; overflow: hidden; white-space: nowrap; text-overflow: ellipsis; }
.cdet { display: flex; flex-direction: column; min-height: 0; min-width: 0; flex: 1; }
.dtabs { display: flex; gap: 2px; padding: 0 10px; border-bottom: 1px solid var(--border); flex: none; }
.dtabs button { border: 0; background: none; padding: 8px 14px 6px; cursor: pointer; color: var(--muted); border-bottom: 2px solid transparent; font-size: 13px; }
.dtabs button.on { color: var(--accent); border-bottom-color: var(--accent); }
.cnt { font-size: 11px; padding: 0 6px; border-radius: 9px; background: var(--panel2); color: var(--muted); margin-left: 2px; }
.ctab { overflow: auto; padding: 14px 18px; display: flex; flex-direction: column; gap: 10px; }
.who { display: flex; gap: 12px; align-items: center; }
.avatar { width: 40px; height: 40px; border-radius: 6px; background: #f2a65a; color: #fff; display: grid; place-items: center; font-weight: 600; flex: none; }
.author { font-weight: 600; }
.kv { display: grid; grid-template-columns: 72px 1fr; gap: 4px 10px; font-size: 12px; color: var(--muted); align-items: center; }
.kv b { font-weight: 400; color: var(--text); }
.ctab h3 { margin: 6px 0 0; font-size: 15px; font-weight: 600; }
.body { white-space: pre-wrap; font: 12.5px/1.5 var(--mono); color: var(--muted); margin: 0; }
.link { color: var(--accent); cursor: pointer; }
.acts { display: flex; gap: 6px; flex-wrap: wrap; }
.flist { border-top: 1px solid var(--border); padding-top: 6px; }
.chtab { display: grid; flex: 1; min-height: 0; }
.ftree { overflow: auto; padding: 6px 0; }
.dv { display: flex; flex-direction: column; min-width: 0; min-height: 0; }
.fr { display: flex; align-items: center; gap: 6px; padding: 3px 4px; cursor: pointer; white-space: nowrap; border-radius: 5px; }
.fr:hover { background: var(--hover); }
.fr.on { background: var(--accentbg); color: var(--accenttx); }
.nm { font-size: 12px; overflow: hidden; text-overflow: ellipsis; }
.add-n { color: var(--ok); font-size: 12px; }
.del-n { color: var(--danger); font-size: 12px; }
</style>
