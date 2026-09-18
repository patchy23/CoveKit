/**
 * useAutoConvert 测试 · 防抖合并、立即执行取消排队、作用域销毁清定时器
 * 注意：Vue watch 回调走调度器微任务，改 ref 后须先 nextTick 再推进假定时器。
 */
import { effectScope, nextTick, ref } from 'vue'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { useAutoConvert } from './useAutoConvert'

afterEach(() => {
  vi.useRealTimers()
})

describe('useAutoConvert', () => {
  it('输入变化防抖执行：连续输入只触发一次', async () => {
    vi.useFakeTimers()
    const scope = effectScope()
    const input = ref('')
    const run = vi.fn()
    scope.run(() => useAutoConvert(run).watchInput(input))

    input.value = 'a'
    input.value = 'ab'
    input.value = 'abc'
    await nextTick()
    vi.advanceTimersByTime(299)
    expect(run).not.toHaveBeenCalled()
    vi.advanceTimersByTime(1)
    expect(run).toHaveBeenCalledTimes(1)

    scope.stop()
  })

  it('runNow 立即执行并取消排队中的防抖任务', async () => {
    vi.useFakeTimers()
    const scope = effectScope()
    const input = ref('')
    const run = vi.fn()
    let api: ReturnType<typeof useAutoConvert>
    scope.run(() => {
      api = useAutoConvert(run)
      api.watchInput(input)
    })

    input.value = 'a'
    await nextTick()
    api!.runNow()
    expect(run).toHaveBeenCalledTimes(1)
    // 排队任务已被取消，推进计时不再重复执行
    vi.advanceTimersByTime(1000)
    expect(run).toHaveBeenCalledTimes(1)

    scope.stop()
  })

  it('作用域销毁后定时器被清理，不再执行', async () => {
    vi.useFakeTimers()
    const scope = effectScope()
    const input = ref('')
    const run = vi.fn()
    scope.run(() => useAutoConvert(run).watchInput(input))

    input.value = 'a'
    await nextTick()
    scope.stop()
    vi.advanceTimersByTime(1000)
    expect(run).not.toHaveBeenCalled()
  })
})
