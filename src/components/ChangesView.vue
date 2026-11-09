<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { S, staged, unstaged, select, isSeen, openMenu, git, type RepoState } from '../store'
import { stageFiles, unstageFiles, stageAll, unstageAll, discardFiles, applyLines, commit, resolveConflict, abortOrContinue, openFile, fileHistory, blame, copy } from '../actions'
import type { StatusEntry } from '../types'
import FileTree from './FileTree.vue'
import DiffView from './DiffView.vue'
import DiffHead from './DiffHead.vue'

defineOptions({ name: 'ChangesView' })
const p = defineProps<{ r: RepoState }>()

const un = computed(() => unstaged(p.r))
const st = computed(() => staged(p.r))
const letterU = (f: StatusEntry) => (f.conflicted ? 'U' : f.untracked ? 'A' : f.y)
const letterS = (f: StatusEntry) => f.x
const unFiles = computed(() => un.value.map((f) => ({ path: f.path, status: letterU(f), key: 'u:' + f.path })))
const stFiles = computed(() => st.value.map((f) => ({ path: f.path, status: letterS(f), key: 's:' + f.path })))
const allPaths = computed(() => [...new Set(p.r.status.map((f) => f.path))])
const seenN = computed(() => allPaths.value.filter((x) => isSeen(p.r, 'u', x)).length)
const sel = computed(() => p.r.sel)
const selEntry = computed(() => p.r.status.find((f) => f.path === sel.value?.path))
const selStatus = computed(() => (selEntry.value ? (sel.value?.list === 's' ? letterS(selEntry.value) : letterU(selEntry.value)) : 'M'))
const conflicts = computed(() => un.value.filter((f) => f.conflicted))
const stateName = computed(() => ({ merge: 'Merge', rebase: 'Rebase', 'cherry-pick': 'Cherry-pick', revert: 'Revert' })[p.r.summary?.state ?? 'merge'])

// commit box
const subject = ref('')
const desc = ref('')
const amend = ref(false)
const err = ref('')
const committing = ref(false)
watch(
  () => p.r.path,
  () => {
    subject.value = desc.value = err.value = ''
    amend.value = false
  },
)
watch(amend, async (on) => {
  if (on && !subject.value && !desc.value) {
    const m = await git(p.r, 'last_message').catch(() => '')
    const [first, ...rest] = String(m).split('\n')
    subject.value = first
    desc.value = rest.join('\n').trim()
  }
})

async function doCommit(push: boolean) {
  err.value = ''
  if (conflicts.value.length) return (err.value = '先解決衝突的檔案')
  if (!st.value.length && !amend.value) return (err.value = '先 Stage 要提交的檔案')
  if (!subject.value.trim()) return (err.value = '輸入 commit 標題')
  committing.value = true
  const msg = subject.value.trim() + (desc.value.trim() ? '\n\n' + desc.value.trim() : '')
  const ok = await commit(p.r, msg, amend.value, push)
  committing.value = false
  if (ok) {
    subject.value = desc.value = ''
    amend.value = false
  }
}

function onKey(e: KeyboardEvent) {
  if ((e.ctrlKey || e.metaKey) && e.key === 'Enter') {
    e.preventDefault()
    doCommit(e.shiftKey)
  }
}

function fileMenu(e: MouseEvent, list: 'u' | 's', path: string) {
  const r = p.r
  const f = r.status.find((x) => x.path === path)
  openMenu(e, [
    list === 'u' ? { label: 'Stage', on: () => stageFiles(r, [path]) } : { label: '取消 Stage', on: () => unstageFiles(r, [path]) },
    ...(list === 'u' && !f?.conflicted ? [{ label: f?.untracked ? '刪除這個新檔案…' : '捨棄修改…', danger: true, on: () => discardFiles(r, [path]) }] : []),
    { sep: true },
    { label: '用編輯器開啟', on: () => openFile(r, path) },
    ...(f?.untracked ? [] : [{ label: '這個檔案的歷史', on: () => fileHistory(r, path) }, { label: 'Blame', on: () => blame(r, path) }]),
    { label: '複製路徑', on: () => copy(path, '路徑') },
  ])
}
</script>

