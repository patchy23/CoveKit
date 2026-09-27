/** 只返回语法结论，不格式化正文、不将解析树传回 WebView。 */
import { isValidJson } from '@/core/format/json'

self.onmessage = (event: MessageEvent<{ id: number; text: string }>) => {
  const { id, text } = event.data
  self.postMessage({ id, result: isValidJson(text) })
}
