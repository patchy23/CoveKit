import { mount, enableAutoUnmount, flushPromises } from '@vue/test-utils'
import { afterEach, expect, it, vi } from 'vitest'
import { computed, ref } from 'vue'
import RemoteEditorWorkspace from './RemoteEditorWorkspace.vue'
import type { RemoteDocument, useRemoteEditor } from './useRemoteEditor'
import { UiTabs, UiTabsOverflowMenu, UiModal, UiCodeEditor } from '@/core/ui'

enableAutoUnmount(afterEach)
vi.stubGlobal(
  'ResizeObserver',
  class {
    observe() {}
    disconnect() {}
    unobserve() {}
  }
)
vi.mock('./RemoteEditorTree.vue', () => ({ default: { template: '<div />' } }))

it('有限宽度收纳页签；最小化保留草稿，关闭未保存文件需选择', async () => {
  const docs = ref(
    Array.from({ length: 12 }, (_, i) => ({
      id: String(i),
      path: '/srv/file-' + i,
      content: 'changed',
      saved: '',
      saving: false,
    }))
  )
  const visible = ref(true),
    active = ref(docs.value[11].path)
  const editor = {
    documents: docs,
    active,
    visible,
    busy: ref(false),
    error: ref(''),
    directory: ref('/srv'),
    current: computed(() => docs.value.find((d) => d.path === active.value)),
    hide: () => (visible.value = false),
    activate: (path: string) => (active.value = path),
    remove: (path: string) => (docs.value = docs.value.filter((d) => d.path !== path)),
  } as unknown as ReturnType<typeof useRemoteEditor>
  const wrapper = mount(RemoteEditorWorkspace, {
    props: { editor, title: 'test' },
    global: { stubs: { UiCodeEditor: true, UiCodeDiff: true } },
  })
  expect(wrapper.getComponent(UiTabs).props('items')).toHaveLength(1)
  expect(wrapper.getComponent(UiTabs).props('items')[0].value).toBe(active.value)
  expect(wrapper.getComponent(UiTabsOverflowMenu).props('items')).toHaveLength(11)
  await wrapper.get('[aria-label="最小化窗口"]').trigger('click')
  expect(visible.value).toBe(false)
  expect(docs.value).toHaveLength(12)
  visible.value = true
  await flushPromises()
  await wrapper.get('[aria-label="关闭窗口"]').trigger('click')
  expect(docs.value).toHaveLength(12)
  expect(wrapper.findAllComponents(UiModal)[0].props('open')).toBe(true)
})

it('工作区保持多个编辑实例，收起不卸载，关闭未保存文件先询问', async () => {
  const documents = ref<RemoteDocument[]>([
    {
      id: 'a',
      path: '/a',
      content: 'draft',
      saved: 'base',
      saving: false,
      error: '',
      conflict: false,
    },
    {
      id: 'b',
      path: '/b',
      content: 'base',
      saved: 'base',
      saving: false,
      error: '',
      conflict: false,
    },
  ])
  const active = ref('/a'),
    visible = ref(true)
  const editor: ReturnType<typeof useRemoteEditor> = {
    documents,
    active,
    visible,
    directory: ref('/'),
    error: ref(''),
    dirty: computed(() => documents.value.filter((d) => d.content !== d.saved)),
    busy: computed(() => false),
    current: computed(() => documents.value.find((d) => d.path === active.value)),
    hide: () => {
      visible.value = false
    },
    activate: (p) => {
      active.value = p
    },
    openFile: vi.fn(),
    save: vi.fn(),
    saveAll: vi.fn(),
    reload: vi.fn(),
    renamed: vi.fn(),
    remove: (p) => {
      documents.value = documents.value.filter((d) => d.path !== p)
    },
  }
  const wrapper = mount(RemoteEditorWorkspace, {
    props: {
      editor,
      title: 'server',
      connection: { sessionId: 's', profileId: 'p', status: 'connected' },
    },
    global: {
      stubs: {
        RemoteEditorTree: true,
        UiCodeEditor: { props: ['modelValue'], template: '<textarea :value="modelValue" />' },
        UiModal: true,
        UiContextMenu: true,
      },
    },
  })
  const inputs = wrapper.findAllComponents(UiCodeEditor)
  expect(inputs).toHaveLength(2)
  wrapper.getComponent(UiTabs).vm.$emit('update:modelValue', '/b')
  await flushPromises()
  expect(wrapper.findAllComponents(UiCodeEditor)[0].element).toBe(inputs[0].element)
  await wrapper.get('[aria-label="最小化窗口"]').trigger('click')
  expect(visible.value).toBe(false)
  expect(wrapper.findAllComponents(UiCodeEditor)).toHaveLength(2)
  visible.value = true
  wrapper.getComponent(UiTabs).vm.$emit('close', '/a')
  await flushPromises()
  expect(documents.value).toHaveLength(2)
  expect(
    wrapper
      .findAllComponents(UiModal)
      .find((m) => m.props('title') === '文件尚未保存')!
      .props('open')
  ).toBe(true)
})
