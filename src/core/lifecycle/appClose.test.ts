/**
 * 退出协商的行为测试（T10-3、AR06）
 *
 * 关键语义：退出也先问页面内 owner → 后端合成裁决 → **裁决通过才清理** → 提交退出；
 * 被拒绝时把原因交回调用方（界面必须能显示），而不是抛错或静默；
 * 强制退出走独立命令，不混用普通退出参数。
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

/** 后端裁决的替身：与 Rust `lifecycle::compose_decision` 同规则（页面上报任一条即拒绝，强制则放行） */
function allowAll(): void {
  invokeCommand.mockResolvedValue({ proceed: true, forced: false, blockers: [], failures: [] })
}

function refuse(reason: string): void {
  invokeCommand.mockResolvedValue({
    proceed: false,
    forced: false,
    blockers: [reason],
    failures: [],
  })
}

describe('应用退出协商', () => {
  beforeEach(() => {
    resetToolOwnersForTest()
    invokeCommand.mockReset()
    listenMock.mockReset()
  })

  it('退出前先请后端裁决，通过后才清理前端 owner，最后提交退出', async () => {
    const order: string[] = []
    registerToolOwner('frp', 'frp.profiles').onDispose(() => {
      order.push('dispose')
    })
    invokeCommand.mockImplementation(async (command: string) => {
      order.push(`invoke:${command}`)
      return { proceed: true, forced: false, blockers: [], failures: [] }
    })

    const decision = await requestAppExit('exit')

    expect(order).toEqual(['invoke:app_request_close', 'dispose', 'invoke:app_commit_close'])
    expect(invokeCommand).toHaveBeenCalledWith('app_request_close', {
      reason: 'exit',
      blockers: [],
    })
    expect(decision.proceed).toBe(true)
  })

  it('被业务拒绝时既不清理也不退出，原因原样交回界面', async () => {
    const dispose = vi.fn()
    registerToolOwner('ssh', 'ssh.sessions').onDispose(dispose)
    refuse('ssh.sessions: 2 个会话仍在连接')

    const decision = await requestAppExit()

    expect(decision.proceed).toBe(false)
    expect(decision.blockers).toEqual(['ssh.sessions: 2 个会话仍在连接'])
    expect(dispose).not.toHaveBeenCalled()
    expect(invokeCommand).not.toHaveBeenCalledWith('app_commit_close', expect.anything())
  })

  it('页面内 owner 拒绝时把原因一起上报后端，不自行决定放行', async () => {
    registerToolOwner('http-ws', 'http-ws.requests').setDirty(true)
    allowAll()

    await requestAppExit()

    expect(invokeCommand).toHaveBeenCalledWith('app_request_close', {
      reason: 'exit',
      blockers: ['http-ws.requests: 有未保存内容'],
    })
  })

  it('清理失败不阻止退出，但仍要留下痕迹', async () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {})
    registerToolOwner('ssh', 'ssh.sessions').onDispose(() => {
      throw new Error('断开失败')
    })
    allowAll()

    const decision = await requestAppExit('exit')

    expect(decision.proceed).toBe(true)
    expect(warn).toHaveBeenCalled()
    warn.mockRestore()
  })

  it('强制退出走独立命令', async () => {
    allowAll()

    await forceAppExit()

    expect(invokeCommand).toHaveBeenCalledWith('app_force_exit')
  })

  it('问不到后端时按拒绝处理，不退化成「静默退出」', async () => {
    invokeCommand.mockRejectedValue(new Error('IPC 不可用'))

    const decision = await requestAppExit()

    expect(decision.proceed).toBe(false)
    expect(decision.blockers[0]).toContain('IPC 不可用')
  })

  it('订阅退出拒绝事件并支持退订', async () => {
    const handler = vi.fn()
    const unlisten = vi.fn()
    listenMock.mockResolvedValue(unlisten)

    const stop = await watchExitVeto(handler)

    expect(listenMock.mock.calls[0][0]).toBe(EXIT_VETO_EVENT)
    // 触发一次事件（模拟后端唤到前台后发来的拒绝原因）
    const listener = listenMock.mock.calls[0][1] as (event: { payload: unknown }) => void
    const payload = {
      proceed: false,
      forced: false,
      blockers: ['frp.profiles: 有档案在运行'],
      failures: [],
    }
    listener({ payload })
    expect(handler).toHaveBeenCalledWith(payload)

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
