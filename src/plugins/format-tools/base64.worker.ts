/** 大文本编解码沿用面板纯函数；不将跨块的 UTF-8 或 padding 语义另写一套。 */
import { decodeBase64, encodeBase64 } from './useBase64'

self.onmessage = (
  event: MessageEvent<{ id: number; text: string; direction: 'encode' | 'decode' }>
) => {
  const { id, text, direction } = event.data
  self.postMessage({ id, result: direction === 'encode' ? encodeBase64(text) : decodeBase64(text) })
}
