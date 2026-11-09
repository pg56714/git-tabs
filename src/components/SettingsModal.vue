<script setup lang="ts">
import { computed, onMounted, reactive, ref } from 'vue'
import { call, pickFile, openUrl, copyText } from '../api'
import { S, closeModal, saveConfig, toast, setFetchTimer, pullLog } from '../store'
import type { Identity, SshTest } from '../types'
import { COLORS, autoShort, tagOf, matchIdentity, newIdentity, normUrl, ruleForUrl, fromFound, type Found } from '../lib/identity'
import { baseName } from '../lib/format'

const p = defineProps<{ tab: string; detected?: { ids: Identity[]; def: string | null } }>()
const tab = ref(p.tab)
const draft = reactive({
  ids: (p.detected?.ids ?? JSON.parse(JSON.stringify(S.cfg.identities))) as Identity[],
  def: p.detected?.def ?? S.cfg.default_identity ?? '',
  theme: S.cfg.theme,
  git: S.cfg.git_path,
  fetch: String(S.cfg.fetch_minutes),
  clone: S.cfg.clone_dir,
})
const err = ref('')
const saving = ref(false)
const home = ref('C:\\Users\\user')
const tests = reactive<Record<string, { running: boolean; res?: SshTest }>>({})
const gens = reactive<Record<string, { open: boolean; path: string; pass: string; pub?: string; err?: string; busy?: boolean }>>({})

onMounted(async () => {
  home.value = (await call<string>('home_dir').catch(() => '')) || home.value
})

const pinOf = (path: string) => S.cfg.repos.find((x) => x.path === path)?.pin ?? null
function matched(id: Identity) {
  return S.repos.filter((r) => {
    const pin = pinOf(r.path)
    if (pin) return pin === id.id
    return (matchIdentity(draft.ids, r.path, r.summary?.remote_url ?? null)?.id ?? (draft.def || null)) === id.id
  })
}
const unmatched = computed(() => S.repos.filter((r) => !pinOf(r.path) && !matchIdentity(draft.ids, r.path, r.summary?.remote_url ?? null)))
const defName = computed(() => draft.ids.find((i) => i.id === draft.def)?.name ?? '')

function assign(repoPath: string, idx: string) {
  if (idx === '') return
  const r = S.repos.find((x) => x.path === repoPath)!
  const rule = r.summary?.remote_url ? ruleForUrl(r.summary.remote_url) : null
  draft.ids[+idx].rules.push(rule ? { t: 'remote', v: rule } : { t: 'dir', v: r.path })
}

const scanning = ref(false)

/** Adds identities for SSH keys / includeIf files on this computer that aren't listed yet. */
async function scan() {
  scanning.value = true
  const found = await call<Found>('identities_detect').catch(() => null)
  scanning.value = false
  if (!found) return toast('讀不到這台電腦的設定', 'err')
  const { ids, def } = fromFound(found, draft.ids)
  draft.ids.push(...ids)
  if (!draft.def && def) draft.def = def
  toast(ids.length ? `找到 ${ids.length} 個新的身分，確認後按儲存` : '沒有找到新的 SSH key 或身分設定', 'info')
}

function add() {
  draft.ids.push(newIdentity(draft.ids))
}

function remove(i: number) {
  const id = draft.ids[i]
  draft.ids.splice(i, 1)
  if (draft.def === id.id) draft.def = draft.ids[0]?.id ?? ''
}

async function browseKey(id: Identity) {
  const f = await pickFile('選擇 SSH 私鑰（不是 .pub）', `${home.value}\\.ssh`)
  if (f) {
    id.key = f.replace(/\.pub$/, '')
    id.verified = false
  }
}

async function test(id: Identity) {
  if (!id.key.trim()) return toast('先填 SSH key 的路徑', 'info')
  tests[id.id] = { running: true }
  const res = await call<SshTest>('ssh_test', { key: id.key, host: id.host || 'github.com' }).catch((e) => ({ ok: false, account: null, output: String(e) }))
  tests[id.id] = { running: false, res }
  if (res.ok && res.account) {
    id.account = res.account
    id.verified = true
  }
  pullLog()
}

function genOpen(id: Identity) {
  const slug = (id.account || id.name).replace(/[^\w-]+/g, '_').toLowerCase() || id.id
  gens[id.id] = gens[id.id] ?? { open: false, path: `${home.value}\\.ssh\\id_ed25519_${slug}`, pass: '' }
  gens[id.id].open = !gens[id.id].open
}

