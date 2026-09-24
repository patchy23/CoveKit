/** 仅 SSH 编辑器的宿主迁移；后台连接仍由主窗口管理。 */
import { nextTick, onUnmounted, ref, watch, type Ref } from 'vue'
import { isTauri } from '@tauri-apps/api/core'
import { WebviewWindow } from '@tauri-apps/api/webviewWindow'
import { useToolLifecycle } from '@/core/lifecycle'
import { ipc } from '../../ipc'
import type { ServerConnection } from '../../contracts'
import { editorChannel } from './channel'
import {
  acceptSnapshot,
  type EditorInitial,
  type EditorSnapshot,
  type EditorUpdate,
} from './protocol'
import {
  applyEditor,
  captureEditor,
  mergeEditorUpdate,
  type EditorViewHandle,
  type RemoteEditor,
} from './snapshot'

export function useNativeEditor(
  editor: RemoteEditor,
  view: Ref<EditorViewHandle | undefined>,
  connection: () => ServerConnection,
  title: () => string,
  onReturn: () => void = () => {}
) {
  const detached = ref(false),
    moving = ref(false)
  let token = '',
    sequence = -1,
    disposed = false
  let channel: Awaited<ReturnType<typeof editorChannel>> | undefined
  let stopDestroyed: (() => void) | undefined
  let intentional = false
  let pendingOpen: string | undefined,
    pendingFocus = false
  const pendingRenames: { oldPath: string; newPath: string }[] = []
  let ready: (() => void) | undefined
  function error(value: unknown) {
    editor.error.value = '独立编辑器：' + String(value)
  }
  function apply(snapshot: EditorSnapshot) {
    if (!acceptSnapshot(sequence, snapshot)) return
    sequence = snapshot.sequence
    applyEditor(editor, snapshot)
  }
  async function cleanup() {
    const oldToken = token
    if (!oldToken) return
    intentional = true
    await ipc.sshEditorWindow(oldToken, 'close')
    stopDestroyed?.()
    stopDestroyed = undefined
    channel?.dispose()
    channel = undefined
    token = ''
    detached.value = false
    moving.value = false
  }
  async function focus() {
    if (token) await ipc.sshEditorWindow(token, 'focus')
  }
  async function detach() {
    if (disposed || moving.value || editor.busy.value) return
    if (token) {
      await focus().catch(error)
      return
    }
    if (!isTauri()) {
      error('当前环境不支持系统窗口')
      return
    }
    moving.value = true
    intentional = false
    token = crypto.randomUUID()
    const openingToken = token
    sequence = -1
    let timer: ReturnType<typeof setTimeout> | undefined
    try {
      const initialized = new Promise<void>((resolve, reject) => {
        ready = resolve
        timer = setTimeout(() => reject(new Error('窗口初始化超时')), 15000)
      })
      // 在可能耗时的创建调用前挂好 rejection，避免孤立超时。
      void initialized.catch(() => {})
      channel = await editorChannel(
        token,
        'ssh-editor-' + token,
        async (type, value) => {
          if (disposed) throw new Error('SSH 工具已关闭')
          if (type === 'ready')
            return {
              snapshot: captureEditor(editor, ++sequence, view.value?.captureLayout()),
              connection: connection(),
              title: title(),
            } satisfies EditorInitial
          if (type === 'mounted') {
            detached.value = true
            editor.hide()
            await ipc.sshEditorWindow(token, 'show')
            ready?.()
            return
          }
          if (type === 'ownership') return detached.value ? 'child' : 'main'
          if (type === 'patch') {
            const update = value as EditorUpdate
            if (acceptSnapshot(sequence, update)) apply(mergeEditorUpdate(editor, update))
            return
          }
          if (type === 'state') {
            apply(value as EditorSnapshot)
            return
          }
          if (type === 'dock' || type === 'closed') {
            intentional = true
            apply(value as EditorSnapshot)
            detached.value = false
            editor.visible.value = type === 'dock'
            await nextTick()
            if (type === 'dock') {
              onReturn()
              await view.value?.restoreLayout((value as EditorSnapshot).layout)
              await ipc.sshEditorWindow(token, 'return')
            }
            return
          }
          throw new Error('未知编辑窗口消息')
        },
        error
      )
      if (disposed) throw new Error('SSH 工具已关闭')
      await ipc.sshEditorWindow(openingToken, 'open')
      if (disposed || token !== openingToken) {
        await ipc.sshEditorWindow(openingToken, 'close')
        throw new Error('SSH 工具已关闭')
      }
      const native = await WebviewWindow.getByLabel('ssh-editor-' + token)
      if (!native) throw new Error('未找到已创建的编辑窗口')
      stopDestroyed = await native.once('tauri://destroyed', () => {
        channel?.dispose()
        channel = undefined
        token = ''
        detached.value = false
        moving.value = false
        if (!intentional && !disposed) {
          editor.visible.value = true
          error('窗口意外关闭，已恢复最近同步的草稿')
        }
      })
      await initialized
      await channel?.request('connection', connection())
      for (const rename of pendingRenames.splice(0)) await channel?.request('rename', rename)
      moving.value = false
      if (pendingFocus) await openFile(pendingOpen)
      pendingFocus = false
      pendingOpen = undefined
    } catch (e) {
      await cleanup().catch(error)
      editor.visible.value = true
      error(e)
    } finally {
      clearTimeout(timer)
      ready = undefined
      moving.value = false
    }
  }
  async function openFile(path?: string) {
    if (moving.value) {
      pendingOpen = path
      pendingFocus = true
      return true
    }
    if (!channel || !detached.value) return false
    try {
      if (path) await channel.request('open', path)
      await focus()
    } catch (e) {
      error(e)
    }
    return true
  }
  watch(
    connection,
    (value) => {
      if (channel && detached.value) void channel.request('connection', value).catch(error)
    },
    { deep: true }
  )
  async function rename(oldPath: string, newPath: string) {
    if (moving.value) {
      pendingRenames.push({ oldPath, newPath })
      return true
    }
    if (channel && detached.value) {
      await channel.request('rename', { oldPath, newPath }).catch(error)
      return true
    }
    return false
  }
  useToolLifecycle('ssh', {
    owner: 'ssh.editor.window.' + crypto.randomUUID(),
    prepare: async () => {
      if (moving.value) return 'SSH 编辑器正在移动窗口'
      if (!channel || !detached.value) return null
      try {
        const snapshot = await channel.request<EditorSnapshot>('capture')
        apply(snapshot)
        return snapshot.documents.some((doc) => doc.content !== doc.saved || doc.saving)
          ? '独立 SSH 编辑窗口有未保存文件或保存任务'
          : null
      } catch {
        return '无法确认独立编辑窗口的草稿，请先移回或关闭编辑器'
      }
    },
    dispose: async () => {
      disposed = true
      await cleanup()
    },
  })
  onUnmounted(() => {
    disposed = true
    void cleanup().catch((e) => console.warn('清理 SSH 编辑窗口失败', e))
  })
  return { detached, moving, detach, openFile, rename }
}
