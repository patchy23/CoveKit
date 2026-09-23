import { mount, enableAutoUnmount, flushPromises } from '@vue/test-utils'
import { afterEach, expect, it, vi } from 'vitest'
import { computed, ref } from 'vue'
import RemoteEditorWorkspace from './RemoteEditorWorkspace.vue'
import { UiTabs, UiCodeEditor, UiModal } from '@/core/ui'
import type { RemoteDocument, useRemoteEditor } from './useRemoteEditor'
enableAutoUnmount(afterEach)
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
  await wrapper
    .findAll('button')
    .find((b) => b.text() === '收起')!
    .trigger('click')
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
