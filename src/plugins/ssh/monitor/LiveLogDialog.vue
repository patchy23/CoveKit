<script setup lang="ts">
/** 服务与容器日志查看器：定时拉取尾部日志，并在跟随模式下自动滚动到底部。 */
import { nextTick, onMounted, onUnmounted, ref } from 'vue'
import { UiButton, UiModal, UiSwitch } from '@/core/ui'
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
const output = ref<HTMLElement | null>(null)
let timer: number | null = null

async function refresh() {
  if (loading.value) return
  loading.value = true
  try {
    const result =
      props.kind === 'service'
        ? await ipc.sshServiceLogs({
            connectionId: props.connectionId,
            serviceName: props.targetId,
            lines: 300,
          })
        : await ipc.sshDockerLogs({
            connectionId: props.connectionId,
            containerId: props.targetId,
            lines: 300,
          })
    if (!result.ok) throw new Error(result.error ?? '日志读取失败')
    content.value = result.logs
    errorMessage.value = ''
    if (following.value) {
      await nextTick()
      output.value?.scrollTo({ top: output.value.scrollHeight })
    }
  } catch (error) {
    errorMessage.value = String(error)
  } finally {
    loading.value = false
  }
}

onMounted(() => {
  void refresh()
  timer = window.setInterval(refresh, 2000)
})

onUnmounted(() => {
  if (timer) clearInterval(timer)
})
</script>

<template>
  <UiModal :open="true" :title="title" size="xl" @close="$emit('close')">
    <div class="mb-[8px] flex items-center gap-[8px] text-body-sm">
      <UiSwitch v-model="following" size="sm" label="自动跟随" />
      <span class="text-text-muted dark:text-text-muted-dark">每 2 秒更新最近 300 行</span>
      <span v-if="errorMessage" class="ml-auto text-danger-strong dark:text-danger-dark">
        {{ errorMessage }}
      </span>
    </div>
    <pre
      ref="output"
      class="max-h-[65vh] min-h-[420px] overflow-auto whitespace-pre-wrap rounded-md bg-[#0d1117] p-[14px] font-mono text-body-sm text-[#e6edf3]"
      @wheel="following = false"
      >{{ content || (loading ? '正在读取日志…' : '（没有输出）') }}</pre>
    <template #footer>
      <UiButton variant="ghost" :loading="loading" @click="refresh">立即刷新</UiButton>
      <UiButton variant="secondary" @click="$emit('close')">关闭</UiButton>
    </template>
  </UiModal>
</template>
