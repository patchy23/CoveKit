import type { useRemoteEditor } from '../useRemoteEditor'
import type { EditorLayout, EditorSnapshot, EditorUpdate } from './protocol'
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
  return {
    ...snapshot,
    ids: snapshot.documents.map((doc) => doc.id),
    documents: snapshot.documents.filter((doc) => {
      const before = old.get(doc.id)
      return (
        !before ||
        (Object.keys(doc) as (keyof typeof doc)[]).some((key) => doc[key] !== before[key])
      )
    }),
  }
}
export function mergeEditorUpdate(editor: RemoteEditor, update: EditorUpdate): EditorSnapshot {
  const documents = new Map(editor.documents.value.map((doc) => [doc.id, doc]))
  for (const doc of update.documents) documents.set(doc.id, doc)
  return {
    ...update,
    documents: update.ids.map((id) => {
      const doc = documents.get(id)
      if (!doc) throw new Error('编辑文档同步不完整')
      return { ...doc }
    }),
  }
}
