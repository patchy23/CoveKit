import { enableAutoUnmount, flushPromises, mount } from '@vue/test-utils'
import { afterEach, expect, it } from 'vitest'
import { EditorView } from '@codemirror/view'
import ComposeCreateDialog from './ComposeCreateDialog.vue'

enableAutoUnmount(afterEach)

it('新增真实弹窗挂载 CodeMirror，模板可见且编辑事务回写内容', async () => {
  const wrapper = mount(ComposeCreateDialog, {
    attachTo: document.body,
    props: {
      connectionId: 'one',
      busy: false,
      connected: true,
      error: '',
      content: 'services: {}',
      defaultDirectory: async () => '/home/test/compose',
    },
  })
  await flushPromises()
  const content = document.querySelector<HTMLElement>('[role="dialog"] .cm-content')
  expect(content).not.toBeNull()
  expect(content?.textContent).toContain('services: {}')
  expect(content?.getAttribute('contenteditable')).toBe('true')
  const view = EditorView.findFromDOM(content!)!
  view.dispatch({
    changes: { from: 0, to: view.state.doc.length, insert: 'services: { web: {} }' },
  })
  expect(wrapper.emitted('update:content')?.at(-1)).toEqual(['services: { web: {} }'])
})
