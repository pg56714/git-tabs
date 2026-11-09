<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, shallowRef, watch } from 'vue'
import type { FileDiff } from '../types'
import { S } from '../store'
import { hunkRows, sideRows, totalLines, type Row } from '../lib/diff'
import { highlightHunks, splitTokens, type Tok } from '../lib/highlight'
import { lineSel } from '../lineSel'

/** mode: 'u' unstaged (can stage/discard), 's' staged (can unstage), 'ro' read-only */
const p = defineProps<{ diff: FileDiff | null; mode: 'u' | 's' | 'ro' }>()
const emit = defineEmits<{ apply: [action: 'stage' | 'unstage' | 'discard', lines: [number, number][]] }>()

const LIMIT = 3000
const showAll = ref(false)
const toks = shallowRef<Tok[][][] | null>(null)
const sel = ref(new Set<string>())
const anchor = ref<[number, number] | null>(null)
const dragging = ref(false)
const curHunk = ref(0)
const body = ref<HTMLElement | null>(null)

const rows = computed(() => (p.diff?.hunks ?? []).map((h) => hunkRows(h)))
const big = computed(() => !!p.diff && totalLines(p.diff) > LIMIT && !showAll.value)
const visibleHunks = computed(() => {
  if (!big.value) return rows.value.map((r, i) => ({ hi: i, rows: r }))
  let left = LIMIT
  const out: { hi: number; rows: Row[] }[] = []
  rows.value.forEach((r, i) => {
    if (left <= 0) return
    out.push({ hi: i, rows: r.slice(0, left) })
    left -= r.length
  })
  return out
})
const editable = computed(() => p.mode !== 'ro')

watch(
  () => p.diff,
  async (d) => {
    sel.value = new Set()
    anchor.value = null
    curHunk.value = 0
    showAll.value = false
    toks.value = null
    body.value?.scrollTo({ top: 0 })
    if (!d || d.binary || totalLines(d) > 8000) return
    const t = await highlightHunks(d.path, d.hunks).catch(() => null)
    if (p.diff === d) toks.value = t
  },
  { immediate: true },
)

const key = (h: number, i: number) => `${h}:${i}`

function segs(hi: number, r: Row) {
  const base = toks.value?.[hi]?.[r.i] ?? [{ text: r.text }]
  return splitTokens(base, r.word)
}

function pick(hi: number, r: Row, e: MouseEvent) {
  if (!editable.value || r.t === ' ') return
  const s = new Set(sel.value)
  if (e.shiftKey && anchor.value && anchor.value[0] === hi) {
    const [a, b] = [Math.min(anchor.value[1], r.i), Math.max(anchor.value[1], r.i)]
    for (const x of rows.value[hi]) if (x.t !== ' ' && x.i >= a && x.i <= b) s.add(key(hi, x.i))
  } else if (s.has(key(hi, r.i)) && !dragging.value) s.delete(key(hi, r.i))
  else s.add(key(hi, r.i))
  sel.value = s
  anchor.value = [hi, r.i]
}

function down(hi: number, r: Row, e: MouseEvent) {
  if (e.button !== 0 || !editable.value || r.t === ' ') return
  e.preventDefault()
  pick(hi, r, e)
  dragging.value = true
}
function enter(hi: number, r: Row) {
  if (!dragging.value || r.t === ' ' || !anchor.value || anchor.value[0] !== hi) return
  const s = new Set(sel.value)
  s.add(key(hi, r.i))
  sel.value = s
}
const up = () => (dragging.value = false)

const pairs = (): [number, number][] => [...sel.value].map((k) => k.split(':').map(Number) as [number, number])

function applySel(action: 'stage' | 'unstage' | 'discard') {
  const v = pairs()
  if (!v.length) return
  emit('apply', action, v)
  sel.value = new Set()
}

function applyHunk(hi: number, action: 'stage' | 'unstage' | 'discard') {
  const v = rows.value[hi].filter((r) => r.t !== ' ').map((r) => [hi, r.i] as [number, number])
  emit('apply', action, v)
}

function gotoHunk(d: number) {
  const hs = body.value?.querySelectorAll<HTMLElement>('.hh')
  if (!hs?.length) return
  curHunk.value = Math.max(0, Math.min(hs.length - 1, curHunk.value + d))
  body.value!.scrollTo({ top: hs[curHunk.value].offsetTop - 4, behavior: 'smooth' })
}
const onHunkEvent = (e: Event) => gotoHunk((e as CustomEvent<number>).detail)

onMounted(() => {
  window.addEventListener('git-tabs:hunk', onHunkEvent)
  window.addEventListener('mouseup', up)
  // "S" stages the selected lines instead of the whole file while lines are selected.
  lineSel.handler = () => {
    if (!sel.value.size || p.mode === 'ro') return false
    applySel(p.mode === 's' ? 'unstage' : 'stage')
    return true
  }
})
onBeforeUnmount(() => {
  window.removeEventListener('git-tabs:hunk', onHunkEvent)
  window.removeEventListener('mouseup', up)
  lineSel.handler = null
})
defineExpose({ gotoHunk })
</script>

