import { mount, flushPromises, enableAutoUnmount } from '@vue/test-utils'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { computed, defineComponent, ref } from 'vue'
import { useNativeEditor } from './useNativeEditor'
import { collectToolBlockers, resetToolOwnersForTest } from '@/core/lifecycle'
import type { RemoteEditor, EditorViewHandle } from './snapshot'
import type { EditorInitial, EditorSnapshot } from './protocol'
const env = vi.hoisted(() => ({
  operation: vi.fn(),
  request: vi.fn(),
  dispose: vi.fn(),
  receive: undefined as unknown as (type: string, value?: unknown) => Promise<unknown>,
  destroyed: undefined as unknown as () => void,
}))
vi.mock('@tauri-apps/api/core', () => ({ isTauri: () => true }))
vi.mock('@tauri-apps/api/webviewWindow', () => ({
  WebviewWindow: {
    getByLabel: async () => ({
      once: async (_event: string, callback: () => void) => {
        env.destroyed = callback
        return () => {}
      },
    }),
  },
}))
vi.mock('../../ipc', () => ({ ipc: { sshEditorWindow: env.operation } }))
vi.mock('./channel', () => ({
  editorChannel: async (_token: string, _peer: string, receive: typeof env.receive) => {
    env.receive = receive
    return { request: env.request, dispose: env.dispose }
  },
}))
enableAutoUnmount(afterEach)
beforeEach(() => {
  vi.clearAllMocks()
  env.operation.mockImplementation(async (_token: string, action: string) => {
    if (action === 'open') {
      await env.receive('ready')
      await env.receive('mounted')
    }
  })
})
afterEach(() => resetToolOwnersForTest())
function setup() {
  const documents = ref([
    {
      id: 'd',
      path: '/a',
      content: 'draft',
      saved: 'original',
      saving: false,
      error: '',
      conflict: false,
    },
  ])
  const visible = ref(true)
  const editor = {
    documents,
    visible,
    active: ref('/a'),
    directory: ref('/'),
    error: ref(''),
    busy: ref(false),
    dirty: computed(() => documents.value.filter((d) => d.content !== d.saved)),
    hide: () => (visible.value = false),
  } as unknown as RemoteEditor
  const restoreLayout = vi.fn().mockResolvedValue(undefined)
  let native!: ReturnType<typeof useNativeEditor>
  mount(
    defineComponent({
      setup() {
        native = useNativeEditor(
          editor,
          ref({
            captureLayout: () => ({ sidebar: true, width: 240, documents: {} }),
            restoreLayout,
            requestClose: () => {},
          } as EditorViewHandle),
          () => ({ sessionId: 'existing-session', profileId: 'p', status: 'connected' }),
          () => 'host'
        )
        return () => null
      },
    })
  )
  return { editor, native, restoreLayout }
}
it('握手后工具内隐藏，复用已有会话；移回采用最终快照', async () => {
  const { editor, native, restoreLayout } = setup()
  await native.detach()
  expect(native.detached.value).toBe(true)
  expect(editor.visible.value).toBe(false)
  const initial = (await env.receive('ready')) as EditorInitial
  expect(initial.connection.sessionId).toBe('existing-session')
  const final: EditorSnapshot = {
    ...initial.snapshot,
    sequence: 3,
    documents: initial.snapshot.documents.map((d) => ({ ...d, content: 'final' })),
  }
  await env.receive('dock', final)
  await env.receive('state', { ...initial.snapshot, sequence: 2 })
  expect(editor.documents.value[0].content).toBe('final')
  expect(editor.visible.value).toBe(true)
  expect(restoreLayout).toHaveBeenCalled()
})
it('原生创建失败仍保留原窗口草稿与可编辑状态', async () => {
  const { editor, native } = setup()
  env.operation.mockRejectedValueOnce(new Error('创建失败'))
  await native.detach()
  expect(editor.visible.value).toBe(true)
  expect(native.moving.value).toBe(false)
  expect(native.detached.value).toBe(false)
  expect(editor.documents.value[0].content).toBe('draft')
  expect(editor.error.value).toContain('创建失败')
})
it('关闭协商先读取子窗口最后草稿；意外销毁恢复影子内容', async () => {
  const { editor, native } = setup()
  await native.detach()
  const initial = (await env.receive('ready')) as EditorInitial
  env.request.mockResolvedValue({ ...initial.snapshot, sequence: 5 })
  const blockers = await collectToolBlockers('ssh', 'exit')
  expect(env.request).toHaveBeenCalledWith('capture')
  expect(blockers.some((v) => v.message.includes('未保存'))).toBe(true)
  env.destroyed()
  await flushPromises()
  expect(editor.visible.value).toBe(true)
  expect(editor.error.value).toContain('意外关闭')
})
