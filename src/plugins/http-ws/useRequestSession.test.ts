import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { newDraft } from './requestDraft'
import type { HttpResponseResult, SseUpdate, WsSession } from './contracts'

const mock = vi.hoisted(() => ({
  httpRequest: vi.fn(),
  wsConnect: vi.fn(),
  wsRecv: vi.fn(),
  wsSend: vi.fn(),
  wsClose: vi.fn(),
  sseStart: vi.fn(),
  sseStop: vi.fn(),
}))
vi.mock('./ipc', () => ({ ipc: mock }))
import { useRequestSession } from './useRequestSession'
function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (reason: unknown) => void
  const promise = new Promise<T>((yes, no) => {
    resolve = yes
    reject = no
  })
  return { promise, resolve, reject }
}
const response: HttpResponseResult = {
  ok: true,
  status: 200,
  statusText: 'OK',
  headers: [],
  body: 'a',
  bodySize: 1,
  durationMs: 10,
}
const socket = (id = 'ws-1', open = true): WsSession => ({
  id,
  url: 'ws://example.invalid',
  connectedAt: 1,
  open,
  messages: [],
  dropped: 0,
})
const flush = async () => {
  for (let i = 0; i < 8; i++) await Promise.resolve()
}

describe('每个页签的请求与连接所有权', () => {
  beforeEach(() => {
    vi.useFakeTimers()
    vi.resetAllMocks()
    mock.wsClose.mockResolvedValue({ ok: true })
    mock.sseStop.mockResolvedValue(undefined)
    mock.wsRecv.mockResolvedValue(socket())
  })
  afterEach(() => vi.useRealTimers())
  it('HTTP 迟到响应不写入已关闭页签，其他页签正常完成', async () => {
    const pending = deferred<HttpResponseResult>()
    mock.httpRequest
      .mockReturnValueOnce(pending.promise)
      .mockResolvedValueOnce({ ...response, body: 'b' })
    const draft = newDraft('http')
    draft.url = 'https://example.invalid'
    const a = useRequestSession(() => draft, vi.fn()),
      b = useRequestSession(() => draft, vi.fn())
    const running = a.run()
    await flush()
    await a.dispose()
    await b.run()
    pending.resolve(response)
    await running
    expect(a.state.response).toBeNull()
    expect(b.state.response?.body).toBe('b')
  })
  it('连接中关闭页签会回收迟到的 WebSocket，停止旧请求不影响新连接', async () => {
    const pending = deferred<WsSession>()
    mock.wsConnect.mockReturnValueOnce(pending.promise).mockResolvedValueOnce(socket('ws-new'))
    const draft = newDraft('ws')
    draft.url = 'ws://example.invalid'
    const session = useRequestSession(() => draft, vi.fn())
    const old = session.run()
    await flush()
    await session.stop()
    await session.run()
    pending.resolve(socket('ws-old'))
    await old
    expect(mock.wsClose).toHaveBeenCalledWith('ws-old')
    expect(session.state.connected).toBe(true)
    await session.dispose()
    expect(mock.wsClose).toHaveBeenCalledWith('ws-new')
  })
  it('轮询无并发重入，清空后旧消息不再出现，关闭会取消后续轮询', async () => {
    mock.wsConnect.mockResolvedValue(socket())
    const pending = deferred<WsSession>()
    mock.wsRecv.mockReturnValueOnce(pending.promise)
    const draft = newDraft('ws')
    draft.url = 'ws://example.invalid'
    const session = useRequestSession(() => draft, vi.fn())
    await session.run()
    await vi.advanceTimersByTimeAsync(2000)
    expect(mock.wsRecv).toHaveBeenCalledTimes(1)
    const snapshot = {
      ...socket(),
      messages: [{ seq: 1, time: 1, direction: 'received' as const, content: 'old' }],
    }
    pending.resolve(snapshot)
    await flush()
    session.clear()
    mock.wsRecv.mockResolvedValue(snapshot)
    await vi.advanceTimersByTimeAsync(350)
    expect(session.state.entries).toEqual([])
    await session.dispose()
    const count = mock.wsRecv.mock.calls.length
    await vi.advanceTimersByTimeAsync(2000)
    expect(mock.wsRecv).toHaveBeenCalledTimes(count)
  })
  it('SSE 消息有界，切换页签继续接收，停止后忽略事件并保留已收内容', async () => {
    let receive!: (update: SseUpdate) => void
    mock.sseStart.mockImplementation((_id, _payload, handler) => {
      receive = handler
      return Promise.resolve()
    })
    const draft = newDraft('sse')
    draft.url = 'https://example.invalid/events'
    const session = useRequestSession(
      () => draft,
      vi.fn(),
      () => false
    )
    await session.run()
    receive({ type: 'connected', status: 200, headers: [] })
    for (let i = 0; i < 501; i++)
      receive({ type: 'event', event: { event: 'delta', id: String(i), data: String(i) } })
    expect(session.state.entries).toHaveLength(500)
    expect(session.state.dropped).toBe(1)
    await session.stop()
    receive({ type: 'event', event: { event: 'late', id: 'late', data: 'late' } })
    expect(session.state.entries.at(-1)?.content).toBe('500')
    expect(mock.sseStop).toHaveBeenCalledTimes(1)
  })
  it('发送失败保留消息，断开失败可重试并明确报告', async () => {
    mock.wsConnect.mockResolvedValue(socket())
    mock.wsSend.mockResolvedValue({ ok: false, message: '发送失败' })
    const draft = newDraft('ws')
    draft.url = 'ws://example.invalid'
    const session = useRequestSession(() => draft, vi.fn())
    await session.run()
    session.state.message = '  raw\n'
    await session.send()
    expect(session.state.message).toBe('  raw\n')
    expect(session.state.error).toBe('发送失败')
    mock.wsClose.mockRejectedValueOnce(new Error('清理失败'))
    await expect(session.stop()).rejects.toThrow('清理失败')
    await session.dispose()
    expect(mock.wsClose).toHaveBeenCalledTimes(2)
  })
})
