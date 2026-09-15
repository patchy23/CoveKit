/**
 * 工具关闭协商的行为测试（T10-1/T10-2）
 *
 * 盯的是三件事：拒绝时必须「不清理、不关闭」；允许时清理失败要能被看见；
 * 清理卡住时不能把关闭流程一起拖死。
 */
import { beforeEach, describe, expect, it, vi } from 'vitest'

const invokeCommand = vi.fn()
vi.mock('@/core/ipc/ipc', () => ({
  invokeCommand: (...args: unknown[]) => invokeCommand(...args),
}))

import {
  disposeAllTools,
  disposeToolOwners,
  hasOwners,
  negotiateToolClose,
  registerToolOwner,
  registeredToolIds,
  resetToolOwnersForTest,
  toolCloseState,
} from './toolContext'

describe('工具关闭协商', () => {
  beforeEach(() => {
    resetToolOwnersForTest()
    invokeCommand.mockReset()
    // 后端裁决的替身：与 Rust `lifecycle::compose_decision` 同规则
    // （页面上报的 blockers 任一条即拒绝，用户强制则放行并把原因原样带回）
    invokeCommand.mockImplementation(
      async (command: string, payload?: { blockers?: string[]; force?: boolean }) => {
        const blockers = payload?.blockers ?? []
        if (command === 'app_request_close') {
          return {
            proceed: payload?.force === true || blockers.length === 0,
            forced: payload?.force === true,
            blockers,
            failures: [],
          }
        }
        return { proceed: true, forced: false, blockers: [], failures: [] }
      }
    )
  })

  it('任一 owner 拒绝时既不清理也不关闭，并保留原因归属', async () => {
    const dispose = vi.fn()
    const owner = registerToolOwner('ssh', 'ssh.sessions')
    owner.onPrepare(() => '2 个会话仍在连接')
    owner.onDispose(dispose)

    const outcome = await negotiateToolClose('ssh')

    expect(outcome.ok).toBe(false)
    expect(outcome.blockers).toEqual([{ owner: 'ssh.sessions', message: '2 个会话仍在连接' }])
    expect(dispose).not.toHaveBeenCalled()
  })

  it('全部允许时执行清理，并把清理失败按 owner 回报', async () => {
    const okDispose = vi.fn()
    const first = registerToolOwner('frp', 'frp.profiles')
    first.onDispose(okDispose)
    const second = registerToolOwner('frp', 'frp.binary')
    second.onDispose(() => {
      throw new Error('停止进程失败')
    })

    const outcome = await negotiateToolClose('frp')

    expect(outcome.ok).toBe(true)
    expect(okDispose).toHaveBeenCalledTimes(1)
    expect(outcome.failures).toEqual([{ owner: 'frp.binary', message: '停止进程失败' }])
  })

  it('强制关闭跳过询问但保留「本来不同意」的记录', async () => {
    const dispose = vi.fn()
    const owner = registerToolOwner('database', 'database.pools')
    owner.setRunning(true)
    owner.onDispose(dispose)

    const outcome = await negotiateToolClose('database', 'tab', { force: true })

    expect(outcome.ok).toBe(true)
    expect(dispose).toHaveBeenCalledTimes(1)
    expect(outcome.blockers[0].message).toContain('有任务正在运行')
  })

  it('未声明 prepare 的 owner 按标记兜底，不允许静默丢弃', async () => {
    registerToolOwner('http-ws', 'http-ws.requests').setDirty(true)
    const dirtyOutcome = await negotiateToolClose('http-ws')
    expect(dirtyOutcome.ok).toBe(false)
    expect(dirtyOutcome.blockers[0].message).toBe('有未保存内容')

    resetToolOwnersForTest()
    registerToolOwner('http-ws', 'http-ws.requests').setRunning(true)
    const runningOutcome = await negotiateToolClose('http-ws')
    expect(runningOutcome.ok).toBe(false)
    expect(runningOutcome.blockers[0].message).toBe('有任务正在运行')
  })

  it('询问本身抛错按拒绝处理，不放过未保存内容', async () => {
    const owner = registerToolOwner('frp', 'frp.profiles')
    owner.onPrepare(() => {
      throw new Error('后端不可用')
    })

    const outcome = await negotiateToolClose('frp')

    expect(outcome.ok).toBe(false)
    expect(outcome.blockers[0].message).toContain('后端不可用')
  })

  it('清理卡住时按超时失败收口，不阻塞关闭', async () => {
    const owner = registerToolOwner('ssh', 'ssh.terminal')
    owner.onDispose(() => new Promise<void>(() => {}))

    const outcome = await negotiateToolClose('ssh', 'tab', { timeoutMs: 20 })

    expect(outcome.ok).toBe(true)
    expect(outcome.timedOut).toBe(true)
    expect(outcome.failures[0].message).toContain('未完成')
  })

  it('退出时清理全部工具，单个失败不影响其余工具', async () => {
    registerToolOwner('frp', 'frp.profiles').onDispose(() => {
      throw new Error('停止失败')
    })
    const sshDispose = vi.fn()
    registerToolOwner('ssh', 'ssh.sessions').onDispose(sshDispose)

    const outcome = await disposeAllTools('exit')

    expect(sshDispose).toHaveBeenCalledTimes(1)
    expect(outcome.failures.map((f) => f.owner)).toEqual(['frp.profiles'])
  })

  it('注销 owner 后不再参与协商，状态与登记列表同步', () => {
    const owner = registerToolOwner('frp', 'frp.profiles')
    owner.setDirty(true)
    expect(hasOwners('frp')).toBe(true)
    expect(toolCloseState('frp')).toEqual({ dirty: true, running: false, owners: 1 })
    expect(registeredToolIds()).toContain('frp')

    owner.unregister()

    expect(hasOwners('frp')).toBe(false)
    expect(registeredToolIds()).not.toContain('frp')
    expect(toolCloseState('frp')).toEqual({ dirty: false, running: false, owners: 0 })
  })

  it('同 owner 重复登记按覆盖处理，不重复清理', async () => {
    const first = vi.fn()
    const second = vi.fn()
    registerToolOwner('database', 'database.pools').onDispose(first)
    registerToolOwner('database', 'database.pools').onDispose(second)

    await disposeToolOwners('database', 'exit')

    expect(first).not.toHaveBeenCalled()
    expect(second).toHaveBeenCalledTimes(1)
  })

  it('顺序不变式：裁决通过之前不清理任何一侧', async () => {
    const calls: string[] = []
    invokeCommand.mockImplementation(async (command: string) => {
      calls.push(command)
      return { proceed: false, forced: false, blockers: ['ssh.session: 有会话'], failures: [] }
    })
    registerToolOwner('ssh', 'ssh.sessions').onDispose(() => {
      calls.push('frontend-dispose')
    })

    const outcome = await negotiateToolClose('ssh')

    expect(outcome.ok).toBe(false)
    expect(calls).toEqual(['app_request_close'])
    expect(outcome.blockers).toContainEqual({ owner: 'ssh.session', message: '有会话' })
  })

  it('后端拒绝时页签留在原地，两侧原因合并后不重复展示', async () => {
    registerToolOwner('frp', 'frp.profiles').setRunning(true)

    const outcome = await negotiateToolClose('frp')

    expect(outcome.ok).toBe(false)
    // 页面上报的「运行中」与后端原样带回的同一条原因只出现一次
    expect(outcome.blockers).toEqual([{ owner: 'frp.profiles', message: '有任务正在运行' }])
  })

  it('裁决通过后先清前端 owner 再提交后端，后端清理失败按失败回报', async () => {
    const calls: string[] = []
    registerToolOwner('frp', 'frp.profiles').onDispose(() => {
      calls.push('frontend-dispose')
    })
    invokeCommand.mockImplementation(async (command: string) => {
      calls.push(command)
      if (command === 'app_commit_close') {
        return { proceed: true, forced: false, blockers: [], failures: ['frp: 结束 frpc 失败'] }
      }
      return { proceed: true, forced: false, blockers: [], failures: [] }
    })

    const outcome = await negotiateToolClose('frp')

    expect(calls).toEqual(['app_request_close', 'frontend-dispose', 'app_commit_close'])
    expect(outcome.ok).toBe(true)
    expect(outcome.failures).toEqual([{ owner: 'frp', message: '结束 frpc 失败' }])
  })

  it('问不到后端时不允许关闭：关到一半比关不掉更难查', async () => {
    const dispose = vi.fn()
    registerToolOwner('ssh', 'ssh.sessions').onDispose(dispose)
    invokeCommand.mockRejectedValue(new Error('IPC 断了'))

    const outcome = await negotiateToolClose('ssh')

    expect(outcome.ok).toBe(false)
    expect(dispose).not.toHaveBeenCalled()
    expect(outcome.blockers[0].message).toContain('IPC 断了')
  })
})
