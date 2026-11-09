<script setup lang="ts">
import { type RepoState } from '../store'
import { newBranch, checkoutCommit, resetTo, copy } from '../actions'
import { ago, dateTime, short } from '../lib/format'

defineOptions({ name: 'ReflogView' })
defineProps<{ r: RepoState }>()
</script>

<template>
  <div class="cbar"><span class="sub">HEAD 的移動紀錄。不小心 reset、rebase 或刪掉分支時，可以從這裡找回 commit。</span></div>
  <div class="list">
    <div v-for="e in r.reflog" :key="e.selector" class="row">
      <span class="mono sub">{{ e.selector }}</span>
      <a class="mono link" title="複製 SHA" @click="copy(e.sha, short(e.sha))">{{ short(e.sha) }}</a>
      <span class="msg" :title="e.message">{{ e.message }}</span>
      <span class="sub" :title="dateTime(e.time)">{{ ago(e.time) }}</span>
      <span class="acts">
        <button class="btn sm" @click="newBranch(r, e.sha)">從這裡開分支</button>
        <button class="btn sm" @click="checkoutCommit(r, e.sha)">Checkout</button>
        <button class="btn sm" @click="resetTo(r, e.sha)">Reset 到這裡</button>
      </span>
    </div>
    <div v-if="!r.reflog.length" class="empty">沒有紀錄</div>
  </div>
</template>

<style scoped>
.list { overflow: auto; flex: 1; }
.row { display: grid; grid-template-columns: 90px 70px minmax(0, 1fr) 100px auto; gap: 10px; align-items: center; padding: 4px 12px; border-bottom: 1px solid var(--border); }
.row:hover { background: var(--hover); }
.msg { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.link { color: var(--accent); cursor: pointer; }
.acts { display: flex; gap: 4px; opacity: 0; }
.row:hover .acts { opacity: 1; }
</style>
