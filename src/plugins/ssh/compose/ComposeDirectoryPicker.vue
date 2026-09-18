<script setup lang="ts">
/** 仅浏览用户指定目录，不用于发现 Compose 项目。 */
import { onMounted, onUnmounted, ref } from 'vue'
import { UiButton, UiIcon, UiIconButton, UiInput, UiModal, UiScrollArea } from '@/core/ui'
import type { FileListResult } from '../contracts'
import { ipc } from '../ipc'
const props = defineProps<{ connectionId: string; initialPath: string }>()
const emit = defineEmits<{ close: []; select: [path: string] }>()
const path = ref(props.initialPath)
const listing = ref<FileListResult>()
const error = ref('')
const loading = ref(false)
let version = 0
onUnmounted(() => {
  version++
})
async function browse(target: string) {
  const request = ++version
  loading.value = true
  error.value = ''
  try {
    const result = await ipc.sshFileList(props.connectionId, target)
    if (request !== version) return
    if (!result.ok) throw new Error(result.error ?? '无法读取目录')
    listing.value = result
    path.value = result.path
  } catch (e) {
    if (request === version) {
      listing.value = undefined
      error.value = String(e)
    }
  } finally {
    if (request === version) loading.value = false
  }
}
onMounted(() => {
  void browse(props.initialPath)
})
</script>

<template>
  <UiModal open title="选择远程项目目录" @close="emit('close')">
    <div class="mb-sm flex gap-sm">
      <UiIconButton
        label="上级"
        title="上级目录"
        size="sm"
        :disabled="loading || !listing?.parentPath"
        @click="listing?.parentPath && browse(listing.parentPath)"
      >
        <UiIcon name="arrow-up" :size="14" />
      </UiIconButton>
      <UiInput
        v-model="path"
        class="min-w-0 flex-1"
        placeholder="远程绝对路径，回车前往"
        @keydown.enter="browse(path)"
      />
    </div>
    <p v-if="error" role="alert" class="text-body-sm text-danger-strong dark:text-danger-dark">
      {{ error }}
    </p>
    <UiScrollArea class="h-[260px]" axis="vertical">
      <UiButton
        v-for="entry in listing?.files.filter((f) => f.isDir && f.name !== '.' && f.name !== '..')"
        :key="entry.path"
        variant="ghost"
        block
        class="!justify-start"
        @click="browse(entry.path)"
        >{{ entry.name }}</UiButton
      >
    </UiScrollArea>
    <template #footer>
      <UiButton variant="ghost" @click="emit('close')">取消</UiButton>
      <UiButton
        :disabled="loading || !listing || path !== listing.path"
        @click="listing && emit('select', listing.path)"
        >选择此目录</UiButton
      >
    </template>
  </UiModal>
</template>