async function gen(id: Identity) {
  const g = gens[id.id]
  if (!id.email.trim()) return (g.err = '先填 user.email，會寫進 key 的註解')
  g.busy = true
  g.err = ''
  try {
    g.pub = await call<string>('ssh_keygen', { path: g.path, email: id.email, passphrase: g.pass })
    id.key = g.path
    id.verified = false
    g.pass = ''
  } catch (e) {
    g.err = String(e).replace(/^Error: /, '')
  }
  g.busy = false
  pullLog()
}

async function save() {
  err.value = ''
  for (const id of draft.ids) {
    if (!id.name.trim()) return (err.value = '每個身分都要有名稱')
    if (id.email.trim() && !/^\S+@\S+\.\S+$/.test(id.email.trim())) return (err.value = `「${id.name}」的 email 格式不對`)
  }
  saving.value = true
  try {
    const def = draft.def && draft.ids.some((i) => i.id === draft.def) ? draft.def : null
    await call('identities_apply', { identities: draft.ids, default: def })
    S.cfg.identities = draft.ids
    S.cfg.default_identity = def
    S.cfg.theme = draft.theme
    S.cfg.git_path = draft.git.trim()
    S.cfg.fetch_minutes = Math.max(0, parseInt(draft.fetch) || 0)
    S.cfg.clone_dir = draft.clone.trim()
    await saveConfig()
    setFetchTimer()
    // Pinned repos keep their own copy of the identity; refresh it.
    for (const r of S.cfg.repos.filter((x) => x.pin)) {
      const id = draft.ids.find((i) => i.id === r.pin)
      await call('identity_pin', { repo: r.path, identity: id ?? null }).catch(() => {})
      if (!id) r.pin = null
    }
    await pullLog()
    toast('設定已儲存')
    closeModal(true)
  } catch (e) {
    err.value = String(e).replace(/^Error: /, '')
  } finally {
    saving.value = false
  }
}

const copyPub = async (s: string) => {
  await copyText(s)
  toast('已複製公鑰')
}
</script>

