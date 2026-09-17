import { afterEach, expect, it, vi } from 'vitest'
import type { Terminal } from 'xterm'
import type { FitAddon } from '@xterm/addon-fit'
import { createTerminalResizeController } from './useTerminalResize'

function setup() {
  let frame: FrameRequestCallback | null = null
  vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {
    frame = callback
    return 1
  })
  vi.stubGlobal('cancelAnimationFrame', () => {
    frame = null
  })
  const terminal = {
    cols: 80,
    rows: 24,
    resize: vi.fn((cols: number, rows: number) => {
      terminal.cols = cols
      terminal.rows = rows
    }),
  }
  const size = { cols: 100, rows: 32 }
  const host = { clientWidth: 800, clientHeight: 500 }
  const resize = vi
    .fn<(...args: [string, number, number]) => Promise<unknown>>()
    .mockResolvedValue(undefined)
  const onError = vi.fn()
  const controller = createTerminalResizeController({
    getTerminal: () => terminal as unknown as Terminal,
    getFitAddon: () => ({ proposeDimensions: () => size }) as FitAddon,
    getHost: () => host as HTMLElement,
    getTerminalId: () => 'terminal',
    resize,
    onError,
  })
  const tick = async () => {
    const callback = frame
    frame = null
    callback?.(0)
    await Promise.resolve()
    await Promise.resolve()
  }
  return { controller, terminal, size, host, resize, onError, tick }
}

afterEach(() => vi.unstubAllGlobals())

it('隐藏时不测量，显示后用新尺寸，重复渲染不重复发送 PTY 请求', async () => {
  const test = setup()
  test.host.clientHeight = 0
  test.controller.scheduleFitAndSync()
  await test.tick()
  expect(test.resize).not.toHaveBeenCalled()
  test.host.clientHeight = 500
  test.controller.scheduleFitAndSync()
  await test.tick()
  expect(test.terminal.rows).toBe(32)
  test.controller.scheduleFitAndSync()
  await test.tick()
  expect(test.resize).toHaveBeenCalledTimes(1)
  test.controller.cancelScheduledFit()
})

it('在途请求串行完成，快速尺寸变化只发最后一次；卸载抑制晚到错误', async () => {
  const test = setup()
  let finish!: () => void
  test.resize.mockImplementationOnce(
    () =>
      new Promise<void>((resolve) => {
        finish = resolve
      })
  )
  test.controller.scheduleFitAndSync()
  await test.tick()
  test.size.rows = 33
  test.controller.scheduleFitAndSync()
  await test.tick()
  test.size.rows = 34
  test.controller.scheduleFitAndSync()
  await test.tick()
  expect(test.resize).toHaveBeenCalledTimes(1)
  finish()
  await test.tick()
  expect(test.resize).toHaveBeenLastCalledWith('terminal', 100, 34)
  let fail!: (reason: Error) => void
  test.resize.mockImplementationOnce(
    () =>
      new Promise((_, reject) => {
        fail = reject
      })
  )
  test.size.rows = 35
  test.controller.scheduleFitAndSync()
  await test.tick()
  test.controller.cancelScheduledFit()
  fail(new Error('late'))
  await test.tick()
  expect(test.onError).not.toHaveBeenCalled()
})
