/**
 * frpStatus 单测：四态映射、日志等级识别、环形缓冲截断
 */
import { describe, expect, it } from 'vitest'
import {
  appendLogLine,
  logLevel,
  logLevelClass,
  statusView,
  profileRuntimeView,
  type FrpLogLine,
} from './frpStatus'
import type { FrpProfileSummary } from '../contracts'

describe('FRP 列表运行快照', () => {
  const profile: FrpProfileSummary = {
    fileName: 'test.toml',
    displayName: 'test',
    remark: '备注',
    serverAddr: 'localhost',
    serverPort: 37000,
    proxyCount: 1,
    proxyTypes: ['TCP'],
    enabledProxyCount: 1,
    mtime: 0,
    state: 'error',
    lastError: '上次异常退出',
    pid: 123,
  }

  it('重新启动和连接成功时清除列表中的旧错误，同时保留档案信息', () => {
    for (const state of ['starting', 'running'] as const) {
      const result = profileRuntimeView(profile, { fileName: profile.fileName, state, pid: 456 })
      expect(result).toMatchObject({ state, pid: 456, remark: '备注' })
      expect(result.lastError).toBeUndefined()
    }
    expect(profile.lastError).toBe('上次异常退出')
  })

  it('新异常及时替换旧错误，进程退出时清除 PID', () => {
    const result = profileRuntimeView(profile, {
      fileName: profile.fileName,
      state: 'error',
      lastError: '新异常',
    })
    expect(result.lastError).toBe('新异常')
    expect(result.pid).toBeUndefined()
  })

  it('尚未取得运行快照时使用列表返回的状态', () => {
    expect(profileRuntimeView(profile, undefined)).toBe(profile)
  })
})

/** 造一条日志行 */
function line(text: string, ts = 0): FrpLogLine {
  return { ts, line: text, stream: 'stdout', level: logLevel(text) }
}

describe('frpStatus · 状态映射', () => {
  it('四态各有文案 key 与语义色，starting 为中间态', () => {
    expect(statusView('running').tone).toBe('success')
    expect(statusView('running').labelKey).toBe('frp.stateRunning')
    expect(statusView('error').tone).toBe('danger')
    expect(statusView('stopped').tone).toBe('neutral')
    const starting = statusView('starting')
    expect(starting.labelKey).toBe('frp.stateStarting')
    expect(starting.transient).toBe(true)
    // 只有中间态禁用按钮
    expect(statusView('running').transient).toBe(false)
    expect(statusView('error').transient).toBe(false)
  })

  it('停止与运行的状态点颜色不同（避免假绿灯观感）', () => {
    expect(statusView('running').dotClass).not.toBe(statusView('stopped').dotClass)
    expect(statusView('error').dotClass).not.toBe(statusView('running').dotClass)
  })
})

describe('frpStatus · 日志等级', () => {
  it('frpc 自身的 [E]/[W]/[I] 标记优先', () => {
    expect(logLevel('2026/09/12 10:00:00 [E] [p1] start error')).toBe('error')
    expect(logLevel('2026/09/12 10:00:00 [W] [p1] retry')).toBe('warn')
    expect(logLevel('2026/09/12 10:00:00 [I] [p1] start proxy success')).toBe('info')
  })

  it('无标记行按关键字兜底（登录失败、端口占用等要显眼）', () => {
    expect(logLevel('login to server failed: EOF')).toBe('error')
    expect(logLevel('token is incorrect')).toBe('error')
    expect(logLevel('bind: address already in use')).toBe('error')
    expect(logLevel('connection refused')).toBe('error')
    expect(logLevel('something warning here')).toBe('warn')
    expect(logLevel('start proxy success')).toBe('info')
  })

  it('三种等级有各自文本色', () => {
    const classes = [logLevelClass('error'), logLevelClass('warn'), logLevelClass('info')]
    expect(new Set(classes).size).toBe(3)
  })
})

describe('frpStatus · 环形缓冲', () => {
  it('未超限时追加', () => {
    const buffer = [line('a'), line('b')]
    const next = appendLogLine(buffer, line('c'), 2000)
    expect(next.map((item) => item.line)).toEqual(['a', 'b', 'c'])
    expect(buffer).toHaveLength(2) // 原数组不被改动
  })

  it('超限时保留最新 max 行', () => {
    let buffer: FrpLogLine[] = []
    for (let i = 0; i < 10; i += 1) {
      buffer = appendLogLine(buffer, line(`l${i}`, i), 3)
    }
    expect(buffer).toHaveLength(3)
    expect(buffer.map((item) => item.line)).toEqual(['l7', 'l8', 'l9'])
  })

  it('恰好在限内不裁剪；max=0 视为不限制', () => {
    const three = [line('a'), line('b'), line('c')]
    expect(appendLogLine(three, line('d'), 4)).toHaveLength(4)
    const unlimited = appendLogLine(three, line('d'), 0)
    expect(unlimited).toHaveLength(4)
  })
})
