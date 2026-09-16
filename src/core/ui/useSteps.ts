/**
 * useSteps · 向导步骤态的最小实现（sync L2）
 *
 * 为什么不是「向导框架组件」：三个向导的步骤数、每步能前进的条件、以及每步要读的状态
 * 都不一样（导出看勾选与密码，导入看校验结果与命名），抽成组件只能把条件判断塞进插槽，
 * 反而更难读。这里只提供步骤游标与可用性判定（方案 §7「不自建向导框架组件」），
 * 步骤内容仍写在各向导里。
 *
 * 边界：只允许向后回退；向前跳步必须每一步都能前进，避免「跳过密码步直接提交」。
 */
import { computed, ref, type ComputedRef, type Ref } from 'vue'

export interface StepsOptions<K extends string> {
  /** 某一步能否前进（默认都能）；返回 false 的步骤会禁用「下一步」 */
  canAdvance?: (step: K) => boolean
}

export interface StepsApi<K extends string> {
  /** 步骤名（顺序即展示顺序） */
  steps: readonly K[]
  /** 当前步下标 */
  index: Ref<number>
  /** 当前步名 */
  current: ComputedRef<K>
  isFirst: ComputedRef<boolean>
  isLast: ComputedRef<boolean>
  /** 指定步（默认当前步）能否前进 */
  canAdvance: (step?: K) => boolean
  /** 「下一步」是否可用（末步恒 false） */
  canGoNext: ComputedRef<boolean>
  /** 「上一步」是否可用 */
  canGoBack: ComputedRef<boolean>
  /** 前进一步；不可前进时返回 false 且不改状态 */
  next: () => boolean
  /** 回退一步；已在首步时返回 false */
  back: () => boolean
  /** 跳到指定步：只能回退，或前进到「沿途每步都能前进」的目标 */
  goTo: (step: K) => boolean
  /** 回到首步（重新打开向导时用） */
  reset: () => void
}

export function useSteps<K extends string>(
  steps: readonly K[],
  options: StepsOptions<K> = {}
): StepsApi<K> {
  if (steps.length === 0) throw new Error('useSteps 至少需要一个步骤')

  const index = ref(0)
  const current = computed(() => steps[Math.min(index.value, steps.length - 1)])
  const isFirst = computed(() => index.value === 0)
  const isLast = computed(() => index.value >= steps.length - 1)

  function canAdvance(step: K = current.value): boolean {
    return options.canAdvance?.(step) ?? true
  }

  const canGoNext = computed(() => !isLast.value && canAdvance())
  const canGoBack = computed(() => !isFirst.value)

  /** 从当前步到目标步之间的每一步是否都能前进（不含目标步本身） */
  function pathOpen(target: number): boolean {
    const from = Math.min(index.value, target)
    const to = Math.max(index.value, target)
    for (let i = from; i < to; i += 1) {
      if (!canAdvance(steps[i])) return false
    }
    return true
  }

  function next(): boolean {
    if (!canGoNext.value) return false
    index.value += 1
    return true
  }

  function back(): boolean {
    if (!canGoBack.value) return false
    index.value -= 1
    return true
  }

  function goTo(step: K): boolean {
    const target = steps.indexOf(step)
    if (target < 0 || target === index.value) return false
    // 回退不受前进条件影响（已走过的步骤是用户看过的事实）；
    // 向前跳步必须沿途每步都能前进，不能借跳步绕过密码/确认步
    if (target > index.value && !pathOpen(target)) return false
    index.value = target
    return true
  }

  function reset(): void {
    index.value = 0
  }

  return {
    steps,
    index,
    current,
    isFirst,
    isLast,
    canAdvance,
    canGoNext,
    canGoBack,
    next,
    back,
    goTo,
    reset,
  }
}
