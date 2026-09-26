import { EditorState } from '@codemirror/state'
import { afterEach, expect, it, vi } from 'vitest'
import { createRegexClient } from './regexSearchClient'

afterEach(() => vi.unstubAllGlobals())
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
