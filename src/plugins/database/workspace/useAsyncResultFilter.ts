import { computed, isReactive, onScopeDispose, ref, shallowRef, watch } from 'vue'
import { createResultFilterTask, needsAsyncFilter } from './resultFilterTask'
import { useResultFilter } from './useResultFilter'

/** 原始查询快照分批筛选；可原地编辑的响应式小表沿用同步依赖追踪。 */
export function useAsyncResultFilter(rows: () => string[][], query: () => string) {
  const source = computed(rows)
  const keyword = computed(() => query().trim().toLowerCase())
  const asynchronous = computed(() => !isReactive(source.value) && needsAsyncFilter(source.value))
  const synchronous = useResultFilter(rows, () => (asynchronous.value ? '' : query()))
  const result = shallowRef<string[][]>([])
  const filtering = ref(false)
  const filterError = ref('')
  const task = createResultFilterTask()
  let revision = 0
  let completion: Promise<void> = Promise.resolve()
  let previous: { source: string[][]; term: string; matches: string[][] } | undefined
  watch(
    [source, keyword, asynchronous],
    ([current, term, async]) => {
      const request = ++revision
      task.destroy()
      filtering.value = false
      filterError.value = ''
      result.value = []
      if (!term || !async) {
        previous = undefined
        return
      }
      const candidates =
        previous?.source === current && term.includes(previous.term) ? previous.matches : current
      previous = undefined
      filtering.value = true
      completion = task
        .run(candidates, term)
        .then((matches) => {
          if (request !== revision) return
          result.value = matches
          previous = { source: current, term, matches }
        })
        .catch((error: unknown) => {
          if (request === revision)
            filterError.value = error instanceof Error ? error.message : '筛选失败'
        })
        .finally(() => {
          if (request === revision) filtering.value = false
        })
    },
    { immediate: true, flush: 'sync' }
  )
  onScopeDispose(() => {
    revision++
    task.destroy()
    previous = undefined
    result.value = []
    filtering.value = false
  })
  const filteredRows = computed(() => {
    if (!keyword.value) return source.value
    return asynchronous.value ? result.value : synchronous.value
  })
  async function waitForFilter() {
    const request = revision
    if (filtering.value) await completion
    if (request !== revision) throw new DOMException('筛选条件或结果已变化，请重试', 'AbortError')
    if (filterError.value) throw new Error(filterError.value)
    return filteredRows.value
  }
  return { filteredRows, filtering, filterError, waitForFilter }
}
