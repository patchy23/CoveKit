<script setup lang="ts">
/** systemd 主配置与 drop-in 原文，只读查看，旧请求不得写入新的服务或连接。 */
import { onBeforeUnmount, ref, watch } from 'vue'
import { UiAlert, UiButton, UiCodeEditor, UiModal } from '@/core/ui'
import { ipc } from '../ipc'

const props = defineProps<{ connectionId: string; serviceName: string }>()
const emit = defineEmits<{ close: [] }>()
const content = ref('')
const error = ref('')
const loading = ref(false)
let sequence = 0

async function load() {
  const current = ++sequence
  content.value = ''
  error.value = ''
  loading.value = true
  try {
    const value = await ipc.sshServiceConfig({
      connectionId: props.connectionId,
      serviceName: props.serviceName,
    })
    if (current === sequence) content.value = value
  } catch (reason) {
    if (current === sequence) error.value = `读取服务配置失败：${String(reason)}`
  } finally {
    if (current === sequence) loading.value = false
  }
}

watch(() => [props.connectionId, props.serviceName], load, { immediate: true })
onBeforeUnmount(() => {
  sequence += 1
})
</script>

<template>
  <UiModal :open="true" :title="`${serviceName} · 服务配置`" size="xl" @close="emit('close')">
    <p class="mb-3 text-body-sm text-text-muted">
      只读显示 systemd 主 unit 与 drop-in 覆盖配置，文件来源保留在原文注释中。
    </p>
    <UiAlert v-if="error" tone="danger">{{ error }}</UiAlert>
    <p v-else-if="loading" class="text-body-sm text-text-muted">正在读取配置…</p>
    <UiCodeEditor
      v-else
      :model-value="content"
      readonly
      language="text"
      :filename="serviceName"
      height="min(60vh, 560px)"
    />
    <template #footer>
      <UiButton variant="ghost" :loading="loading" @click="load">重新读取</UiButton>
      <UiButton variant="secondary" @click="emit('close')">关闭</UiButton>
    </template>
  </UiModal>
</template>
