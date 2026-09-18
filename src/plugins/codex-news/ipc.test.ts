import { expect, it, vi } from 'vitest'
import { ipc } from './ipc'
const invoke = vi.hoisted(() => vi.fn().mockResolvedValue(null))
vi.mock('@/core/ipc/ipc', () => ({ invokeCommand: invoke }))
it('与 Rust 命令清单保持名称和载荷一致', async () => {
  await ipc.fetch()
  expect(invoke).toHaveBeenLastCalledWith('codex_news_fetch')
  await ipc.load()
  expect(invoke).toHaveBeenLastCalledWith('codex_news_load')
  const value = { version: 1 as const, snapshot: null, read: {}, auto: true, checkedAt: '' }
  await ipc.save(value)
  expect(invoke).toHaveBeenLastCalledWith('codex_news_save', { value })
})
