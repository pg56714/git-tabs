<script setup lang="ts">
import { computed, nextTick, reactive, ref, watch } from 'vue'
import { S, closeModal, type Field } from '../store'
import SettingsModal from './SettingsModal.vue'
import AddRepoModal from './AddRepoModal.vue'
import BlameModal from './BlameModal.vue'

const m = computed(() => S.modal)
const vals = reactive<Record<string, string>>({})
const errs = reactive<Record<string, string>>({})
const box = ref<HTMLElement | null>(null)

watch(m, async (v) => {
  for (const k of Object.keys(vals)) delete vals[k]
  for (const k of Object.keys(errs)) delete errs[k]
  if (v?.kind === 'prompt') for (const f of v.props.fields as Field[]) vals[f.key] = f.value ?? ''
  await nextTick()
  const first = box.value?.querySelector<HTMLElement>('input:not([type=checkbox]),textarea,button.primary')
  first?.focus()
})

function submitPrompt() {
  let bad = false
  for (const f of m.value!.props.fields as Field[]) {
    const e = f.check?.(vals[f.key] ?? '', vals) ?? null
    errs[f.key] = e ?? ''
    if (e) bad = true
  }
  if (!bad) closeModal({ ...vals })
}

function onKey(e: KeyboardEvent) {
  if (e.key === 'Enter' && !(e.target instanceof HTMLTextAreaElement)) {
    e.preventDefault()
    if (m.value?.kind === 'prompt') submitPrompt()
    else if (m.value?.kind === 'confirm') closeModal(true)
  }
}

type Choice = { value: string; label: string; primary?: boolean; danger?: boolean }
</script>

<template>
  <div v-if="m" class="shade" @mousedown.self="closeModal(null)">
    <SettingsModal v-if="m.kind === 'settings'" :tab="(m.props.tab as string) ?? 'ids'" />
    <AddRepoModal v-else-if="m.kind === 'addrepo'" />
    <BlameModal v-else-if="m.kind === 'blame'" :repo="m.props.repo as string" :path="m.props.path as string" :rev="m.props.rev as string | undefined" />
    <div v-else ref="box" class="modal" @keydown="onKey">
      <h2>{{ m.props.title }}</h2>
      <div class="body">
        <template v-if="m.kind === 'confirm'">
          <!-- body is built with esc() for every dynamic part -->
          <div v-html="m.props.body" />
        </template>
        <template v-else-if="m.kind === 'prompt'">
          <template v-for="f in m.props.fields as Field[]" :key="f.key">
            <label v-if="f.type === 'checkbox'" class="ck"><input type="checkbox" :checked="vals[f.key] === '1'" @change="vals[f.key] = ($event.target as HTMLInputElement).checked ? '1' : ''" /> {{ f.label }}</label>
            <label v-else class="row">
              <span>{{ f.label }}</span>
              <textarea v-if="f.type === 'textarea'" v-model="vals[f.key]" class="field" rows="3" :placeholder="f.placeholder" />
              <select v-else-if="f.type === 'select'" v-model="vals[f.key]" class="field">
                <option v-for="o in f.options" :key="o.value" :value="o.value">{{ o.label }}</option>
              </select>
              <input v-else v-model="vals[f.key]" class="field" :type="f.type === 'password' ? 'password' : 'text'" :placeholder="f.placeholder" @input="errs[f.key] = ''" />
            </label>
            <div v-if="errs[f.key]" class="err">{{ errs[f.key] }}</div>
          </template>
        </template>
        <template v-else-if="m.kind === 'choice'">
          <div>{{ m.props.body }}</div>
          <button v-for="c in m.props.choices as Choice[]" :key="c.value" class="btn choice" :class="{ primary: c.primary, danger: c.danger }" @click="closeModal(c.value)">{{ c.label }}</button>
        </template>
      </div>
      <div class="foot">
        <button class="btn" @click="closeModal(null)">取消</button>
        <button v-if="m.kind === 'confirm'" class="btn" :class="m.props.danger ? 'danger' : 'primary'" @click="closeModal(true)">{{ m.props.ok }}</button>
        <button v-if="m.kind === 'prompt'" class="btn primary" @click="submitPrompt">{{ m.props.ok }}</button>
      </div>
    </div>
  </div>
</template>

<style>
.shade { position: fixed; inset: 0; background: rgba(10, 12, 16, 0.35); z-index: 50; }
.modal { position: absolute; left: 50%; top: 10vh; transform: translateX(-50%); width: min(540px, 92vw); background: var(--panel); border: 1px solid var(--border); border-radius: 12px; box-shadow: var(--shadow); overflow: hidden; display: flex; flex-direction: column; max-height: 82vh; }
.modal.wide { width: min(880px, 94vw); }
.modal h2 { font-size: 15px; margin: 0; padding: 14px 16px 10px; }
.modal .body { padding: 0 16px 14px; display: flex; flex-direction: column; gap: 10px; overflow: auto; }
.modal .foot { display: flex; justify-content: flex-end; gap: 8px; padding: 10px 16px; border-top: 1px solid var(--border); background: var(--panel2); align-items: center; }
.modal label.row { display: grid; grid-template-columns: 110px 1fr; align-items: center; gap: 10px; }
.modal .ck { display: flex; gap: 6px; align-items: center; }
.modal .choice { height: auto; padding: 8px 12px; justify-content: flex-start; white-space: normal; text-align: left; }
</style>
