import { computed, getCurrentScope, isReactive, onScopeDispose, ref, shallowRef, watch } from 'vue'
import { createResultFilterTask, needsAsyncFilter } from './resultFilterTask'

/** 查询结果按快照替换；收窄关键词时仅复查上轮命中行，不复制单元格文本。 */
export function useResultFilter(rows: () => string[][], query: () => string) {
  const source = computed(rows)
  const keyword = computed(() => query().trim().toLowerCase())
  let cache = new WeakMap<string[][], { keyword: string; matches: string[][] }>()
  const sizes = new WeakMap<string[][], boolean>()
  const task = createResultFilterTask()
  const busy = ref(false)
  const error = ref('')
  const matches = shallowRef<string[][]>([])
  let input: { rows: string[][]; term: string } | undefined
  let waiting: Promise<string[][]> | undefined
  let epoch = 0
  function invalidate() {
    epoch++
    const sameSource = input?.rows === source.value
    task.cancel(sameSource && !!keyword.value)
    input = undefined
    waiting = undefined
    busy.value = false
    error.value = ''
    if (!sameSource) matches.value = []
  }
  const stop = watch([source, keyword], invalidate, { flush: 'sync' })
  function destroy() {
    stop()
    invalidate()
    task.destroy()
  }
  if (getCurrentScope()) onScopeDispose(destroy)
  const result = computed(() => {
    const current = source.value
    const term = keyword.value
    if (!term) {
      cache = new WeakMap()
      return current
    }
    // 工作区的查询与编辑结果是不可变 markRaw 快照；原地响应式小模型保留依赖跟踪。
    if (!isReactive(current)) {
      let large = sizes.get(current)
      if (large === undefined) {
        large = needsAsyncFilter(current)
        sizes.set(current, large)
      }
      if (large) {
        if (input?.rows !== current || input.term !== term) {
          input = { rows: current, term }
          const token = ++epoch
          busy.value = true
          error.value = ''
          waiting = task.run(current, term).then(
            (indices) => {
              if (token !== epoch) throw new DOMException('筛选已取消', 'AbortError')
              const rows = Array.from(indices, (index) => current[index])
              matches.value = rows
              busy.value = false
              return rows
            },
            (reason) => {
              if (token === epoch) {
                busy.value = false
                matches.value = []
                if (!(reason instanceof Error && reason.name === 'AbortError'))
                  error.value = reason instanceof Error ? reason.message : String(reason)
              }
              throw reason
            }
          )
          // 渲染消费方通过 error 展示失败；导出消费方仍等待同一原始 Promise 获得错误。
          void waiting.catch(() => undefined)
        }
        return matches.value
      }
    }
    // 响应式数组可能原地编辑，必须遍历全量以保持单元格依赖；原始结果快照才复用候选集。
    const previous = isReactive(current) ? undefined : cache.get(current)
    const candidates = previous && term.includes(previous.keyword) ? previous.matches : current
    // 可打印 ASCII 非字母不会因小写转换生成或消失；数字、时间及标点查询无需复制全文。
    // 非 ASCII 关键词仍走完整转换，保留 İ、K 等大小写映射语义。
    const literal = /^[\x20-\x40\x5b-\x60\x7b-\x7e]+$/.test(term)
    const synchronousMatches = candidates.filter((row) =>
      row.some((cell) => (literal ? cell : cell.toLowerCase()).includes(term))
    )
    // 只保留最近一次筛选，弱键不延长被替换/关闭结果的生命周期。
    cache = new WeakMap()
    if (!isReactive(current)) cache.set(current, { keyword: term, matches: synchronousMatches })
    return synchronousMatches
  })
  async function ready() {
    const value = result.value // 触发当前版本；不读取上一轮的投影。
    const token = epoch
    const rows = waiting ? await waiting : value
    if (token !== epoch) throw new DOMException('筛选已取消', 'AbortError')
    return rows
  }
  return Object.assign(result, { busy, error, ready, destroy })
}
