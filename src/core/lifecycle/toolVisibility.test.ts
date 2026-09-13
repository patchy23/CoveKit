/**
 * 工具可见性分发的行为测试（T10-1/T10-7）
 *
 * 三种来源必须分别可见：页签切换（active）、设置页覆盖（covered）、窗口隐藏（hidden）。
 * 压成一个 boolean 会让插件无法区分「失焦」与「关闭」，从而误断连接。
 */
import { beforeEach, describe, expect, it, vi } from 'vitest'
import {
  publishGlobalHidden,
  publishToolVisibility,
  resetToolVisibilityForTest,
  toolVisibility,
  watchToolVisibility,
} from './toolVisibility'

describe('工具可见性分发', () => {
  beforeEach(() => {
    resetToolVisibilityForTest()
  })

  it('未登记过的工具返回默认不可见状态，不抛错', () => {
    expect(toolVisibility('unknown')).toEqual({ active: false, covered: false, hidden: false })
  })

  it('分发只改变被指定的来源，其他来源保持不变', () => {
    publishToolVisibility('ssh', { active: true })
    publishToolVisibility('ssh', { covered: true })

    expect(toolVisibility('ssh')).toEqual({ active: true, covered: true, hidden: false })
  })

  it('状态未变化时不重复通知订阅者', () => {
    const handler = vi.fn()
    watchToolVisibility('frp', handler)

    publishToolVisibility('frp', { active: true })
    publishToolVisibility('frp', { active: true })

    expect(handler).toHaveBeenCalledTimes(1)
  })

  it('窗口隐藏广播到所有已登记工具', () => {
    publishToolVisibility('ssh', { active: true })
    publishToolVisibility('frp', { active: true })
    const sshHandler = vi.fn()
    const frpHandler = vi.fn()
    watchToolVisibility('ssh', sshHandler)
    watchToolVisibility('frp', frpHandler)

    publishGlobalHidden(true)

    expect(toolVisibility('ssh').hidden).toBe(true)
    expect(toolVisibility('frp').hidden).toBe(true)
    expect(sshHandler).toHaveBeenLastCalledWith({ active: true, covered: false, hidden: true })
    expect(frpHandler).toHaveBeenLastCalledWith({ active: true, covered: false, hidden: true })
  })

  it('退订后不再收到通知，且不影响其他订阅者', () => {
    const a = vi.fn()
    const b = vi.fn()
    const stopA = watchToolVisibility('database', a)
    watchToolVisibility('database', b)

    publishToolVisibility('database', { active: true })
    stopA()
    publishToolVisibility('database', { active: false })

    expect(a).toHaveBeenCalledTimes(1)
    expect(b).toHaveBeenCalledTimes(2)
  })
})
