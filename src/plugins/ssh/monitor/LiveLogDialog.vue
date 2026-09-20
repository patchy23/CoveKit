<script setup lang="ts">
/** 服务与容器日志查看器：定时拉取尾部日志，并在跟随模式下自动滚动到底部。 */
import { onMounted, onUnmounted, ref } from 'vue'
import { UiFloatingWindow, UiLogViewer } from '@/core/ui'
import { useCopy } from '@/core/feedback/useCopy'
import { ipc } from '../ipc'

const props = defineProps<{
  activation?: number
  title: string
  connectionId: string
  kind: 'service' | 'docker'
  targetId: string
}>()
defineEmits<{ (event: 'close'): void }>()

const content = ref('')
const errorMessage = ref('')
const loading = ref(false)
const { copyText } = useCopy()
const lineLimit = ref(300)
let timer: number | null = null
let disposed = false

function changeLineLimit(value: number) {
  lineLimit.value = value
  void refresh()
}

async function refresh() {
  if (loading.value || disposed) return
  const lines = lineLimit.value
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
    if (disposed || lines !== lineLimit.value) return
    if (!result.ok) throw new Error(result.error ?? '日志读取失败')
    content.value = result.logs
    errorMessage.value = ''
  } catch (error) {
    if (!disposed && lines === lineLimit.value) errorMessage.value = String(error)
  } finally {
    loading.value = false
    // 请求过程中切换档位：忽略旧结果，串行补拉最新档位，避免并发覆盖。
    if (!disposed && lines !== lineLimit.value) void refresh()
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
  <UiFloatingWindow :title="title" :activation="activation" @close="$emit('close')">
    <UiLogViewer
      :content="content"
      :loading="loading"
      :error="errorMessage"
      @limit-change="changeLineLimit"
      @refresh="refresh"
      @copy="copyText($event)"
    />
  </UiFloatingWindow>
</template>
