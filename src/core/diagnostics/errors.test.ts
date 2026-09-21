/** 全局异常保留内存详情，落盘只发送固定摘要；日志发送失败不再进入上报链路。 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createApp, type App } from 'vue'
import { installErrorCollectors, clearErrors, listErrors } from './errors'

const mocks = vi.hoisted(() => ({ desktop: true, error: vi.fn() }))
vi.mock('@tauri-apps/api/core', () => ({ isTauri: () => mocks.desktop }))
vi.mock('@tauri-apps/plugin-log', () => ({ error: mocks.error }))

const listeners: Array<[string, EventListenerOrEventListenerObject]> = []
let app: App

beforeEach(() => {
  app = createApp({})
  clearErrors()
  mocks.desktop = true
  mocks.error.mockReset().mockResolvedValue(undefined)
  const original = window.addEventListener.bind(window)
  vi.spyOn(window, 'addEventListener').mockImplementation((type, listener, options) => {
    listeners.push([type, listener])
    original(type, listener, options)
  })
})

afterEach(() => {
  for (const [type, listener] of listeners.splice(0)) window.removeEventListener(type, listener)
  vi.restoreAllMocks()
  clearErrors()
})

describe('全局异常日志', () => {
  it('异常原文留在内存清单，文件日志不包含秘密和堆栈', async () => {
    installErrorCollectors(app)
    app.config.errorHandler?.(new Error('password=fixture-secret'), null, 'render')
    await Promise.resolve()
    expect(listErrors()[0]?.message).toContain('fixture-secret')
    expect(mocks.error).toHaveBeenCalledExactlyOnceWith('前端异常 code=ui.render_failed')
  })

  it('日志 API 拒绝时只输出安全本地提示，不递归记录或替换原异常', async () => {
    const warning = vi.spyOn(console, 'warn').mockImplementation(() => {})
    mocks.error.mockRejectedValue(new Error('transport-secret'))
    installErrorCollectors(app)
    window.dispatchEvent(new ErrorEvent('error', { message: '原始故障' }))
    await Promise.resolve()
    await Promise.resolve()
    expect(mocks.error).toHaveBeenCalledTimes(1)
    expect(warning).toHaveBeenCalledExactlyOnceWith('[diagnostics] 日志发送失败')
    expect(listErrors()).toHaveLength(1)
    expect(listErrors()[0]?.message).toBe('原始故障')
  })

  it('浏览器预览继续收集错误，但不调用桌面日志接口', () => {
    mocks.desktop = false
    installErrorCollectors(app)
    app.config.errorHandler?.(new Error('预览异常'), null, 'render')
    expect(listErrors()).toHaveLength(1)
    expect(mocks.error).not.toHaveBeenCalled()
  })
})
