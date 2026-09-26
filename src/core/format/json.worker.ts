/** 大 JSON 沿用同一格式化实现，输入修订或关闭由宿主终止计算。 */
import { formatJson } from './json'

self.onmessage = (event: MessageEvent<{ id: number; text: string; indent: number }>) => {
  const { id, text, indent } = event.data
  self.postMessage({ id, result: formatJson(text, indent) })
}
