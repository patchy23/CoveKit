import type { useRemoteEditor } from '../useRemoteEditor'
import type {
  EditorDocumentUpdate,
  EditorLayout,
  EditorSnapshot,
  EditorTextChange,
  EditorUpdate,
} from './protocol'
export type RemoteEditor = ReturnType<typeof useRemoteEditor>
export interface EditorViewHandle {
  captureLayout: () => EditorLayout
  restoreLayout: (layout?: EditorLayout) => Promise<void>
  requestClose: () => void
}
export function captureEditor(
  editor: RemoteEditor,
  sequence: number,
  layout?: EditorLayout
): EditorSnapshot {
  return {
    sequence,
    documents: editor.documents.value.map((doc) => ({ ...doc })),
    active: editor.active.value,
    directory: editor.directory.value,
    error: editor.error.value,
    layout,
  }
}
export function applyEditor(editor: RemoteEditor, snapshot: EditorSnapshot) {
  editor.documents.value = snapshot.documents
  editor.active.value = snapshot.active
  editor.directory.value = snapshot.directory
  editor.error.value = snapshot.error
}

/** 只传 UTF-16 变化区间；全量替换更小时沿用完整字段，避免放大 IPC。 */
function textChange(before: string, after: string): EditorTextChange | undefined {
  let from = 0
  const common = Math.min(before.length, after.length)
  while (from < common && before.charCodeAt(from) === after.charCodeAt(from)) from++
  let to = before.length
  let end = after.length
  while (to > from && end > from && before.charCodeAt(to - 1) === after.charCodeAt(end - 1)) {
    to--
    end--
  }
  if (end - from + 64 >= after.length) return undefined
  return { baseLength: before.length, from, to, insert: after.slice(from, end) }
}

export function editorUpdate(snapshot: EditorSnapshot, previous?: EditorSnapshot): EditorUpdate {
  const old = new Map(previous?.documents.map((doc) => [doc.id, doc]))
  const documents: EditorDocumentUpdate[] = []
  for (const doc of snapshot.documents) {
    const before = old.get(doc.id)
    if (!before) {
      documents.push({ id: doc.id, document: doc })
      continue
    }
    const changes: Partial<Omit<typeof doc, 'id'>> = {}
    const cleared: (keyof typeof changes)[] = []
    const textChanges: Partial<Record<'content' | 'saved' | 'remoteContent', EditorTextChange>> = {}
    const keys = new Set([...Object.keys(before), ...Object.keys(doc)])
    for (const name of keys) {
      if (name === 'id') continue
      const key = name as keyof typeof changes
      if (doc[key] === before[key]) continue
      if (
        (key === 'content' || key === 'saved' || key === 'remoteContent') &&
        typeof doc[key] === 'string' &&
        typeof before[key] === 'string'
      ) {
        const delta = textChange(before[key], doc[key])
        if (delta) {
          textChanges[key] = delta
          continue
        }
      }
      // undefined 经 JSON 传输会被丢弃，删除字段必须有显式表示。
      if (doc[key] === undefined) cleared.push(key)
      else Object.assign(changes, { [key]: doc[key] })
    }
    if (cleared.length || Object.keys(changes).length || Object.keys(textChanges).length)
      documents.push({
        id: doc.id,
        changes,
        cleared,
        ...(Object.keys(textChanges).length ? { textChanges } : {}),
      })
  }
  return {
    sequence: snapshot.sequence,
    baseSequence: previous?.sequence,
    active: snapshot.active,
    directory: snapshot.directory,
    error: snapshot.error,
    ids: snapshot.documents.map((doc) => doc.id),
    documents,
  }
}
export function mergeEditorUpdate(
  editor: RemoteEditor,
  update: EditorUpdate,
  sequence: number
): EditorSnapshot {
  if (update.baseSequence !== undefined && update.baseSequence !== sequence)
    throw new Error('编辑文档同步基线已变化，请重新同步完整快照')
  const documents = new Map(editor.documents.value.map((doc) => [doc.id, doc]))
  for (const patch of update.documents) {
    if ('document' in patch) {
      documents.set(patch.id, patch.document)
      continue
    }
    const before = documents.get(patch.id)
    if (!before) throw new Error('编辑文档同步不完整')
    const doc = { ...before, ...patch.changes }
    for (const [field, delta] of Object.entries(patch.textChanges ?? {})) {
      if (field !== 'content' && field !== 'saved' && field !== 'remoteContent')
        throw new Error('编辑文档文本同步字段无效')
      const source = before[field]
      if (
        typeof source !== 'string' ||
        source.length !== delta.baseLength ||
        !Number.isSafeInteger(delta.from) ||
        !Number.isSafeInteger(delta.to) ||
        delta.from < 0 ||
        delta.to < delta.from ||
        delta.to > source.length ||
        typeof delta.insert !== 'string'
      )
        throw new Error('编辑文档文本同步基线无效')
      doc[field] = source.slice(0, delta.from) + delta.insert + source.slice(delta.to)
    }
    for (const key of patch.cleared) Reflect.deleteProperty(doc, key)
    documents.set(patch.id, doc)
  }
  return {
    sequence: update.sequence,
    active: update.active,
    directory: update.directory,
    error: update.error,
    documents: update.ids.map((id) => {
      const doc = documents.get(id)
      if (!doc) throw new Error('编辑文档同步不完整')
      return { ...doc }
    }),
  }
}
