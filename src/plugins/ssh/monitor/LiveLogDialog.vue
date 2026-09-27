<script setup lang="ts">
/** 服务与容器日志查看器：定时拉取尾部日志，并在跟随模式下自动滚动到底部。 */
import { onMounted, onUnmounted, ref, watch } from 'vue'
import { UiFloatingWindow, UiLogViewer } from '@/core/ui'
import { useCopy } from '@/core/feedback/useCopy'
import { readLogSnapshot } from './logRequests'

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
let fingerprint: string | undefined
let fingerprintKey = ''
const sourceKey = () =>
  JSON.stringify([props.connectionId, props.kind, props.targetId, lineLimit.value])
watch(
  () => [props.connectionId, props.kind, props.targetId],
  () => {
    fingerprint = undefined
    fingerprintKey = ''
    content.value = ''
    errorMessage.value = ''
    void refresh()
  }
)

function changeLineLimit(value: number) {
  lineLimit.value = value
  void refresh()
}

async function refresh() {
  if (loading.value || disposed) return
  const lines = lineLimit.value
  const key = sourceKey()
  const previousFingerprint = fingerprintKey === key ? fingerprint : undefined
  loading.value = true
  try {
    const result = await readLogSnapshot(
      props.kind,
      props.connectionId,
      props.targetId,
      lines,
      previousFingerprint
    )
    if (disposed || key !== sourceKey()) return
    if (!result.ok) throw new Error(result.error ?? '日志读取失败')
    if (result.unchanged) {
      if (!previousFingerprint || result.fingerprint !== previousFingerprint) {
        fingerprint = undefined
        throw new Error('日志响应基线不一致，下次刷新将重新读取完整内容')
      }
    } else if (typeof result.logs === 'string') content.value = result.logs
    else {
      fingerprint = undefined
      throw new Error('日志响应缺少正文，下次刷新将重新读取完整内容')
    }
    fingerprint = result.fingerprint
    fingerprintKey = key
    errorMessage.value = ''
  } catch (error) {
    if (!disposed && key === sourceKey()) errorMessage.value = String(error)
  } finally {
    loading.value = false
    // 请求过程中切换档位：忽略旧结果，串行补拉最新档位，避免并发覆盖。
    if (!disposed && key !== sourceKey()) void refresh()
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
