<script setup lang="ts">
import { nextTick, onMounted, onUnmounted, ref, watch } from 'vue'
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow'
import { UiAlert, UiButton, UiSpinner } from '@/core/ui'
import Toast from '@/features/ui/Toast.vue'
import { useSettingsStore } from '@/stores/settings'
import type { ServerConnection } from '../../contracts'
import { ipc } from '../../ipc'
import RemoteEditorWorkspace from '../RemoteEditorWorkspace.vue'
import { useRemoteEditor } from '../useRemoteEditor'
import { editorChannel } from './channel'
import { applyEditor, captureEditor, editorUpdate, type EditorViewHandle } from './snapshot'
import type { EditorInitial, EditorSnapshot } from './protocol'

const token = new URLSearchParams(window.location.search).get('sshEditor') ?? ''
const connection = ref<ServerConnection>()
const title = ref('SSH 文件编辑器'),
  ready = ref(false),
  moving = ref(false),
  failure = ref(''),
  handedOff = ref(false)
const view = ref<EditorViewHandle>()
const editor = useRemoteEditor(() => connection.value)
const settings = useSettingsStore()
let channel: Awaited<ReturnType<typeof editorChannel>> | undefined
let stopClose: (() => void) | undefined
let timer: ReturnType<typeof setTimeout> | undefined
let acknowledged: EditorSnapshot | undefined
let sequence = 0,
  disposed = false,
  sending = false,
  changed = false
function error(value: unknown) {
  failure.value = String(value)
}
function snapshot() {
  return captureEditor(editor, ++sequence, view.value?.captureLayout())
}
function schedule() {
  changed = true
  if (!ready.value || disposed || moving.value || sending || timer) return
  timer = setTimeout(() => {
    timer = undefined
    if (!channel || moving.value || disposed) return
    sending = true
    changed = false
    const current = snapshot()
    void channel
      .request('patch', editorUpdate(current, acknowledged))
      .then(() => {
        acknowledged = current
      })
      .catch(error)
      .finally(() => {
        sending = false
        if (changed) schedule()
      })
  }, 80)
}
watch([editor.documents, editor.active, editor.directory, editor.error], schedule, { deep: true })
async function finish(type: 'dock' | 'closed') {
  if (!channel || moving.value || editor.busy.value) return
  moving.value = true
  clearTimeout(timer)
  timer = undefined
  try {
    await channel.request(type, snapshot())
    handedOff.value = true
    // 收到主窗口接收确认后再销毁，移回失败时原窗口和草稿仍在。
    await ipc.sshEditorWindow(token, 'close')
  } catch (e) {
    editor.visible.value = true
    error(e)
    // 回执失败可能是结果未知，先确认编辑权，不能直接恢复双端编辑。
    await recoverOwnership()
  }
}
async function recoverOwnership() {
  if (!channel) return
  try {
    const owner = await channel.request<string>('ownership')
    handedOff.value = owner === 'main'
    if (handedOff.value) await ipc.sshEditorWindow(token, 'close')
    else {
      moving.value = false
      failure.value = ''
      editor.visible.value = true
      schedule()
    }
  } catch (e) {
    error(e)
  }
}
onMounted(async () => {
  try {
    const native = getCurrentWebviewWindow()
    if (native.label !== 'ssh-editor-' + token) throw new Error('编辑器窗口身份不匹配')
    void settings.init().catch(error)
    stopClose = await native.onCloseRequested((event) => {
      event.preventDefault()
      if (!ready.value) {
        void ipc.sshEditorWindow(token, 'close').catch(error)
        return
      }
      if (!moving.value) view.value?.requestClose()
    })
    channel = await editorChannel(
      token,
      'main',
      async (type, value) => {
        if (disposed) throw new Error('编辑器已关闭')
        if (type === 'capture') return snapshot()
        if (type === 'connection') {
          connection.value = value as ServerConnection
          return
        }
        if (moving.value) throw new Error('编辑器正在移回主窗口')
        if (type === 'open') {
          await editor.openFile({ path: String(value) })
          return
        }
        if (type === 'rename') {
          const rename = value as { oldPath: string; newPath: string }
          editor.renamed(rename.oldPath, rename.newPath)
          return
        }
        throw new Error('未知编辑器操作')
      },
      error
    )
    const initial = await channel.request<EditorInitial>('ready')
    if (disposed) return
    connection.value = initial.connection
    title.value = initial.title
    sequence = initial.snapshot.sequence
    for (const doc of initial.snapshot.documents) {
      if (doc.saving) {
        doc.saving = false
        doc.error = '上次保存结果未确认，请核对远端内容后再保存'
      }
    }
    applyEditor(editor, initial.snapshot)
    editor.visible.value = true
    ready.value = true
    await nextTick()
    await view.value?.restoreLayout(initial.snapshot.layout)
    await channel.request('mounted')
    schedule()
  } catch (e) {
    error(e)
  }
})
onUnmounted(() => {
  disposed = true
  clearTimeout(timer)
  stopClose?.()
  channel?.dispose()
  settings.unsubscribeSystemTheme()
  settings.$dispose()
})
</script>
<template>
  <div
    class="flex h-screen min-h-0 flex-col bg-surface text-primary dark:bg-surface-dark dark:text-primary-dark"
  >
    <UiAlert v-if="failure" tone="danger" class="shrink-0"
      >{{ failure }}
      <UiButton v-if="moving" size="sm" variant="ghost" @click="recoverOwnership"
        >重新确认窗口状态</UiButton
      >
    </UiAlert>
    <UiSpinner v-if="!ready" label="正在接入 SSH 编辑器" />
    <RemoteEditorWorkspace
      v-if="ready"
      ref="view"
      :editor="editor"
      :connection="connection"
      :title="title"
      standalone
      :moving="moving"
      @dock="finish('dock')"
      @closed="finish('closed')"
    />
    <Toast />
  </div>
</template>
