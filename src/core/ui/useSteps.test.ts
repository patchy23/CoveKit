/**
 * 向导步骤态用例（sync L2）
 *
 * 关键口径：不可前进的步骤禁止跳过（不能跳过密码步直接提交）；回退不受前进条件影响；
 * 末步没有「下一步」；空步骤列表是编程错误，直接抛而不是静默给一个空向导。
 */
import { ref } from 'vue'
import { describe, expect, it } from 'vitest'
import { useSteps } from '@/core/ui/useSteps'

const STEPS = ['select', 'confirm', 'save'] as const

describe('useSteps', () => {
  it('初始停在首步，首步不能回退、末步没有下一步', () => {
    const api = useSteps(STEPS)
    expect(api.current.value).toBe('select')
    expect(api.canGoBack.value).toBe(false)
    expect(api.canGoNext.value).toBe(true)
    expect(api.back()).toBe(false)
    expect(api.index.value).toBe(0)

    api.next()
    api.next()
    expect(api.current.value).toBe('save')
    expect(api.isLast.value).toBe(true)
    expect(api.canGoNext.value).toBe(false)
    expect(api.next()).toBe(false)
    expect(api.index.value).toBe(2)
  })

  it('按步判定能否前进：禁用步阻止下一步，回退不受影响', () => {
    const api = useSteps(STEPS, { canAdvance: (step) => step !== 'confirm' })
    expect(api.canAdvance('select')).toBe(true)
    expect(api.canAdvance()).toBe(true)

    expect(api.next()).toBe(true)
    expect(api.current.value).toBe('confirm')
    // 当前步不可前进：停在原地，不产生「点了没反应」的静默前跳
    expect(api.canGoNext.value).toBe(false)
    expect(api.next()).toBe(false)
    expect(api.index.value).toBe(1)

    // 回退只看位置，不看好坏
    expect(api.back()).toBe(true)
    expect(api.current.value).toBe('select')
  })

  it('前进条件随外部状态变化（勾选为空时禁止进入下一步）', () => {
    const ready = ref(false)
    const api = useSteps(STEPS, { canAdvance: () => ready.value })
    expect(api.canGoNext.value).toBe(false)

    ready.value = true
    expect(api.canGoNext.value).toBe(true)
  })

  it('跳步：可以回退，向前必须沿途每步都能前进', () => {
    const api = useSteps(STEPS, { canAdvance: (step) => step !== 'select' })
    api.index.value = 2

    expect(api.goTo('select')).toBe(true)
    expect(api.index.value).toBe(0)
    // 首步不可前进 → 不能跳过它跳到 confirm
    expect(api.goTo('confirm')).toBe(false)
    expect(api.index.value).toBe(0)
    // 未知步骤不移动
    expect(api.goTo('nope' as 'select')).toBe(false)
  })

  it('reset 回到首步', () => {
    const api = useSteps(STEPS)
    api.next()
    api.next()
    api.reset()
    expect(api.index.value).toBe(0)
    expect(api.current.value).toBe('select')
  })

  it('空步骤列表直接报错，不返回空向导', () => {
    expect(() => useSteps([] as string[])).toThrow('useSteps 至少需要一个步骤')
  })
})
