/**
 * 工具关闭协商的行为测试（T10-1/T10-2）
 *
 * 盯的是三件事：拒绝时必须「不清理、不关闭」；允许时清理失败要能被看见；
 * 清理卡住时不能把关闭流程一起拖死。
 */
import { beforeEach, describe, expect, it, vi } from 'vitest'
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
})
