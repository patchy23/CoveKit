import type { useRemoteEditor } from '../useRemoteEditor'
import type { EditorDocumentUpdate, EditorLayout, EditorSnapshot, EditorUpdate } from './protocol'
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
    const keys = new Set([...Object.keys(before), ...Object.keys(doc)])
    for (const name of keys) {
      if (name === 'id') continue
      const key = name as keyof typeof changes
      if (doc[key] === before[key]) continue
      // undefined 经 JSON 传输会被丢弃，删除字段必须有显式表示。
      if (doc[key] === undefined) cleared.push(key)
      else Object.assign(changes, { [key]: doc[key] })
    }
    if (cleared.length || Object.keys(changes).length)
      documents.push({ id: doc.id, changes, cleared })
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
