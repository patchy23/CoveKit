import { mount, enableAutoUnmount, flushPromises } from '@vue/test-utils'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import NativeEditorApp from './NativeEditorApp.vue'
import RemoteEditorWorkspace from '../RemoteEditorWorkspace.vue'
import { UiModal } from '@/core/ui'
import type { EditorInitial } from './protocol'
const env = vi.hoisted(() => ({
  request: vi.fn(),
  operation: vi.fn(),
  dispose: vi.fn(),
  closing: undefined as unknown as (event: { preventDefault: () => void }) => void,
}))
const token = '12345678-1234-1234-1234-123456789abc'
vi.mock('@tauri-apps/api/webviewWindow', () => ({
  getCurrentWebviewWindow: () => ({
    label: 'ssh-editor-12345678-1234-1234-1234-123456789abc',
    onCloseRequested: async (callback: typeof env.closing) => {
      env.closing = callback
      return () => {}
    },
  }),
}))
vi.mock('@/stores/settings', () => ({
  useSettingsStore: () => ({ init: async () => {}, unsubscribeSystemTheme() {}, $dispose() {} }),
}))
vi.mock('./channel', () => ({
  editorChannel: async () => ({ request: env.request, dispose: env.dispose }),
}))
vi.mock('../../ipc', () => ({ ipc: { sshEditorWindow: env.operation } }))
vi.mock('../RemoteEditorTree.vue', () => ({ default: { template: '<div />' } }))
enableAutoUnmount(afterEach)
beforeEach(() => {
  vi.clearAllMocks()
  window.history.replaceState(null, '', '/?sshEditor=' + token)
  const initial: EditorInitial = {
    title: 'host',
    connection: { profileId: 'p', sessionId: 'same-session', status: 'connected' },
    snapshot: {
      sequence: 7,
      active: '/a',
      directory: '/',
      error: '',
      documents: [
        {
          id: 'd',
          path: '/a',
          content: 'draft',
          saved: 'old',
          saving: false,
          conflict: false,
          error: '',
        },
      ],
    },
  }
  env.request.mockImplementation(async (type: string) =>
    type === 'ready' ? initial : type === 'ownership' ? 'main' : undefined
  )
})
function setup() {
  return mount(NativeEditorApp, {
    global: {
      stubs: {
        Toast: true,
        UiCodeEditor: true,
        UiCodeDiff: true,
        DialogPortal: { template: '<slot />' },
      },
    },
  })
}
it('独立入口复用会话，移回先交接草稿再关闭原生窗口', async () => {
  const wrapper = setup()
  await flushPromises()
  const workspace = wrapper.getComponent(RemoteEditorWorkspace)
  expect(workspace.props('standalone')).toBe(true)
  expect(workspace.props('connection')?.sessionId).toBe('same-session')
  workspace.vm.$emit('dock')
  await flushPromises()
  expect(env.request).toHaveBeenCalledWith(
    'dock',
    expect.objectContaining({
      documents: [expect.objectContaining({ content: 'draft', saved: 'old' })],
    })
  )
  expect(env.operation).toHaveBeenCalledWith(token, 'close')
  expect(env.request.mock.invocationCallOrder.at(-1)!).toBeLessThan(
    env.operation.mock.invocationCallOrder[0]
  )
})
it('系统关闭按钮先确认未保存内容，不立即销毁窗口', async () => {
  const wrapper = setup()
  await flushPromises()
  const preventDefault = vi.fn()
  env.closing({ preventDefault })
  await flushPromises()
  expect(preventDefault).toHaveBeenCalled()
  expect(
    wrapper
      .findAllComponents(UiModal)
      .find((modal) => modal.props('title') === '文件尚未保存')
      ?.props('open')
  ).toBe(true)
  expect(env.operation).not.toHaveBeenCalled()
})
