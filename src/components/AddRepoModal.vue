<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { call, pickFolder } from '../api'
import { S, closeModal, addRepos, toast, saveConfig } from '../store'
import { baseName } from '../lib/format'

const scanDir = ref(S.cfg.clone_dir || 'D:\\project')
const found = ref<string[]>([])
const picked = ref(new Set<string>())
const pin = ref('')
const url = ref('')
const cloneTo = ref('')
const busy = ref(false)
const err = ref('')
const open = computed(() => new Set(S.repos.map((r) => r.path.replace(/\//g, '\\').toLowerCase())))
const fresh = computed(() => found.value.filter((p) => !open.value.has(p.replace(/\//g, '\\').toLowerCase())))

async function scan() {
  err.value = ''
  found.value = await call<string[]>('scan_dir', { path: scanDir.value }).catch(() => [])
  if (!found.value.length) err.value = '這個資料夾底下沒有找到 git repo'
}

async function browse() {
  const d = await pickFolder('選擇要掃描的資料夾')
  if (d) {
    scanDir.value = d
    await scan()
  }
}

async function addPicked() {
  busy.value = true
  const n = await addRepos([...picked.value], pin.value || null)
  busy.value = false
  if (n) {
    toast(`已加入 ${n} 個 repo`)
    closeModal(true)
  }
}

async function addOne() {
  const d = await pickFolder('選擇 repo 資料夾')
  if (!d) return
  if (await addRepos([d], pin.value || null)) closeModal(true)
}

const defaultClone = computed(() => {
  const name = url.value.trim().replace(/\.git$/, '').split(/[/:]/).pop() ?? ''
  return name ? `${scanDir.value.replace(/[\\/]$/, '')}\\${name}` : ''
})

async function clone() {
  err.value = ''
  if (!url.value.trim()) return (err.value = '輸入 repo 網址')
  const dir = cloneTo.value.trim() || defaultClone.value
  busy.value = true
  try {
    const path = await call<string>('clone', { url: url.value.trim(), dir })
    await addRepos([path], pin.value || null)
    toast('已 clone ' + baseName(path))
    closeModal(true)
  } catch (e) {
    err.value = String(e).replace(/^Error: /, '')
  } finally {
    busy.value = false
  }
}

onMounted(async () => {
  await scan()
  if (!S.cfg.clone_dir) {
    S.cfg.clone_dir = scanDir.value
    saveConfig()
  }
})
</script>

<template>
  <div class="modal wide">
    <h2>加入 repo</h2>
    <div class="body">
      <div class="line">
        <input v-model="scanDir" class="field mono" @keydown.enter="scan" />
        <button class="btn" @click="scan">掃描</button>
        <button class="btn" @click="browse">選資料夾…</button>
      </div>
      <div class="sub">在這個資料夾底下找到 {{ found.length }} 個 repo，其中 {{ fresh.length }} 個還沒開：</div>
      <div class="list">
        <label v-for="p in fresh" :key="p" class="item">
          <input type="checkbox" :checked="picked.has(p)" @change="picked.has(p) ? picked.delete(p) : picked.add(p)" />
          <b>{{ baseName(p) }}</b> <span class="sub mono">{{ p }}</span>
        </label>
        <div v-if="!fresh.length" class="sub">（都已經開了）</div>
      </div>
      <div class="line">
        <span class="sub">或</span>
        <button class="btn sm" @click="addOne">直接選一個 repo 資料夾…</button>
      </div>
      <fieldset>
        <legend>Clone 新的 repo</legend>
        <div class="line"><input v-model="url" class="field mono" placeholder="git@github.com:org/repo.git" /></div>
        <div class="line"><input v-model="cloneTo" class="field mono" :placeholder="defaultClone || '存放位置'" /><button class="btn" :disabled="busy" @click="clone">Clone</button></div>
      </fieldset>
      <label v-if="S.cfg.identities.length" class="row">
        <span>身分</span>
        <select v-model="pin" class="field">
          <option value="">自動（依規則）</option>
          <option v-for="id in S.cfg.identities" :key="id.id" :value="id.id">{{ id.name }}（{{ id.account || id.email }}）</option>
        </select>
      </label>
      <div v-if="err" class="err">{{ err }}</div>
    </div>
    <div class="foot">
      <span v-if="busy" class="sub">處理中…</span>
      <button class="btn" @click="closeModal(null)">取消</button>
      <button class="btn primary" :disabled="!picked.size || busy" @click="addPicked">加入 {{ picked.size || '' }}</button>
    </div>
  </div>
</template>

<style scoped>
.line { display: flex; gap: 6px; align-items: center; }
.list { max-height: 260px; overflow: auto; border: 1px solid var(--border); border-radius: 8px; padding: 6px; }
.item { display: flex; gap: 8px; align-items: center; padding: 3px 4px; border-radius: 5px; cursor: pointer; white-space: nowrap; overflow: hidden; }
.item b { flex: none; }
.item .sub { overflow: hidden; text-overflow: ellipsis; }
.item:hover { background: var(--hover); }
fieldset { border: 1px solid var(--border); border-radius: 8px; display: flex; flex-direction: column; gap: 6px; }
legend { color: var(--muted); font-size: 12px; padding: 0 4px; }
</style>
