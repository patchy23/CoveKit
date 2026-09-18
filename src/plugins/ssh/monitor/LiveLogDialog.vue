<script setup lang="ts">
/** 服务与容器日志查看器：定时拉取尾部日志，并在跟随模式下自动滚动到底部。 */
import { nextTick, onMounted, onUnmounted, ref } from 'vue'
import { UiButton, UiModal, UiScrollArea, UiSelect, UiSwitch, UiToolbar } from '@/core/ui'
import { ipc } from '../ipc'

const props = defineProps<{
  title: string
  connectionId: string
  kind: 'service' | 'docker'
  targetId: string
}>()
defineEmits<{ (event: 'close'): void }>()

const content = ref('')
const errorMessage = ref('')
const loading = ref(false)
const following = ref(true)
/** 固定档位与后端 2000 行上限一致，不开放无限量拉取。 */
const lineOptions = [100, 300, 500, 1000, 2000].map((lines) => ({
  value: String(lines),
  label: `最近 ${lines} 行`,
}))
const lineLimit = ref('300')
const output = ref<HTMLElement | null>(null)
let timer: number | null = null
let disposed = false

function changeLineLimit(value: string) {
  if (!lineOptions.some((option) => option.value === value) || value === lineLimit.value) return
  lineLimit.value = value
  content.value = tailLines(content.value, Number(value))
  void refresh()
}

function tailLines(logs: string, limit: number) {
  return logs
    .replace(/\r?\n$/, '')
    .split('\n')
    .slice(-limit)
    .join('\n')
}

async function refresh() {
  if (loading.value || disposed) return
  const lines = Number(lineLimit.value)
  loading.value = true
  try {
    const result =
      props.kind === 'service'
        ? await ipc.sshServiceLogs({
            connectionId: props.connectionId,
            serviceName: props.targetId,
            lines,
          })
        : await ipc.sshDockerLogs({
            connectionId: props.connectionId,
            containerId: props.targetId,
            lines,
          })
    if (disposed || lines !== Number(lineLimit.value)) return
    if (!result.ok) throw new Error(result.error ?? '日志读取失败')
    content.value = tailLines(result.logs, lines)
    errorMessage.value = ''
    if (following.value) {
      await nextTick()
      output.value?.scrollTo({ top: output.value.scrollHeight })
    }
  } catch (error) {
    if (!disposed && lines === Number(lineLimit.value)) errorMessage.value = String(error)
  } finally {
    loading.value = false
    // 请求过程中切换档位：忽略旧结果，串行补拉最新档位，避免并发覆盖。
    if (!disposed && lines !== Number(lineLimit.value)) void refresh()
  }
}

onMounted(() => {
  void refresh()
  timer = window.setInterval(refresh, 2000)
})

onUnmounted(() => {
  disposed = true
  if (timer) clearInterval(timer)
})
</script>

<template>
  <UiModal :open="true" :title="title" size="full" width="1040px" @close="$emit('close')">
    <div class="flex min-h-0 min-w-0 flex-1 flex-col gap-sm overflow-hidden p-md">
      <UiToolbar class="text-body-sm">
        <UiSwitch v-model="following" size="sm" label="自动跟随" />
        <UiSelect
          :model-value="lineLimit"
          :options="lineOptions"
          size="sm"
          title="最多显示行数"
          @update:model-value="changeLineLimit"
        />
        <span class="text-text-muted dark:text-text-muted-dark">每 2 秒更新</span>
        <span
          v-if="errorMessage"
          class="min-w-0 break-all text-danger-strong dark:text-danger-dark"
        >
          {{ errorMessage }}
        </span>
      </UiToolbar>
      <UiScrollArea as-child axis="both" theme="dark">
        <pre
          ref="output"
          class="min-h-0 min-w-0 flex-1 whitespace-pre-wrap rounded-md bg-[#0d1117] p-[14px] font-mono text-body-sm text-[#e6edf3]"
          @wheel="following = false"
          >{{ content || (loading ? '正在读取日志…' : '（没有输出）') }}</pre>
      </UiScrollArea>
    </div>
    <template #footer>
      <UiButton variant="ghost" :loading="loading" @click="refresh">立即刷新</UiButton>
      <UiButton variant="secondary" @click="$emit('close')">关闭</UiButton>
    </template>
  </UiModal>
</template>
