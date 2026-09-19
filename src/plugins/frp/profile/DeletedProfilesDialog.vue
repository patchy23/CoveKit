<script setup lang="ts">
/** 已删除配置：恢复到原位置，不自动启动或切换当前编辑档案。 */
import { ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { UiAlert, UiButton, UiEmptyState, UiModal, UiScrollArea, UiSpinner } from '@/core/ui'
import { ipc } from '../ipc'
import type { FrpDeletedProfile } from '../contracts'

const props = defineProps<{ open: boolean }>()
const emit = defineEmits<{ close: []; restored: [] }>()
const { t } = useI18n()
const entries = ref<FrpDeletedProfile[]>([])
const loading = ref(false)
const busy = ref(false)
const error = ref('')

async function load() {
  loading.value = true
  error.value = ''
  try {
    entries.value = await ipc.profilesDeleted()
  } catch (reason) {
    error.value = String(reason)
  } finally {
    loading.value = false
  }
}

watch(
  () => props.open,
  (open) => {
    if (open) void load()
  },
  { immediate: true }
)

async function restore(entry: FrpDeletedProfile) {
  if (busy.value) return
  busy.value = true
  error.value = ''
  try {
    const result = await ipc.profileRestore(entry.trashName, entry.managed)
    if (!result.ok) throw new Error(result.error ?? t('frp.unknownError'))
    entries.value = entries.value.filter((item) => item !== entry)
    emit('restored')
  } catch (reason) {
    error.value = String(reason)
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <UiModal :open="open" :title="t('frp.deletedTitle')" size="lg" @close="!busy && emit('close')">
    <p class="mb-[12px] text-body-sm text-text-muted dark:text-text-muted-dark">
      {{ t('frp.deletedHint') }}
    </p>
    <UiAlert v-if="error" tone="danger" class="mb-[12px]">{{ error }}</UiAlert>
    <UiSpinner v-if="loading" size="sm" />
    <UiEmptyState v-else-if="entries.length === 0 && !error" :title="t('frp.deletedEmpty')" />
    <UiScrollArea v-else class="max-h-[360px]">
      <div
        v-for="entry in entries"
        :key="`${entry.managed}:${entry.trashName}`"
        class="flex items-center gap-[12px] border-b border-border py-[10px] dark:border-border-dark"
      >
        <div class="min-w-0 flex-1">
          <div class="truncate text-body-sm dark:text-primary-dark">{{ entry.fileName }}</div>
          <div class="text-caption text-text-muted dark:text-text-muted-dark">
            {{ new Date(entry.deletedAt).toLocaleString() }}
          </div>
        </div>
        <UiButton size="sm" :disabled="busy || loading" @click="restore(entry)">{{
          t('frp.restoreProfile')
        }}</UiButton>
      </div>
    </UiScrollArea>
  </UiModal>
</template>
