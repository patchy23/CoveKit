import type { Text } from '@codemirror/state'

/** 同一文档修订的全文只序列化一次，不为撤销历史积累字符串副本。 */
export function createDocumentTextReader(): (document: Text) => string {
  // 只保留最近一次读取，弱键不延长关闭文档的生命周期。
  let cache = new WeakMap<Text, string>()
  return (document) => {
    const existing = cache.get(document)
    if (existing !== undefined) return existing
    const text = document.toString()
    cache = new WeakMap([[document, text]])
    return text
  }
}
