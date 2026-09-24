<script setup lang="ts">
/** 编辑状态属于连接；窗口挂载于工具层，功能页和连接切换不隐式最小化。 */
import { ref, watch } from 'vue'
import type { ServerConnection } from '../contracts'
import { ipc } from '../ipc'
import { useRemoteEditor } from './useRemoteEditor'
import { useNativeEditor } from './native/useNativeEditor'
import type { EditorViewHandle } from './native/snapshot'
import RemoteEditorWorkspace from './RemoteEditorWorkspace.vue'
const props = defineProps<{
  connection: ServerConnection
  title: string
  request: { id: number; path?: string }
  rename?: { oldPath: string; newPath: string }
  directory?: string
}>()
const emit = defineEmits<{
  state: [value: { dirty: boolean; busy: boolean }]
  minimized: [value: boolean]
  returned: []
}>()
const initialized = ref(false)
const editor = useRemoteEditor(() => props.connection)
const view = ref<EditorViewHandle>()
const native = useNativeEditor(
  editor,
  view,
  () => props.connection,
  () => props.title,
  () => emit('returned')
)
watch(
  () => [editor.dirty.value.length, editor.busy.value, native.detached.value, native.moving.value],
  () =>
    emit('state', {
      dirty: !!editor.dirty.value.length,
      busy: editor.busy.value || native.detached.value || native.moving.value,
    }),
  { immediate: true }
)
watch(
  () => props.rename,
  async (value) => {
    if (value && !(await native.rename(value.oldPath, value.newPath)))
      editor.renamed(value.oldPath, value.newPath)
  }
)
watch(
  () => props.request,
  async (request) => {
    emit('minimized', false)
    if (await native.openFile(request.path)) return
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
    ref="view"
    :moving="native.moving.value"
    :ready="initialized"
    :activation="request.id"
    :editor="editor"
    :connection="connection"
    :title="title"
    @detach="native.detach"
    @minimize="emit('minimized', true)"
    @closed="emit('minimized', false)"
  />
</template>
