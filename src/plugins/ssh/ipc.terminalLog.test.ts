import { expect, it, vi } from 'vitest'
import { ipc } from './ipc'

const invoke = vi.hoisted(() => vi.fn())
vi.mock('@/core/ipc/ipc', () => ({ invokeCommand: invoke }))

it('打开日志目录使用无路径和会话参数的 owner 命令', async () => {
  invoke.mockResolvedValueOnce(undefined)
  await ipc.terminalLogOpenDir()
  expect(invoke).toHaveBeenCalledWith('ssh_terminal_log_open_dir', {})
})