<template>
  <div class="modal wide">
    <h2>設定</h2>
    <div class="tabs">
      <div class="seg">
        <button :class="{ on: tab === 'ids' }" @click="tab = 'ids'">身分與 SSH key</button>
        <button :class="{ on: tab === 'general' }" @click="tab = 'general'">一般</button>
      </div>
    </div>
    <div class="body">
      <template v-if="tab === 'ids'">
        <div v-if="detected" class="okline">已從這台電腦找到 {{ detected.ids.length }} 個身分（~/.ssh 的 key 和現有的 git 設定）。確認名稱、email 和規則後按「儲存」才會生效；不需要的按 🗑 刪掉。</div>
        <div class="sub">依照 remote 網址或資料夾自動選身分。儲存時會寫進 git 全域設定的 <span class="mono">includeIf</span>，所以在終端機或 Claude 執行的 git 也會用同一個身分。</div>
        <div v-if="!draft.ids.length" class="emptybox">還沒有身分。按「＋ 新增身分」，填好 SSH key 和 email。</div>
        <div v-for="(id, i) in draft.ids" :key="id.id" class="card">
          <div class="top">
            <input v-model="id.name" class="field name" placeholder="例如：個人 GitHub" />
            <span class="idc" :class="'c-' + id.color">{{ tagOf(id) }}</span>
            <span class="sub">
              <template v-if="id.verified"><span class="ok">✓ 已驗證</span> 帳號 <b class="mono">{{ id.account }}</b></template>
              <template v-else>還沒驗證這把 key 對應哪個帳號</template>
            </span>
            <span class="grow" />
            <button class="btn sm ghost" title="刪除這個身分" @click="remove(i)">🗑</button>
          </div>
          <div class="row">
            <span>標籤</span>
            <span class="line">
              <input v-model="id.short" class="field" style="width: 110px" maxlength="6" :placeholder="autoShort(id.name)" />
              <span class="sub">留空會自動用「{{ autoShort(id.name) }}」</span>
              <span class="grow" />
              <button v-for="c in COLORS" :key="c" class="sw" :class="['c-' + c, { on: id.color === c }]" :title="c" :aria-label="'顏色 ' + c" @click="id.color = c" />
            </span>
          </div>
          <div class="row">
            <span>SSH key</span>
            <span class="line">
              <input v-model="id.key" class="field mono" placeholder="C:\Users\you\.ssh\id_ed25519_xxx" @input="id.verified = false" />
              <button class="btn sm" @click="browseKey(id)">選檔案…</button>
              <button class="btn sm" :disabled="tests[id.id]?.running" @click="test(id)">{{ tests[id.id]?.running ? '連線中…' : '測試連線' }}</button>
              <button class="btn sm" :class="{ on: gens[id.id]?.open }" @click="genOpen(id)">產生新 key</button>
            </span>
          </div>
          <div v-if="tests[id.id]?.res" :class="tests[id.id].res!.ok ? 'okline' : 'warnline'" class="mono">{{ tests[id.id].res!.output || '（沒有輸出）' }}</div>
          <div v-if="gens[id.id]?.open" class="gen">
            <template v-if="!gens[id.id].pub">
              <div class="row"><span>存到</span><input v-model="gens[id.id].path" class="field mono" /></div>
              <div class="row"><span>密碼（可留空）</span><input v-model="gens[id.id].pass" class="field" type="password" placeholder="留空就不設密碼" /></div>
              <div class="line"><button class="btn sm primary" :disabled="gens[id.id].busy" @click="gen(id)">產生 ed25519 key</button><span class="sub">私鑰只會留在這台電腦。已經存在的檔案不會被覆蓋。</span></div>
              <div v-if="gens[id.id].err" class="err">{{ gens[id.id].err }}</div>
            </template>
            <template v-else>
              <div class="sub">已產生 <span class="mono">{{ gens[id.id].path }}</span>。把下面的公鑰加到這個帳號的 GitHub SSH keys：</div>
              <div class="pub">{{ gens[id.id].pub }}</div>
              <div class="line">
                <button class="btn sm" @click="copyPub(gens[id.id].pub!)">複製公鑰</button>
                <button class="btn sm" @click="openUrl(`https://${id.host || 'github.com'}/settings/ssh/new`)">打開 GitHub 新增 SSH key 頁面</button>
                <span class="sub">貼好之後按「測試連線」</span>
              </div>
            </template>
          </div>
          <div class="row"><span>主機</span><input v-model="id.host" class="field mono" style="width: 200px" placeholder="github.com" /></div>
          <div class="row"><span>user.name</span><input v-model="id.user" class="field" placeholder="Your Name" /></div>
          <div class="row"><span>user.email</span><input v-model="id.email" class="field" placeholder="name@example.com" /></div>
          <div class="sub">什麼時候用這個身分（符合任一條）</div>
          <div v-for="(ru, j) in id.rules" :key="j" class="rule">
            <select v-model="ru.t" class="field">
              <option value="remote">remote 網址符合</option>
              <option value="dir">資料夾在底下</option>
            </select>
            <input v-model="ru.v" class="field mono" :placeholder="ru.t === 'remote' ? 'github.com/your-org/*' : 'D:\\work\\'" />
            <button class="btn sm ghost" title="刪除規則" @click="id.rules.splice(j, 1)">×</button>
          </div>
          <div class="line">
            <button class="btn sm" @click="id.rules.push({ t: 'remote', v: '' })">＋ 規則</button>
            <span class="sub">目前對應：{{ matched(id).map((r) => r.name).join('、') || '（沒有開著的 repo 符合）' }}</span>
          </div>
        </div>
        <div class="line">
          <button class="btn" @click="add">＋ 新增身分</button>
          <button class="btn" :disabled="scanning" title="找 ~/.ssh 的 key、~/.ssh/config 和 git 的 includeIf" @click="scan">{{ scanning ? '尋找中…' : '從這台電腦找' }}</button>
          <span class="grow" />
          <template v-if="draft.ids.length">
            <span class="sub">都沒對應時用</span>
            <select v-model="draft.def" class="field" style="width: auto">
              <option value="">git 原本的全域設定</option>
              <option v-for="id in draft.ids" :key="id.id" :value="id.id">{{ id.name }}</option>
            </select>
          </template>
        </div>
        <template v-if="draft.ids.length">
          <div v-if="unmatched.length && defName" class="sub">這些 repo 沒有符合的規則，會用預設的「{{ defName }}」；要換成別的身分可以在這裡指定：</div>
          <div v-for="r in unmatched" :key="r.path" class="line" :class="defName ? 'infoline' : 'warnline'">
            <span><b>{{ r.name }}</b>（{{ normUrl(r.summary?.remote_url) || '沒有 remote' }}）{{ defName ? '用預設' : '沒有對應的身分' }}</span>
            <span class="grow" />
            <select class="field" style="width: auto" @change="assign(r.path, ($event.target as HTMLSelectElement).value)">
              <option value="">指定給…</option>
              <option v-for="(id, i) in draft.ids" :key="id.id" :value="i">{{ id.name }}</option>
            </select>
          </div>
          <div v-if="!unmatched.length && S.repos.length" class="okline">✓ 開著的 repo 都有對應的身分</div>
        </template>
      </template>
      <template v-else>
        <label class="row"><span>外觀</span>
          <select v-model="draft.theme" class="field"><option value="system">跟系統</option><option value="light">淺色</option><option value="dark">深色</option></select>
        </label>
        <label class="row"><span>git 路徑</span><input v-model="draft.git" class="field mono" placeholder="留空就用 PATH 裡的 git" /></label>
        <div class="sub indent">目前：{{ S.gitVersion || '找不到 git' }}</div>
        <label class="row"><span>背景 fetch</span>
          <select v-model="draft.fetch" class="field"><option value="0">不要</option><option value="5">每 5 分鐘</option><option value="10">每 10 分鐘</option><option value="30">每 30 分鐘</option><option value="60">每小時</option></select>
        </label>
        <label class="row"><span>Repo 資料夾</span><input v-model="draft.clone" class="field mono" placeholder="D:\project" /></label>
        <div class="sub indent">「加入 repo」會先掃描這裡，clone 也預設放這裡。</div>
        <div class="sub">HTTPS 的 repo 用 Git Credential Manager 存帳號 token；網址帶帳號名稱（https://帳號@github.com/…）就能分開存。設定檔位置：app 的設定資料夾 config.json。目前開了 {{ S.repos.length }} 個 repo：{{ S.repos.map((r) => baseName(r.path)).join('、') }}</div>
      </template>
      <div v-if="err" class="err">{{ err }}</div>
    </div>
    <div class="foot">
      <button class="btn" @click="closeModal(null)">取消</button>
      <button class="btn primary" :disabled="saving" @click="save">{{ saving ? '儲存中…' : '儲存' }}</button>
    </div>
  </div>
