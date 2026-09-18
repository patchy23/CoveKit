import { beforeEach, expect, it, vi } from 'vitest'
const invoke = vi.hoisted(() => vi.fn())
vi.mock('@/core/ipc/ipc', () => ({ invokeCommand: invoke }))
vi.mock('@tauri-apps/api/core', () => ({
  Channel: class {
    onmessage: unknown
  },
}))
import { ipc } from './ipc'
import { newDraft, requestPayload, toRecord } from './requestDraft'
beforeEach(() => invoke.mockReset())

it('SSE 使用 camelCase Channel 契约，停止使用同一会话 ID', async () => {
  const draft = newDraft('sse')
  draft.url = 'https://example.invalid/events'
  const payload = requestPayload(draft),
    receive = vi.fn()
  await ipc.sseStart('sse-test', payload, receive)
  const [command, args] = invoke.mock.calls[0]
  expect(command).toBe('sse_start')
  expect(args.id).toBe('sse-test')
  expect(args.payload).toEqual(payload)
  args.onEvent.onmessage({ type: 'closed' })
  expect(receive).toHaveBeenCalledWith({ type: 'closed' })
  await ipc.sseStop('sse-test')
  expect(invoke).toHaveBeenLastCalledWith('sse_stop', { id: 'sse-test' })
})

it('保存和 WS 握手字段平铺传入，持久化包含分组和无秘密设置', async () => {
  const draft = newDraft('ws')
  draft.url = 'ws://example.invalid'
  draft.auth = { mode: 'bearer', credentialId: 'credential-ref' }
  const saved = toRecord(draft, '连接', '测试')
  await ipc.apiSave(saved)
  expect(invoke).toHaveBeenLastCalledWith('api_save', saved)
  const handshake = {
    url: draft.url,
    headers: [['X-Test', 'value']] as [string, string][],
    timeoutMs: 5000,
    auth: draft.auth,
  }
  await ipc.wsConnect(handshake)
  expect(invoke).toHaveBeenLastCalledWith('ws_connect', handshake)
})
