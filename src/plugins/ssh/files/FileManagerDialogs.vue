<script setup lang="ts">
import { UiConfirmDialog } from '@/core/ui'
import { UiInputDialog } from '@/core/ui'
import EditorDialog from './EditorDialog.vue'
import type { RemoteFile } from '../contracts'

defineProps<{
  editing: {
    connectionId: string
    path: string
    content: string
    modifiedAt?: number
    conflict?: boolean
    /** 冲突时远端当前内容（供差异对比） */
    remoteContent?: string
  } | null
  saving: boolean
  renameTarget: RemoteFile | null
  deleteTarget: RemoteFile | null
}>()
const emit = defineEmits<{
  cancelEdit: []
  cancelRename: []
  cancelDelete: []
  save: [content: string, force?: boolean]
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
    :conflict="editing.conflict"
    :remote-content="editing.remoteContent"
    @save="(content, force) => emit('save', content, force)"
    @cancel="emit('cancelEdit')"
  />
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
