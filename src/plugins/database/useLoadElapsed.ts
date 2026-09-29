import { computed, onScopeDispose, ref, watch } from 'vue'

/** 在异步加载期间显示经过时间，并随拥有它的视图一起清理计时器。 */
export function useLoadElapsed(isLoading: () => boolean) {
  const elapsedMs = ref(0)
  let startedAt = 0
  let timer: ReturnType<typeof setInterval> | undefined

  watch(
    isLoading,
    (loading) => {
      if (timer) clearInterval(timer)
      timer = undefined
      elapsedMs.value = 0
      if (!loading) return

      startedAt = Date.now()
      timer = setInterval(() => {
        elapsedMs.value = Date.now() - startedAt
      }, 100)
    },
    { immediate: true }
  )

  onScopeDispose(() => {
    if (timer) clearInterval(timer)
  })

  return computed(() => `${(elapsedMs.value / 1000).toFixed(1)} 秒`)
}
