import { expect, it, vi } from 'vitest'
import { ipc } from '../../ipc'
const invoke = vi.hoisted(() => vi.fn().mockResolvedValue(null))
vi.mock('@/core/ipc/ipc', () => ({ invokeCommand: invoke }))
it('独立窗口走 SSH 专用命令，载荷不包含任意 URL 或认证数据', async () => {
  const token = '12345678-1234-1234-1234-123456789abc'
  for (const action of ['open', 'show', 'focus', 'close', 'return'] as const) {
    await ipc.sshEditorWindow(token, action)
    expect(invoke).toHaveBeenLastCalledWith('ssh_editor_window', { token, action })
  }
})
