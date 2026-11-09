<script setup lang="ts">
import { computed } from 'vue'
import { S, identityOf, type RepoState } from '../store'
import { tagOf } from '../lib/identity'

const p = defineProps<{ r: RepoState }>()
const who = computed(() => identityOf(p.r))
</script>

<template>
  <template v-if="S.cfg.identities.length">
    <span v-if="who.id" class="idc" :class="'c-' + (who.id.color || 'gray')" :title="`${who.id.name}：${who.id.account || who.id.email}${who.pinned ? '（手動指定）' : '（依規則自動）'}`">
      {{ who.pinned ? '📌 ' : '' }}{{ tagOf(who.id) }}
    </span>
    <span v-else class="idc u" title="沒有對應的身分，會用預設">?</span>
  </template>
</template>