</template>

<style scoped>
.tabs { padding: 0 16px 10px; border-bottom: 1px solid var(--border); margin-bottom: 12px; }
.card { border: 1px solid var(--border); border-radius: 10px; padding: 12px; display: flex; flex-direction: column; gap: 8px; }
.top { display: flex; align-items: center; gap: 8px; }
.name { width: 180px; font-weight: 600; }
.row { display: grid; grid-template-columns: 110px 1fr; align-items: center; gap: 10px; }
.line { display: flex; gap: 8px; align-items: center; flex-wrap: wrap; }
.rule { display: grid; grid-template-columns: 160px 1fr auto; gap: 6px; align-items: center; }
.sw { width: 18px; height: 18px; border-radius: 50%; border: 0; padding: 0; cursor: pointer; background: var(--ct); }
.sw.on { outline: 2px solid var(--text); outline-offset: 2px; }
.infoline { font-size: 12px; color: var(--muted); background: var(--panel2); border-radius: 6px; padding: 4px 8px; }
.okline { font-size: 12px; color: var(--ok); background: var(--okbg); border-radius: 6px; padding: 6px 8px; white-space: pre-wrap; word-break: break-word; }
.warnline { font-size: 12px; color: var(--warn); background: var(--warnbg); border-radius: 6px; padding: 6px 8px; white-space: pre-wrap; word-break: break-word; }
.gen { display: flex; flex-direction: column; gap: 6px; padding: 8px; border: 1px dashed var(--border); border-radius: 8px; }
.pub { font: 11.5px var(--mono); background: var(--panel2); border: 1px solid var(--border); border-radius: 6px; padding: 6px 8px; word-break: break-all; user-select: all; }
.emptybox { padding: 16px; border: 1px dashed var(--border); border-radius: 10px; color: var(--muted); text-align: center; }
.indent { margin: -6px 0 0 120px; }
</style>
