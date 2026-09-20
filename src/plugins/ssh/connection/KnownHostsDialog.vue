<script setup lang="ts">
import { UiScrollArea } from '@/core/ui'
import { UiTooltip } from '@/core/ui'
/**
 * KnownHostsDialog · 已知主机管理
 * 列出 CoveKit 私有 known_hosts 的条目（算法 + SHA256 指纹），支持删除单条/整台主机。
 * 服务器重装等合法变更也可在此删除旧指纹后重连。
 */
import { ref, watch } from 'vue'
import type { KnownHostEntry } from '../contracts'
import { ipc } from '../ipc'
import { UiButton, UiListRow, UiModal } from '@/core/ui'

const props = defineProps<{
  open: boolean
}>()

const emit = defineEmits<{
  (e: 'close'): void
}>()

const entries = ref<KnownHostEntry[]>([])
const loaded = ref(false)
const loadFailed = ref(false)

async function load() {
  loaded.value = false
  try {
    entries.value = await ipc.sshKnownHostList()
    loadFailed.value = false
  } catch {
    loadFailed.value = true
  } finally {
    loaded.value = true
  }
}

async function remove(entry: KnownHostEntry) {
  try {
    const result = await ipc.sshKnownHostDelete({
      host: entry.host,
      port: entry.port,
      fingerprint: entry.fingerprint,
    })
    if (result.ok) entries.value = entries.value.filter((e) => e !== entry)
  } catch {
    /* 删除失败保持列表不变 */
  }
  if (entries.value.length === 0) await load()
}

watch(
  () => props.open,
  (open) => {
    if (open) void load()
  },
  { immediate: true }
)

defineExpose({ load })
</script>

<template>
  <UiModal :open="open" title="已知主机" width="min(560px, 92vw)" @close="emit('close')">
    <p class="mb-[10px] text-body-sm text-secondary dark:text-secondary-dark">
      首次连接时保存的主机指纹。连接时若服务器指纹与此处不一致会被阻断；服务器重装后可删除对应条目再重连。
    </p>

    <div
      v-if="loadFailed"
      class="select-text py-[16px] text-center text-body-sm text-danger-strong dark:text-danger-dark"
    >
      已知主机列表加载失败，请重试。
    </div>
    <div
      v-else-if="loaded && entries.length === 0"
      class="py-[16px] text-center text-body-sm text-text-muted dark:text-text-muted-dark"
    >
      暂无已知主机（首次连接服务器并选择「保存并连接」后出现在这里）。
    </div>

    <UiScrollArea v-else as-child axis="vertical">
      <div class="max-h-[380px] rounded-md border border-border dark:border-border-dark">
        <UiTooltip
          v-for="entry in entries"
          :key="`${entry.host}:${entry.port}:${entry.fingerprint}`"
          :content="entry.fingerprint"
        >
          <UiListRow size="sm">
            <div class="flex min-w-0 flex-1 select-text items-center gap-[10px]">
              <span class="font-mono text-body-sm text-primary dark:text-primary-dark">
                {{ entry.host }}<span v-if="entry.port !== 22">:{{ entry.port }}</span>
              </span>
              <span class="shrink-0 text-caption text-text-muted dark:text-text-muted-dark">
                {{ entry.algorithm }}
              </span>
              <span
                class="min-w-0 flex-1 truncate font-mono text-caption text-text-muted dark:text-text-muted-dark"
              >
                {{ entry.fingerprint }}
              </span>
            </div>
            <UiButton
              variant="ghost"
              size="xs"
              class="!h-auto !px-[8px] !py-[3px] text-caption text-danger-strong dark:text-danger-dark"
              @click="remove(entry)"
            >
              删除
            </UiButton>
          </UiListRow>
        </UiTooltip>
      </div>
    </UiScrollArea>

    <template #footer>
      <UiButton variant="ghost" @click="load()">刷新</UiButton>
      <UiButton variant="primary" @click="emit('close')">完成</UiButton>
    </template>
  </UiModal>
</template>
