/**
 * 退出协商的行为测试（T10-3）
 *
 * 关键语义：退出前先跑本进程工具清理；被拒绝时把原因交回调用方（界面必须能显示），
 * 而不是抛错或静默；强制退出走独立命令，不混用普通退出参数。
 */
import { beforeEach, describe, expect, it, vi } from 'vitest'

const invokeCommand = vi.fn()
vi.mock('@/core/ipc/ipc', () => ({
  invokeCommand: (...args: unknown[]) => invokeCommand(...args),
}))

const listenMock = vi.fn()
vi.mock('@tauri-apps/api/event', () => ({
  listen: (...args: unknown[]) => listenMock(...args),
}))

import { EXIT_VETO_EVENT, forceAppExit, requestAppExit, watchExitVeto } from './appClose'
import { registerToolOwner, resetToolOwnersForTest } from './toolContext'

describe('应用退出协商', () => {
  beforeEach(() => {
    resetToolOwnersForTest()
    invokeCommand.mockReset()
    listenMock.mockReset()
  })

  it('退出前先清理已登记工具，再请后端执行退出', async () => {
    const order: string[] = []
    registerToolOwner('frp', 'frp.profiles').onDispose(() => {
      order.push('dispose')
    })
    invokeCommand.mockImplementation(async (command: string) => {
      order.push(`invoke:${command}`)
      return { started: true, forced: false, blockers: [] }
    })

    const decision = await requestAppExit('exit')

    expect(order).toEqual(['dispose', 'invoke:app_request_exit'])
    expect(invokeCommand).toHaveBeenCalledWith('app_request_exit', { reason: 'exit' })
    expect(decision.started).toBe(true)
  })

  it('被业务拒绝时原样返回原因，不抛错也不退出', async () => {
    invokeCommand.mockResolvedValue({
      started: false,
      forced: false,
      blockers: ['ssh.sessions: 2 个会话仍在连接'],
    })

    const decision = await requestAppExit()

    expect(decision.started).toBe(false)
    expect(decision.blockers).toEqual(['ssh.sessions: 2 个会话仍在连接'])
  })

  it('清理失败不阻止退出，但仍要留下痕迹', async () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {})
    registerToolOwner('ssh', 'ssh.sessions').onDispose(() => {
      throw new Error('断开失败')
    })
    invokeCommand.mockResolvedValue({ started: true, forced: true, blockers: [] })

    const decision = await requestAppExit('exit')

    expect(decision.started).toBe(true)
    expect(warn).toHaveBeenCalled()
    warn.mockRestore()
  })

  it('强制退出走独立命令', async () => {
    invokeCommand.mockResolvedValue({ started: true, forced: true, blockers: [] })

    await forceAppExit()

    expect(invokeCommand).toHaveBeenCalledWith('app_force_exit')
  })

  it('订阅退出拒绝事件并支持退订', async () => {
    const handler = vi.fn()
    const unlisten = vi.fn()
    listenMock.mockResolvedValue(unlisten)

    const stop = await watchExitVeto(handler)

    expect(listenMock.mock.calls[0][0]).toBe(EXIT_VETO_EVENT)
    // 触发一次事件（模拟后端唤到前台后发来的拒绝原因）
    const listener = listenMock.mock.calls[0][1] as (event: { payload: unknown }) => void
    listener({
      payload: { started: false, forced: false, blockers: ['frp.profiles: 有档案在运行'] },
    })
    expect(handler).toHaveBeenCalledWith({
      started: false,
      forced: false,
      blockers: ['frp.profiles: 有档案在运行'],
    })

    stop()
    expect(unlisten).toHaveBeenCalledTimes(1)
  })

  it('事件通道不可用时退化为空订阅，不让界面起不来', async () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {})
    listenMock.mockRejectedValue(new Error('无事件通道'))

    const stop = await watchExitVeto(vi.fn())

    expect(typeof stop).toBe('function')
    expect(warn).toHaveBeenCalled()
    warn.mockRestore()
  })
})
