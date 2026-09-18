/** 每个接口页签独立拥有请求与长连接，迟到结果不得写回已关闭页签。 */
import { reactive } from 'vue'
import { ipc } from './ipc'
import { requestPayload, type RequestDraft } from './requestDraft'
import type { HttpResponseResult, SseUpdate } from './contracts'

export interface StreamEntry {
  seq: number
  time: number
  direction: string
  kind: string
  eventId: string
  content: string
  retry?: number | null
}
export function useRequestSession(
  getDraft: () => RequestDraft,
  report: (message: string) => void,
  isActive: () => boolean = () => true
) {
  const state = reactive({
    busy: false,
    connected: false,
    sending: false,
    error: '',
    response: null as HttpResponseResult | null,
    respondedAt: '',
    entries: [] as StreamEntry[],
    dropped: 0,
    streamStatus: 0,
    streamHeaders: [] as [string, string][],
    message: '',
  })
  let epoch = 0
  let disposed = false
  let wsId: string | null = null
  let sseId: string | null = null
  let timer: ReturnType<typeof setTimeout> | undefined
  let clearedSeq = 0
  let nextSeq = 0
  let streamChars = 0
  function fail(error: unknown) {
    state.error = error instanceof Error ? error.message : String(error)
  }
  async function closeWs(id: string) {
    const result = await ipc.wsClose(id)
    if (!result.ok) throw new Error(result.message || '断开连接失败')
  }
  async function release() {
    clearTimeout(timer)
    const ws = wsId,
      sse = sseId
    const results = await Promise.allSettled([
      ws ? closeWs(ws) : Promise.resolve(),
      sse ? ipc.sseStop(sse) : Promise.resolve(),
    ])
    if (results[0].status === 'fulfilled' && wsId === ws) wsId = null
    if (results[1].status === 'fulfilled' && sseId === sse) sseId = null
    const errors = results.filter((r): r is PromiseRejectedResult => r.status === 'rejected')
    if (errors.length) throw new Error(errors.map((r) => String(r.reason)).join('；'))
  }
  async function stop() {
    epoch++
    state.busy = false
    state.connected = false
    state.sending = false
    try {
      await release()
    } catch (error) {
      fail(error)
      throw error
    }
  }
  async function dispose() {
    disposed = true
    await stop()
  }
  async function poll(token: number, id: string) {
    try {
      const snapshot = await ipc.wsRecv(id)
      if (disposed || token !== epoch || wsId !== id) return
      state.connected = snapshot.open
      state.entries = snapshot.messages
        .filter((m) => m.seq > clearedSeq)
        .map((m) => ({
          seq: m.seq,
          time: m.time,
          direction: m.direction,
          kind: m.direction === 'sent' ? '发送' : '接收',
          eventId: '',
          content: m.content,
        }))
      state.dropped = snapshot.dropped
      if (snapshot.error) state.error = snapshot.error
    } catch (error) {
      if (!disposed && token === epoch) {
        fail(error)
        state.connected = false
      }
    }
    if (!disposed && token === epoch && state.connected)
      timer = setTimeout(() => void poll(token, id), isActive() ? 350 : 1200)
  }
  function receive(token: number, update: SseUpdate) {
    if (disposed || token !== epoch) return
    if (update.type === 'connected') {
      state.busy = false
      state.connected = true
      state.streamStatus = update.status
      state.streamHeaders = update.headers
    }
    if (update.type === 'closed' || update.type === 'error') {
      state.busy = false
      state.connected = false
    }
    if (update.type === 'error') state.error = update.message
    if (update.type === 'event') {
      streamChars += update.event.data.length + update.event.event.length + update.event.id.length
      state.entries.push({
        seq: ++nextSeq,
        time: Date.now(),
        direction: 'received',
        kind: update.event.event,
        eventId: update.event.id,
        content: update.event.data,
        retry: update.event.retry,
      })
      while (state.entries.length > 500 || streamChars > 4 * 1024 * 1024) {
        const removed = state.entries.shift()
        if (removed)
          streamChars -= removed.content.length + removed.kind.length + removed.eventId.length
        state.dropped++
      }
    }
  }
  async function run() {
    if (disposed || state.busy) return
    if (state.connected) {
      await stop()
      return
    }
    state.error = ''
    const token = ++epoch
    state.busy = true
    try {
      await release()
      if (disposed || token !== epoch) return
      const draft = getDraft(),
        payload = requestPayload(draft)
      if (draft.type === 'http') {
        state.response = null
        const response = await ipc.httpRequest(payload)
        if (!disposed && token === epoch) {
          state.response = response
          state.respondedAt = new Date().toLocaleTimeString()
          state.error = response.error || ''
        }
      } else if (draft.type === 'ws') {
        const session = await ipc.wsConnect({
          url: payload.url,
          headers: payload.headers,
          auth: payload.auth,
          timeoutMs: payload.timeoutMs,
        })
        if (disposed || token !== epoch) {
          await closeWs(session.id).catch((error) => report(`清理迟到连接失败：${String(error)}`))
          return
        }
        wsId = session.id
        clearedSeq = 0
        state.entries = []
        state.dropped = 0
        state.connected = session.open
        void poll(token, session.id)
      } else {
        const id = crypto.randomUUID()
        sseId = id
        state.entries = []
        streamChars = 0
        state.dropped = 0
        state.streamStatus = 0
        state.streamHeaders = []
        await ipc.sseStart(id, payload, (update) => receive(token, update))
        if (disposed || token !== epoch)
          await ipc.sseStop(id).catch((error) => report(`清理迟到连接失败：${String(error)}`))
      }
    } catch (error) {
      if (!disposed && token === epoch) {
        fail(error)
        state.busy = false
        state.connected = false
      }
    } finally {
      if (!disposed && token === epoch && getDraft().type !== 'sse') state.busy = false
    }
  }
  async function send() {
    if (!wsId || !state.connected || state.sending || !state.message) return
    const token = epoch,
      id = wsId
    state.sending = true
    try {
      const result = await ipc.wsSend(id, state.message)
      if (!result.ok) throw new Error(result.message || '发送失败')
    } catch (error) {
      if (token === epoch && !disposed) fail(error)
    } finally {
      if (token === epoch && !disposed) state.sending = false
    }
  }
  function clear() {
    clearedSeq = state.entries.at(-1)?.seq ?? clearedSeq
    state.entries = []
    streamChars = 0
    state.dropped = 0
  }
  return { state, run, stop, dispose, send, clear }
}
export type RequestSession = ReturnType<typeof useRequestSession>