<template>
  <div v-if="!diff" class="empty"><div><span class="big">◇</span>選一個檔案來看修改</div></div>
  <div v-else-if="diff.binary" class="empty"><div><span class="big">⬚</span>二進位檔案，無法顯示文字差異</div></div>
  <div v-else-if="!diff.hunks.length" class="empty"><div><span class="big">≡</span>沒有內容差異<br /><span class="sub">可能只改了檔案權限，或是空檔案</span></div></div>
  <div v-else ref="body" class="dbody" :class="{ wrap: S.wrap, side: S.side }">
    <div v-if="sel.size" class="selbar">
      已選 {{ sel.size }} 行
      <template v-if="mode === 'u'">
        <button class="btn sm primary" @click="applySel('stage')">Stage 選取的行 <kbd>S</kbd></button>
        <button class="btn sm" @click="applySel('discard')">捨棄選取的行</button>
      </template>
      <button v-else-if="mode === 's'" class="btn sm primary" @click="applySel('unstage')">取消 Stage 選取的行 <kbd>S</kbd></button>
      <button class="btn sm ghost" @click="sel = new Set()">清除選取</button>
    </div>
    <template v-for="vh in visibleHunks" :key="vh.hi">
      <div class="hh" :class="{ cur: vh.hi === curHunk }">
        <span class="hdr">{{ diff.hunks[vh.hi].header }}</span>
        <span class="grow" />
        <template v-if="mode === 'u'">
          <button class="btn sm" @click="applyHunk(vh.hi, 'stage')">Stage 這段</button>
          <button class="btn sm" @click="applyHunk(vh.hi, 'discard')">捨棄這段</button>
        </template>
        <button v-else-if="mode === 's'" class="btn sm" @click="applyHunk(vh.hi, 'unstage')">取消 Stage 這段</button>
      </div>
      <template v-if="!S.side">
        <div
          v-for="r in vh.rows"
          :key="r.i"
          class="dl"
          :class="{ add: r.t === '+', del: r.t === '-', pick: sel.has(key(vh.hi, r.i)), can: editable && r.t !== ' ' }"
          @mousedown="down(vh.hi, r, $event)"
          @mouseenter="enter(vh.hi, r)"
        >
          <span class="ln">{{ r.old ?? '' }}</span><span class="ln">{{ r.new ?? '' }}</span><span class="sg">{{ r.t === ' ' ? '' : r.t }}</span>
          <span class="code"><span v-for="(t, k) in segs(vh.hi, r)" :key="k" class="tok" :class="t.mark ? (r.t === '+' ? 'wa' : 'wd') : ''" :style="t.light ? { '--l': t.light, '--d': t.dark } : undefined">{{ t.text }}</span></span>
        </div>
      </template>
      <template v-else>
        <div v-for="(pair, k) in sideRows(vh.rows)" :key="k" class="sr">
          <div
            v-for="(x, side) in pair"
            :key="side"
            class="half"
            :class="{ blank: !x, add: x?.t === '+', del: x?.t === '-', pick: x && sel.has(key(vh.hi, x.i)), can: x && editable && x.t !== ' ' }"
            @mousedown="x && down(vh.hi, x, $event)"
            @mouseenter="x && enter(vh.hi, x)"
          >
            <template v-if="x">
              <span class="ln">{{ side === 0 ? x.old : x.new }}</span>
              <span class="code"><span v-for="(t, j) in segs(vh.hi, x)" :key="j" class="tok" :class="t.mark ? (x.t === '+' ? 'wa' : 'wd') : ''" :style="t.light ? { '--l': t.light, '--d': t.dark } : undefined">{{ t.text }}</span></span>
            </template>
          </div>
        </div>
      </template>
    </template>
    <div v-if="big" class="more">
      檔案很大，只顯示前 {{ LIMIT }} 行。<button class="btn sm" @click="showAll = true">顯示全部（{{ totalLines(diff) }} 行）</button>
    </div>
  </div>
</template>

<style scoped>
.dbody { flex: 1; overflow: auto; font: 12.5px/1.6 var(--mono); position: relative; min-height: 0; }
.selbar { position: sticky; top: 0; z-index: 3; display: flex; align-items: center; gap: 8px; padding: 6px 10px; background: var(--accentbg); color: var(--accenttx); font-family: 'Segoe UI', system-ui, sans-serif; font-size: 12.5px; }
.hh { display: flex; align-items: center; gap: 6px; background: var(--hunk); color: var(--muted); padding: 2px 10px; position: sticky; top: 0; z-index: 2; min-height: 28px; font-size: 12px; }
.selbar + .hh { top: 36px; }
.hh.cur { box-shadow: inset 3px 0 var(--accent); }
.hh .btn { font-family: 'Segoe UI', system-ui, sans-serif; opacity: 0.85; }
.hdr { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.dl { display: grid; grid-template-columns: 48px 48px 16px minmax(0, 1fr); }
.ln { color: var(--faint); text-align: right; padding-right: 8px; user-select: none; border-right: 1px solid var(--border); }
.sg { color: var(--faint); text-align: center; user-select: none; }
.code { white-space: pre; padding-right: 16px; }
.wrap .code { white-space: pre-wrap; word-break: break-all; }
.add { background: var(--addbg); }
.del { background: var(--delbg); }
.add .ln, .del .ln { color: var(--muted); }
.can { cursor: pointer; }
.can:hover .ln { background: rgba(127, 127, 127, 0.15); }
.pick, .pick.add, .pick.del { background: var(--selbg); box-shadow: inset 3px 0 var(--accent); }
.wa { background: var(--addw); border-radius: 2px; }
.wd { background: var(--delw); border-radius: 2px; }
.sr { display: grid; grid-template-columns: minmax(0, 1fr) minmax(0, 1fr); }
.half { display: grid; grid-template-columns: 48px minmax(0, 1fr); min-height: 20px; overflow: hidden; }
.half + .half { border-left: 1px solid var(--border); }
.half .code { padding-left: 8px; }
.half.blank { background: repeating-linear-gradient(135deg, transparent 0 6px, var(--panel2) 6px 12px); }
.more { padding: 12px; color: var(--muted); font-family: 'Segoe UI', system-ui, sans-serif; display: flex; gap: 10px; align-items: center; }
</style>