<template>
  <div v-if="r.summary?.state" class="banner">
    <b>正在 {{ stateName }}</b>
    <span v-if="conflicts.length">還有 {{ conflicts.length }} 個衝突的檔案。解決後按「繼續」。</span>
    <span v-else>衝突都解決了，按「繼續」完成。</span>
    <span class="grow" />
    <button class="btn sm primary" :disabled="conflicts.length > 0" @click="abortOrContinue(r, 'continue')">繼續</button>
    <button class="btn sm" @click="abortOrContinue(r, 'abort')">中止 {{ stateName }}</button>
  </div>
  <div v-if="!r.status.length" class="empty">
    <div><span class="big">✓</span>沒有未提交的修改<br /><span class="sub">工作目錄是乾淨的</span></div>
  </div>
  <div v-else class="review">
    <div class="fp">
      <div class="top">
        <span class="sub">已看 {{ seenN }} / {{ allPaths.length }} 個檔案</span>
      </div>
      <div class="prog"><div :style="{ width: Math.round((seenN / allPaths.length) * 100) + '%' }" /></div>
      <div class="scroll">
        <div class="fsec">
          未暫存 <span class="sub">{{ un.length }}</span><span class="grow" />
          <button v-if="un.length" class="btn sm" @click="stageAll(r)">全部 Stage</button>
        </div>
        <FileTree :files="unFiles" :selected="sel?.list === 'u' ? sel.path : null" :seen="(x) => isSeen(r, 'u', x)" @select="(x) => select(r, 'u', x)" @dbl="(x) => stageFiles(r, [x])" @menu="(e, x) => fileMenu(e, 'u', x)" />
        <div v-if="!un.length" class="none">（沒有）</div>
        <div class="fsec">
          已暫存 <span class="sub">{{ st.length }}</span><span class="grow" />
          <button v-if="st.length" class="btn sm" @click="unstageAll(r)">全部取消</button>
        </div>
        <FileTree :files="stFiles" :selected="sel?.list === 's' ? sel.path : null" :seen="(x) => isSeen(r, 's', x)" @select="(x) => select(r, 's', x)" @dbl="(x) => unstageFiles(r, [x])" @menu="(e, x) => fileMenu(e, 's', x)" />
        <div v-if="!st.length" class="none">（沒有。按 Stage 把要提交的檔案放進來）</div>
      </div>
      <div class="cbox" @keydown="onKey">
        <input v-model="subject" class="field" placeholder="Commit 標題" @input="err = ''" />
        <textarea v-model="desc" class="field" placeholder="說明（可留空）" />
        <div v-if="err" class="err">{{ err }}</div>
        <label class="sub amend"><input v-model="amend" type="checkbox" /> Amend（改寫上一個 commit）</label>
        <div class="row">
          <span class="sub"><kbd>Ctrl</kbd>+<kbd>Enter</kbd></span><span class="grow" />
          <button class="btn" :disabled="committing" title="Ctrl+Shift+Enter" @click="doCommit(true)">Commit 並 Push</button>
          <button class="btn primary" :disabled="committing" @click="doCommit(false)">{{ amend ? 'Amend' : 'Commit' }}</button>
        </div>
      </div>
    </div>
    <div class="dv">
      <template v-if="sel">
        <DiffHead :r="r" :diff="r.diff" :list="sel.list" :path="sel.path" :status="selStatus">
          <template v-if="selEntry?.conflicted">
            <button class="btn sm" @click="resolveConflict(r, sel.path, 'use-ours')">用我的</button>
            <button class="btn sm" @click="resolveConflict(r, sel.path, 'use-theirs')">用對方的</button>
            <button class="btn sm" @click="openFile(r, sel.path)">用編輯器開啟</button>
            <button class="btn sm primary" @click="resolveConflict(r, sel.path, 'mark-resolved')">標記已解決</button>
          </template>
          <template v-else-if="sel.list === 'u'">
            <button class="btn sm" title="S" @click="stageFiles(r, [sel.path])">Stage 檔案</button>
            <button class="btn sm" @click="discardFiles(r, [sel.path])">捨棄</button>
          </template>
          <button v-else class="btn sm" title="S" @click="unstageFiles(r, [sel.path])">取消 Stage</button>
        </DiffHead>
        <DiffView :diff="r.diffFor === sel.list + ':' + sel.path ? r.diff : null" :mode="selEntry?.conflicted ? 'ro' : (sel.list as 'u' | 's')" @apply="(a, l) => applyLines(r, sel!.path, a, l)" />
      </template>
      <div v-else class="empty"><div>選一個檔案</div></div>
      <div class="hint">
        <span><kbd>J</kbd>/<kbd>K</kbd> 下/上個檔案</span><span><kbd>N</kbd>/<kbd>P</kbd> 下/上個改動</span><span><kbd>S</kbd> Stage</span><span><kbd>V</kbd> 已看</span>
        <span>點行號旁可選取單行，按住拖曳或 Shift 選多行</span><span class="grow" /><span v-if="S.side">並排</span>
      </div>
    </div>
  </div>
</template>

<style scoped>
.review { display: grid; grid-template-columns: 300px minmax(0, 1fr); flex: 1; min-height: 0; }
.fp { border-right: 1px solid var(--border); display: flex; flex-direction: column; min-height: 0; }
.top { padding: 8px 10px 4px; }
.prog { height: 3px; background: var(--border); margin: 0 10px 4px; border-radius: 2px; }
.prog div { height: 3px; background: var(--accent); border-radius: 2px; transition: width 0.2s; }
.scroll { flex: 1; overflow: auto; padding-bottom: 8px; }
.fsec { display: flex; align-items: center; gap: 6px; padding: 8px 10px 4px; font-size: 12px; color: var(--muted); position: sticky; top: 0; background: var(--panel); z-index: 1; }
.none { padding: 2px 16px; color: var(--faint); font-size: 12px; }
.cbox { border-top: 1px solid var(--border); padding: 8px; display: flex; flex-direction: column; gap: 6px; }
.cbox textarea { height: 56px; resize: vertical; }
.amend { white-space: nowrap; }
.row { display: flex; align-items: center; gap: 8px; }
.dv { display: flex; flex-direction: column; min-width: 0; min-height: 0; }
.hint { font-size: 11.5px; color: var(--faint); padding: 4px 10px; border-top: 1px solid var(--border); display: flex; gap: 12px; flex-wrap: wrap; }
</style>
