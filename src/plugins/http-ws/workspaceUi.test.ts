import { enableAutoUnmount, flushPromises, mount } from '@vue/test-utils'
import { afterEach, expect, it, vi } from 'vitest'
import { ref } from 'vue'
import { createPinia } from 'pinia'
import Workspace from './index.vue'
import ApiSidebar from './ApiSidebar.vue'
import HttpPanel from './HttpPanel.vue'
import HttpRequestBuilder from './HttpRequestBuilder.vue'
import NewRequestMenu from './NewRequestMenu.vue'
import type { ApiRecord } from './contracts'
const mock = vi.hoisted(() => ({
  apiList: vi.fn(),
  apiGroupList: vi.fn(),
  httpRequest: vi.fn(),
  sseStop: vi.fn(),
  wsClose: vi.fn(),
}))
vi.mock('./ipc', () => ({ ipc: mock }))
vi.mock('@/core/dataTransfer/useDataRefresh', () => ({ useDataRefresh: vi.fn() }))
vi.mock('@/core/lifecycle', () => ({
  useToolLifecycle: () => ({
    dirty: ref(false),
    running: ref(false),
    visibility: ref({ active: true, hidden: false, covered: false }),
  }),
}))
enableAutoUnmount(afterEach)

it('真实页面打开与切换接口保留组件、配置和结果，协议入口只负责新建', async () => {
  const record: ApiRecord = {
    id: 1,
    type: 'http',
    name: 'HTTP 示例',
    method: 'GET',
    url: 'https://example.invalid',
    params: '[]',
    headers: '[]',
    bodyMode: 'none',
    body: '',
    groupName: '测试',
    options: '{}',
    updatedAt: '',
  }
  mock.apiList.mockResolvedValue([record])
  mock.apiGroupList.mockResolvedValue(['测试'])
  mock.httpRequest.mockResolvedValue({
    ok: true,
    status: 200,
    statusText: 'OK',
    headers: [],
    body: 'response',
    bodySize: 8,
    durationMs: 1,
  })
  const wrapper = mount(Workspace, {
    attachTo: document.body,
    global: {
      plugins: [createPinia()],
      stubs: {
        UiCodeEditor: { props: ['modelValue'], template: '<pre>{{modelValue}}</pre>' },
        CredentialPicker: true,
        UiModal: true,
        ConfirmDialog: true,
      },
    },
  })
  await flushPromises()
  wrapper.getComponent(ApiSidebar).vm.$emit('select', record)
  await flushPromises()
  const first = wrapper.getComponent(HttpPanel)
  await first.get('input[aria-label="请求地址"]').setValue('https://example.invalid/changed')
  await first
    .findAll('button')
    .find((button) => button.text() === '发送')!
    .trigger('click')
  await flushPromises()
  expect(first.text()).toContain('200 OK')
  expect(first.text()).toContain('response')
  expect(wrapper.text()).toContain('* GET HTTP 示例')
  wrapper.findAllComponents(NewRequestMenu).at(-1)!.vm.$emit('create', 'sse')
  await flushPromises()
  expect(wrapper.findAllComponents(HttpPanel)).toHaveLength(2)
  expect(wrapper.text()).toContain('* SSE 未命名 SSE')
  expect(first.isVisible()).toBe(false)
  expect(wrapper.findAllComponents(HttpRequestBuilder)).toHaveLength(2)
  wrapper.getComponent(ApiSidebar).vm.$emit('select', record)
  await flushPromises()
  expect(first.isVisible()).toBe(true)
  expect((first.get('input[aria-label="请求地址"]').element as HTMLInputElement).value).toBe(
    'https://example.invalid/changed'
  )
  expect(first.text()).toContain('response')
})
