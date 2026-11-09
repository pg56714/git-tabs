<script setup lang="ts">
import { openModal } from '../store'
</script>

<template>
  <div class="welcome">
    <div class="box">
      <img src="/icon.svg" width="72" height="72" alt="" />
      <h1>git-tabs</h1>
      <p class="muted">每個 repo 一個分頁。先加入你的 repo。</p>
      <button class="btn primary" @click="openModal('addrepo')">＋ 加入 repo</button>
      <p class="sub">之後可以用 <kbd>Ctrl</kbd>+<kbd>K</kbd> 搜尋所有指令</p>
    </div>
  </div>
</template>

<style scoped>
.welcome { display: grid; place-items: center; background: var(--panel); }
.box { text-align: center; display: flex; flex-direction: column; align-items: center; gap: 10px; }
h1 { margin: 0; font-size: 22px; }
p { margin: 0; }
</style>
