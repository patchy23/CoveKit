import { computed, onScopeDispose, shallowRef } from 'vue'

/** 当前指针或键盘目标的唯一提示归属；旧目标的延迟回调不能重新抢占。 */
const activeTooltip = shallowRef<symbol | null>(null)

export function useTooltipOwnership() {
  const id = Symbol('tooltip')
  const isOwner = computed(() => activeTooltip.value === id)
  function claim() {
    activeTooltip.value = id
  }
  function release() {
    if (activeTooltip.value === id) activeTooltip.value = null
  }
  onScopeDispose(release)
  return { isOwner, claim, release }
}
