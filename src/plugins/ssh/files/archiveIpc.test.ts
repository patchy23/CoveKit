import { expect, it, vi } from 'vitest'
import { ipc } from '../ipc'
import type { ArchiveEvent, ArchiveRequest } from '../contracts'
const invoke = vi.hoisted(() => vi.fn())
vi.mock('@/core/ipc/ipc', () => ({ invokeCommand: invoke }))
vi.mock('@tauri-apps/api/core', () => ({
  Channel: class {
    onmessage: (event: ArchiveEvent) => void = () => {}
  },
}))
it('归档必需 Channel 使用顶层参数，结束后释放回调', async () => {
  let finish!: (result: ArchiveEvent) => void
  invoke.mockImplementationOnce(
    () =>
      new Promise((resolve) => {
        finish = resolve
      })
  )
  const request: ArchiveRequest = {
    operation: 'preview',
    format: 'zip',
    paths: ['/a.zip'],
    output: '',
  }
  const receive = vi.fn()
  const pending = ipc.sshArchiveRun('s', 'archive-1', request, receive)
  const channel = invoke.mock.calls[0][1].progress
  expect(invoke).toHaveBeenCalledWith('ssh_archive_run', {
    connectionId: 's',
    jobId: 'archive-1',
    request,
    progress: channel,
  })
  channel.onmessage({ kind: 'queued' })
  finish({ kind: 'result', status: 'succeeded' })
  await pending
  channel.onmessage({ kind: 'progress' })
  expect(receive).toHaveBeenCalledTimes(1)
  await ipc.sshArchiveCancel('archive-1')
  expect(invoke).toHaveBeenLastCalledWith('ssh_archive_cancel', { jobId: 'archive-1' })
})
it('目录查询分别绑定终端身份与本机目录偏好', async () => {
  await ipc.sshTerminalDirectory('term-1')
  expect(invoke).toHaveBeenLastCalledWith('ssh_terminal_directory', { terminalId: 'term-1' })
  await ipc.sshLocalDefaultDirectory('/Downloads')
  expect(invoke).toHaveBeenLastCalledWith('ssh_local_default_directory', {
    preferred: '/Downloads',
  })
})
