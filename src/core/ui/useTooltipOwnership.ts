import { onScopeDispose, readonly, shallowRef, type ShallowRef } from 'vue'

/** 当前指针或键盘目标的唯一提示归属；旧目标的延迟回调不能重新抢占。 */
// 只持有当前目标，不让每个列表行订阅一个全局响应式值。
let activeTooltip: { owned: ShallowRef<boolean> } | null = null

export function useTooltipOwnership() {
  const owner = { owned: shallowRef(false) }
  let disposed = false
  function claim() {
    if (disposed || activeTooltip === owner) return
    const previous = activeTooltip
    activeTooltip = owner
    if (previous) previous.owned.value = false
    owner.owned.value = activeTooltip === owner
  }
  function release() {
    if (activeTooltip !== owner) return
    activeTooltip = null
    owner.owned.value = false
  }
  onScopeDispose(() => {
    disposed = true
    release()
  })
  return { isOwner: readonly(owner.owned), claim, release }
}
