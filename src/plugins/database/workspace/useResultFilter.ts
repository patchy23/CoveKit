import { computed, isReactive } from 'vue'

/** 查询结果按快照替换；收窄关键词时仅复查上轮命中行，不复制单元格文本。 */
export function useResultFilter(rows: () => string[][], query: () => string) {
  const source = computed(rows)
  const keyword = computed(() => query().trim().toLowerCase())
  let cache = new WeakMap<string[][], { keyword: string; matches: string[][] }>()
  return computed(() => {
    const current = source.value
    const term = keyword.value
    if (!term) {
      cache = new WeakMap()
      return current
    }
    // 响应式数组可能原地编辑，必须遍历全量以保持单元格依赖；原始结果快照才复用候选集。
    const previous = isReactive(current) ? undefined : cache.get(current)
    const candidates = previous && term.includes(previous.keyword) ? previous.matches : current
    // 可打印 ASCII 非字母不会因小写转换生成或消失；数字、时间及标点查询无需复制全文。
    // 非 ASCII 关键词仍走完整转换，保留 İ、K 等大小写映射语义。
    const literal = /^[\x20-\x40\x5b-\x60\x7b-\x7e]+$/.test(term)
    const matches = candidates.filter((row) =>
      row.some((cell) => (literal ? cell : cell.toLowerCase()).includes(term))
    )
    // 只保留最近一次筛选，弱键不延长被替换/关闭结果的生命周期。
    cache = new WeakMap()
    if (!isReactive(current)) cache.set(current, { keyword: term, matches })
    return matches
  })
}
