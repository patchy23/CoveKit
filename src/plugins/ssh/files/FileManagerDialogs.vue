<script setup lang="ts">
import { UiConfirmDialog } from '@/core/ui'
import { UiInputDialog } from '@/core/ui'
import type { RemoteFile } from '../contracts'

defineProps<{
  renameTarget: RemoteFile | null
  deleteTarget: RemoteFile | null
}>()
const emit = defineEmits<{
  cancelRename: []
  cancelDelete: []
  rename: [name: string]
  delete: []
}>()
</script>

<template>
  <UiInputDialog
    :open="renameTarget !== null"
    title="重命名"
    label="新名称"
    :initial-value="renameTarget?.name"
    confirm-label="重命名"
    @close="emit('cancelRename')"
    @confirm="emit('rename', $event)"
  />
  <UiConfirmDialog
    :open="deleteTarget !== null"
    title="删除文件"
    :message="`确定删除「${deleteTarget?.name ?? ''}」？此操作不可恢复。`"
    confirm-label="删除"
    danger
    @close="emit('cancelDelete')"
    @confirm="emit('delete')"
  />
</template>
