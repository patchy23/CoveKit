import { expect, it } from 'vitest'
import { ref } from 'vue'
import { captureEditor, editorUpdate, mergeEditorUpdate, type RemoteEditor } from './snapshot'
it('增量只发送变化文档，保留其他草稿并准确同步关闭与顺序', () => {
  const a = {
    id: 'a',
    path: '/a',
    content: 'draft',
    saved: '',
    saving: false,
    error: '',
    conflict: false,
  }
  const b = { ...a, id: 'b', path: '/b', content: 'large unchanged document' }
  const editor = {
    documents: ref([a, b]),
    active: ref('/a'),
    directory: ref('/'),
    error: ref(''),
  } as RemoteEditor
  const previous = captureEditor(editor, 1)
  editor.documents.value[0].content = 'new draft'
  const current = captureEditor(editor, 2)
  expect(previous.documents[0].content).toBe('draft')
  const patch = editorUpdate(current, previous)
  expect(patch.documents.map((doc) => doc.id)).toEqual(['a'])
  const result = mergeEditorUpdate(editor, patch)
  expect(result.documents.map((doc) => doc.content)).toEqual(['new draft', b.content])
  const closed = editorUpdate(
    { ...current, sequence: 3, documents: [current.documents[1]] },
    current
  )
  expect(closed.documents).toHaveLength(0)
  expect(mergeEditorUpdate(editor, closed).documents.map((doc) => doc.id)).toEqual(['b'])
})
