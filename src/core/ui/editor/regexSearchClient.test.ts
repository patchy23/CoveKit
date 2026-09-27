import { EditorState } from '@codemirror/state'
import { afterEach, expect, it, vi } from 'vitest'
import { createRegexClient } from './regexSearchClient'

afterEach(() => vi.unstubAllGlobals())

it('连续请求取消旧等待者，消息解码失败释放缓存文档，重试发送完整快照', async () => {
  const instances: Worker[] = []
  vi.stubGlobal(
    'Worker',
    class {
      terminate = vi.fn()
      postMessage = vi.fn()
      constructor() {
        instances.push(this as unknown as Worker)
      }
    }
  )
  const client = createRegexClient((doc) => doc.toString())
  const doc = EditorState.create({ doc: 'aaa' }).doc
  const request = {
    query: 'a',
    replacement: '',
    wordChars: '',
    action: 'scan' as const,
    options: { regexp: true, caseSensitive: true, wholeWord: false },
    selection: { from: 0, to: 0 },
    viewport: { from: 0, to: 3 },
  }
  const first = client.run(doc, request)
  const cancelled = expect(first).rejects.toMatchObject({ name: 'AbortError' })
  const late = instances[0].onmessageerror!
  const second = client.run(doc, request)
  const failed = expect(second).rejects.toThrow('无法读取正则查找结果')
  await cancelled
  late.call(instances[0], new MessageEvent('messageerror'))
  expect(instances[1].terminate).not.toHaveBeenCalled()
  instances[1].onmessageerror!(new MessageEvent('messageerror'))
  await failed
  expect(instances[1].terminate).toHaveBeenCalledOnce()
  expect(instances[1].onmessage).toBeNull()
  const next = client.run(doc, request)
  expect(instances[2].postMessage).toHaveBeenCalledWith(
    expect.objectContaining({ document: 'aaa' })
  )
  const response = {
    id: 3,
    scan: { positions: [0], lengths: [1], truncated: false },
    highlights: [],
  }
  instances[2].onmessage!(new MessageEvent('message', { data: response }))
  expect(await next).toEqual(response)
  const reused = client.run(doc, request)
  expect(instances).toHaveLength(3)
  expect(vi.mocked(instances[2].postMessage).mock.lastCall![0]).not.toHaveProperty('document')
  const rejected = expect(reused).rejects.toThrow('响应格式无效')
  instances[2].onmessage!(new MessageEvent('message', { data: null }))
  await rejected
  client.reset()
})
it('终止挂起正则后释放 Worker，下一修订重新发送快照且忽略旧响应', async () => {
  const instances: {
    terminate: ReturnType<typeof vi.fn>
    postMessage: ReturnType<typeof vi.fn>
    onmessage?: (event: { data: unknown }) => void
  }[] = []
  vi.stubGlobal(
    'Worker',
    class {
      terminate = vi.fn()
      postMessage = vi.fn()
      constructor() {
        instances.push(this)
      }
    }
  )
  const client = createRegexClient((doc) => doc.toString())
  const request = {
    query: 'a+',
    replacement: '',
    options: { regexp: true, caseSensitive: true, wholeWord: false },
    wordChars: '',
    action: 'scan' as const,
    selection: { from: 0, to: 0 },
    viewport: { from: 0, to: 1 },
  }
  const first = client.run(EditorState.create({ doc: 'aaa' }).doc, request)
  const rejected = expect(first).rejects.toMatchObject({ name: 'AbortError' })
  client.reset()
  await rejected
  expect(instances[0].terminate).toHaveBeenCalledOnce()
  const next = client.run(EditorState.create({ doc: 'b' }).doc, request)
  expect(instances[1].postMessage).toHaveBeenCalledWith(expect.objectContaining({ document: 'b' }))
  instances[0].onmessage?.({ data: { id: 1 } })
  const response = { id: 2, scan: { positions: [], lengths: [], truncated: false }, highlights: [] }
  instances[1].onmessage?.({ data: response })
  expect(await next).toEqual(response)
  client.reset()
})
