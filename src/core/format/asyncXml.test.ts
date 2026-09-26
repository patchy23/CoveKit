import { afterEach, expect, it, vi } from 'vitest'
import { createXmlFormatter, XML_WORKER_THRESHOLD } from './asyncXml'
import { formatXml, minifyXml, prettyPrintXml } from './xml'

class FakeWorker {
  static instances: FakeWorker[] = []
  onmessage?: (event: { data: unknown }) => void
  onerror?: (event: { message: string }) => void
  postMessage = vi.fn()
  terminate = vi.fn()
  constructor() {
    FakeWorker.instances.push(this)
  }
  reply() {
    const { id, text, indent, mode, loose } = this.postMessage.mock.lastCall![0]
    this.onmessage?.({
      data: {
        id,
        result: {
          ok: true,
          output: mode === 'minify' ? minifyXml(text) : prettyPrintXml(text, indent),
          ...(loose ? { loose: true } : {}),
        },
      },
    })
  }
}
afterEach(() => {
  vi.unstubAllGlobals()
  FakeWorker.instances = []
})

it('小 XML 同步处理，大 XML 保留声明、命名空间、注释和 CDATA 的原格式化结果', async () => {
  vi.stubGlobal('Worker', FakeWorker)
  const formatter = createXmlFormatter()
  expect(formatter.run('<a><b/></a>')).toEqual(formatXml('<a><b/></a>'))
  expect(FakeWorker.instances).toHaveLength(0)
  const text = `<?xml version="1.0"?><a xmlns="urn:test"><!-- 注释 --><b><![CDATA[<>&${'中'.repeat(XML_WORKER_THRESHOLD)}]]></b></a>`
  const result = formatter.run(text, 4)
  FakeWorker.instances[0].reply()
  expect(await result).toEqual(formatXml(text, 4))
  expect(FakeWorker.instances[0].terminate).toHaveBeenCalledOnce()
})

it('大 XML 结构错误仍在严格校验中失败，不发送宽松工作任务', () => {
  vi.stubGlobal('Worker', FakeWorker)
  const text = `<a><b>${'x'.repeat(XML_WORKER_THRESHOLD)}</a>`
  expect(createXmlFormatter().run(text)).toEqual(formatXml(text))
  expect(formatXml(text).ok).toBe(false)
  expect(FakeWorker.instances).toHaveLength(0)
})

it('压缩保持原有空白与非法结构处理语义，不引入额外校验', async () => {
  vi.stubGlobal('Worker', FakeWorker)
  const text = `  <a>  <b>${'x'.repeat(XML_WORKER_THRESHOLD)}  </a>  `
  const result = createXmlFormatter().run(text, 2, 'minify')
  FakeWorker.instances[0].reply()
  expect(await result).toEqual({ ok: true, output: minifyXml(text) })
})

it('替换输入和销毁终止任务，迟到结果不完成后来的请求，工作器故障可感知', async () => {
  vi.stubGlobal('Worker', FakeWorker)
  const formatter = createXmlFormatter()
  const text = `<a>${'x'.repeat(XML_WORKER_THRESHOLD)}</a>`
  const first = expect(formatter.run(text)).rejects.toMatchObject({ name: 'AbortError' })
  formatter.run('<b/>')
  await first
  const second = expect(formatter.run(text)).rejects.toMatchObject({ name: 'AbortError' })
  FakeWorker.instances[0].reply()
  formatter.destroy()
  await second
  const third = expect(formatter.run(text)).rejects.toThrow('worker failed')
  FakeWorker.instances[2].onerror?.({ message: 'worker failed' })
  await third
  for (const worker of FakeWorker.instances) expect(worker.terminate).toHaveBeenCalledOnce()
})
