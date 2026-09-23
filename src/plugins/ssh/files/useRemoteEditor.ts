/** 每个连接的多文件编辑状态，关闭/收起不隐式写回。 */
import { computed, onUnmounted, reactive, ref } from 'vue'
import { ipc } from '../ipc'
import type { RemoteFile, ServerConnection } from '../contracts'
import { useToolLifecycle } from '@/core/lifecycle'
export interface RemoteDocument {
  id: string
  path: string
  content: string
  saved: string
  modifiedAt?: number
  saving: boolean
  error: string
  conflict: boolean
  remoteContent?: string
}
export function useRemoteEditor(connection: () => ServerConnection | undefined) {
  const documents = ref<RemoteDocument[]>([]),
    active = ref(''),
    visible = ref(false),
    error = ref('')
  const directory = ref('/')
  let focusVersion = 0
  let disposed = false
  const pending = reactive(new Set<string>())
  const dirty = computed(() => documents.value.filter((d) => d.content !== d.saved))
  const busy = computed(() => documents.value.some((d) => d.saving) || pending.size > 0)
  const current = computed(() => documents.value.find((d) => d.path === active.value))
  async function openFile(file: Pick<RemoteFile, 'path'>) {
    const focus = ++focusVersion
    if (!documents.value.length)
      directory.value = file.path.slice(0, file.path.lastIndexOf('/')) || '/'
    visible.value = true
    const found = documents.value.find((d) => d.path === file.path)
    if (found) {
      active.value = found.path
      return
    }
    const id = connection()?.sessionId
    if (!id || connection()?.status !== 'connected' || pending.has(file.path)) return
    pending.add(file.path)
    error.value = ''
    try {
      const result = await ipc.sshEditOpen(id, file.path)
      if (disposed || connection()?.sessionId !== id) return
      if (!result.ok) throw new Error(result.error ?? '读取失败')
      if (!documents.value.some((d) => d.path === result.path))
        documents.value.push({
          id: crypto.randomUUID(),
          path: result.path,
          content: result.content,
          saved: result.content,
          modifiedAt: result.modifiedAt,
          saving: false,
          error: '',
          conflict: false,
        })
      if (focus === focusVersion) active.value = result.path
    } catch (e) {
      if (!disposed) error.value = String(e)
    } finally {
      pending.delete(file.path)
    }
  }
  async function save(doc: RemoteDocument, force = false) {
    const id = connection()?.sessionId
    if (!id || connection()?.status !== 'connected') {
      doc.error = 'SSH 未连接，修改已保留'
      return false
    }
    if (doc.saving) return false
    if (doc.conflict && !force) {
      doc.error = '请先比较远端变化，再决定覆盖或重新加载'
      return false
    }
    const content = doc.content
    doc.saving = true
    doc.error = ''
    try {
      const result = await ipc.sshEditSave(
        id,
        doc.path,
        content,
        force ? undefined : doc.modifiedAt
      )
      if (disposed || connection()?.sessionId !== id) {
        doc.error = '连接已变化，请重新核对远端内容'
        return false
      }
      if (!result.ok) {
        doc.conflict = !!result.conflict
        doc.remoteContent = result.remoteContent
        doc.error = result.error ?? (result.conflict ? '远端文件已变化，保存被阻止' : '保存失败')
        return false
      }
      // 只标记本次提交的内容；保存期间新输入仍保持 dirty。
      doc.saved = content
      doc.conflict = false
      const fresh = await ipc.sshEditOpen(id, doc.path)
      if (disposed || connection()?.sessionId !== id) return false
      if (fresh.ok && fresh.content === content) {
        doc.modifiedAt = fresh.modifiedAt
        return true
      }
      doc.conflict = true
      doc.remoteContent = fresh.ok ? fresh.content : undefined
      doc.error = '文件已写入，但无法确认最新版本；再次保存前请核对远端'
      return false
    } catch (e) {
      doc.error = String(e)
      return false
    } finally {
      doc.saving = false
    }
  }
  async function saveAll() {
    let success = true
    for (const doc of [...dirty.value]) if (!(await save(doc))) success = false
    return success
  }
  async function reload(doc: RemoteDocument) {
    const id = connection()?.sessionId
    if (!id || connection()?.status !== 'connected' || doc.saving) return false
    const beforeReload = doc.content
    doc.saving = true
    try {
      const fresh = await ipc.sshEditOpen(id, doc.path)
      if (disposed || connection()?.sessionId !== id) return false
      if (!fresh.ok) throw new Error(fresh.error ?? '读取失败')
      if (doc.content === beforeReload) doc.content = fresh.content
      doc.saved = fresh.content
      doc.modifiedAt = fresh.modifiedAt
      doc.error = ''
      doc.conflict = false
      doc.remoteContent = undefined
      return true
    } catch (e) {
      doc.error = String(e)
      return false
    } finally {
      doc.saving = false
    }
  }
  function renamed(oldPath: string, newPath: string) {
    for (const doc of documents.value)
      if (doc.path === oldPath || doc.path.startsWith(oldPath + '/')) {
        const next = newPath + doc.path.slice(oldPath.length)
        if (active.value === doc.path) active.value = next
        doc.path = next
      }
  }
  function remove(path: string) {
    documents.value = documents.value.filter((d) => d.path !== path)
    if (active.value === path) active.value = documents.value.at(-1)?.path ?? ''
  }
  useToolLifecycle('ssh', {
    owner: 'ssh.editor.' + (connection()?.sessionId ?? crypto.randomUUID()),
    prepare: () =>
      dirty.value.length || busy.value
        ? '远程编辑器有 ' + dirty.value.length + ' 个未保存文件或保存任务'
        : null,
    dispose: async () => {
      disposed = true
      documents.value = []
    },
  })
  onUnmounted(() => {
    disposed = true
  })
  function hide() {
    visible.value = false
  }
  function activate(path: string) {
    focusVersion++
    active.value = path
  }
  return {
    directory,
    hide,
    activate,
    documents,
    active,
    visible,
    error,
    dirty,
    busy,
    current,
    openFile,
    save,
    saveAll,
    remove,
    reload,
    renamed,
  }
}
