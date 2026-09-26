import { expect, it } from 'vitest'
import { ref } from 'vue'
import { captureEditor, editorUpdate, mergeEditorUpdate, type RemoteEditor } from './snapshot'
import type { RemoteDocument } from '../useRemoteEditor'
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
  editor.documents.value = previous.documents
  const result = mergeEditorUpdate(editor, patch, 1)
  expect(result.documents.map((doc) => doc.content)).toEqual(['new draft', b.content])
  const closed = editorUpdate(
    { ...current, sequence: 3, documents: [current.documents[1]] },
    current
  )
  expect(closed.documents).toHaveLength(0)
  expect(mergeEditorUpdate(editor, closed, 2).documents.map((doc) => doc.id)).toEqual(['b'])
})

function fixture() {
  const document: RemoteDocument = {
    id: 'a',
    path: '/a',
    content: '大文件'.repeat(100_000),
    saved: '基线'.repeat(100_000),
    saving: false,
    error: '',
    conflict: true,
    modifiedAt: 5,
    remoteContent: '旧远端内容',
  }
  const editor = {
    documents: ref([document]),
    active: ref('/a'),
    directory: ref('/'),
    error: ref(''),
  } as RemoteEditor
  return { editor, before: captureEditor(editor, 1) }
}

it('状态与路径变化不搬运大文本，文本修改也不重复发送保存基线', () => {
  const { editor, before } = fixture()
  editor.documents.value[0].saving = true
  editor.documents.value[0].path = '/renamed'
  const patch = editorUpdate(captureEditor(editor, 2), before)
  expect(JSON.stringify(patch).length).toBeLessThan(500)
  expect(patch.documents).toEqual([
    { id: 'a', changes: { saving: true, path: '/renamed' }, cleared: [] },
  ])
  editor.documents.value[0].content = '新内容'
  const edited = editorUpdate(captureEditor(editor, 3), before)
  expect(JSON.stringify(edited)).not.toContain('基线')
  editor.documents.value = before.documents
  const result = mergeEditorUpdate(editor, edited, 1)
  expect(result.documents[0]).toMatchObject({
    content: '新内容',
    saved: before.documents[0].saved,
    saving: true,
  })
})

it('JSON 往返后仍准确清除可选字段，新增文档完整且关闭与排序保持', () => {
  const { editor, before } = fixture()
  editor.documents.value[0].remoteContent = undefined
  delete editor.documents.value[0].modifiedAt
  editor.documents.value.unshift({ ...before.documents[0], id: 'b', path: '/b' })
  const current = captureEditor(editor, 2)
  const patch = JSON.parse(JSON.stringify(editorUpdate(current, before)))
  editor.documents.value = before.documents
  const result = mergeEditorUpdate(editor, patch, 1)
  expect(result.documents.map((doc) => doc.id)).toEqual(['b', 'a'])
  expect(result.documents[1].remoteContent).toBeUndefined()
  expect(result.documents[1].modifiedAt).toBeUndefined()
  expect(result.documents[0].content).toBe(before.documents[0].content)
  expect(editor.documents.value[0].remoteContent).toBe('旧远端内容')
})

it('失步增量被拒绝且不污染草稿，完整补发可从任意基线恢复', () => {
  const { editor, before } = fixture()
  editor.documents.value[0].content = '更新草稿'
  const current = captureEditor(editor, 3)
  editor.documents.value = before.documents
  expect(() => mergeEditorUpdate(editor, editorUpdate(current, before), 2)).toThrow('基线')
  expect(editor.documents.value[0].content).toBe(before.documents[0].content)
  expect(mergeEditorUpdate(editor, editorUpdate(current), 2).documents[0].content).toBe('更新草稿')
})

it('大文档局部输入仅同步差量，正文和保存基线经 JSON 往返完整恢复', () => {
  const { editor, before } = fixture()
  const original = before.documents[0].content
  const next = original.slice(0, 300) + '🙂\r\n修改' + original.slice(303)
  editor.documents.value[0].content = next
  editor.documents.value[0].saved += '保存'
  const update = JSON.parse(JSON.stringify(editorUpdate(captureEditor(editor, 2), before)))
  expect(JSON.stringify(update).length).toBeLessThan(600)
  editor.documents.value = before.documents
  const merged = mergeEditorUpdate(editor, update, 1)
  expect(merged.documents[0].content).toBe(next)
  expect(merged.documents[0].saved).toBe(before.documents[0].saved + '保存')
  expect(editor.documents.value[0].content).toBe(original)
  update.documents[0].textChanges.content.to = original.length + 1
  expect(() => mergeEditorUpdate(editor, update, 1)).toThrow('基线')
  expect(editor.documents.value[0].content).toBe(original)
})

it('代理对边界、删除和整文替换均能精确恢复', () => {
  for (const replacement of ['😀', '', '替换整篇']) {
    const { editor, before } = fixture()
    editor.documents.value[0].content =
      replacement === '替换整篇'
        ? replacement
        : before.documents[0].content.slice(0, 80) +
          replacement +
          before.documents[0].content.slice(90)
    const current = captureEditor(editor, 2)
    const update = JSON.parse(JSON.stringify(editorUpdate(current, before)))
    editor.documents.value = before.documents
    expect(mergeEditorUpdate(editor, update, 1).documents[0].content).toBe(
      current.documents[0].content
    )
  }
})
