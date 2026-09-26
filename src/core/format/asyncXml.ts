import { formatXml, minifyXml, prepareXml, type XmlResult } from './xml'
import { createWorkerTask } from './workerTask'

export const XML_WORKER_THRESHOLD = 64 * 1024
export function createXmlFormatter() {
  const task = createWorkerTask<
    { text: string; indent: number; mode: 'format' | 'minify'; loose?: boolean },
    XmlResult
  >(
    () => new Worker(new URL('./xml.worker.ts', import.meta.url), { type: 'module' }),
    'XML 转换工作器运行失败'
  )
  function run(
    text: string,
    indent = 2,
    mode: 'format' | 'minify' = 'format'
  ): XmlResult | Promise<XmlResult> {
    task.cancel()
    if (text.length <= XML_WORKER_THRESHOLD)
      return mode === 'minify' ? { ok: true, output: minifyXml(text) } : formatXml(text, indent)
    if (mode === 'minify') return task.run({ text, indent, mode })
    // Worker 没有 DOMParser；不替换 XML 解析器，也不把严格语法错误当作宽松成功。
    const prepared = prepareXml(text)
    if (!prepared.ok) return prepared
    return task.run({ text: prepared.text, indent, mode, loose: prepared.loose })
  }
  return { run, cancel: task.cancel, destroy: task.destroy }
}
