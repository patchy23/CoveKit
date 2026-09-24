<script setup lang="ts">
/** 编辑器属于连接；首次使用才加载，收起和功能切换保留文档。 */
import { ref, watch } from 'vue'
import type { ServerConnection } from '../contracts'
import { ipc } from '../ipc'
import { useRemoteEditor } from './useRemoteEditor'
import RemoteEditorWorkspace from './RemoteEditorWorkspace.vue'
const props = defineProps<{
  connection: ServerConnection
  title: string
  request: { id: number; path?: string }
  rename?: { oldPath: string; newPath: string }
  directory?: string
  location: string
}>()
const emit = defineEmits<{
  state: [value: { dirty: boolean; busy: boolean }]
  minimized: [value: boolean]
}>()
const initialized = ref(false)
const editor = useRemoteEditor(() => props.connection)
watch(
  () => [editor.dirty.value.length, editor.busy.value],
  () => emit('state', { dirty: !!editor.dirty.value.length, busy: editor.busy.value }),
  { immediate: true }
)
watch(
  () => props.location,
  () => {
    if (editor.visible.value) {
      editor.hide()
      emit('minimized', true)
    }
  }
)
watch(
  () => props.rename,
  (value) => {
    if (value) editor.renamed(value.oldPath, value.newPath)
  }
)
watch(
  () => props.request,
  async (request) => {
    emit('minimized', false)
    if (request.path) {
      initialized.value = true
      await editor.openFile({ path: request.path })
      return
    }
    editor.visible.value = true
    if (editor.documents.value.length) return
    if (props.directory) {
      editor.directory.value = props.directory
      initialized.value = true
      return
    }
    const sessionId = props.connection.sessionId
    if (props.connection.status !== 'connected') {
      editor.error.value = 'SSH 未连接'
      initialized.value = true
      return
    }
    try {
      const home = await ipc.sshComposeHome(sessionId)
      if (
        props.connection.sessionId === sessionId &&
        props.request.id === request.id &&
        !editor.documents.value.length
      )
        editor.directory.value = home
    } catch (error) {
      if (props.request.id === request.id) editor.error.value = '读取初始目录失败：' + String(error)
    } finally {
      if (props.request.id === request.id) initialized.value = true
    }
  },
  { immediate: true }
)
</script>
<template>
  <RemoteEditorWorkspace
    :ready="initialized"
    :activation="request.id"
    :editor="editor"
    :connection="connection"
    :title="title"
    @minimize="emit('minimized', true)"
    @closed="emit('minimized', false)"
  />
</template>
