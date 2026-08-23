<script setup lang="ts">
import ConfirmDialog from '@/core/ui/ConfirmDialog.vue'
import InputDialog from '@/core/ui/InputDialog.vue'
import EditorDialog from './EditorDialog.vue'
import type { RemoteFile } from './contracts'

defineProps<{
  editing: { connectionId: string; path: string; content: string } | null
  saving: boolean
  renameTarget: RemoteFile | null
  deleteTarget: RemoteFile | null
}>()
const emit = defineEmits<{
  cancelEdit: []
  cancelRename: []
  cancelDelete: []
  save: [content: string]
  rename: [name: string]
  delete: []
}>()
</script>

<template>
  <EditorDialog
    v-if="editing"
    :path="editing.path"
    :content="editing.content"
    :saving="saving"
    @save="emit('save', $event)"
    @cancel="emit('cancelEdit')"
  />
  <InputDialog
    :open="renameTarget !== null"
    title="重命名"
    label="新名称"
    :initial-value="renameTarget?.name"
    confirm-label="重命名"
    @close="emit('cancelRename')"
    @confirm="emit('rename', $event)"
  />
  <ConfirmDialog
    :open="deleteTarget !== null"
    title="删除文件"
    :message="`确定删除「${deleteTarget?.name ?? ''}」？此操作不可恢复。`"
    confirm-label="删除"
    danger
    @close="emit('cancelDelete')"
    @confirm="emit('delete')"
  />
</template>
