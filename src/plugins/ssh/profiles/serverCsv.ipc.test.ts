import { expect, it, vi } from 'vitest'
import { ipc } from '../ipc'
const invoke = vi.hoisted(() => vi.fn())
vi.mock('@/core/ipc/ipc', () => ({ invokeCommand: invoke }))
it('CSV 和书签命令维持顶层 camelCase 契约，导出只传选择不返回密码', async () => {
  invoke.mockResolvedValueOnce('host,username')
  expect(await ipc.sshServerCsvRead('/input.csv')).toBe('host,username')
  expect(invoke).toHaveBeenLastCalledWith('ssh_server_csv_read', { path: '/input.csv' })
  await ipc.sshServerCsvWrite('/template.csv', 'host,username')
  expect(invoke).toHaveBeenLastCalledWith('ssh_server_csv_write', {
    path: '/template.csv',
    content: 'host,username',
  })
  invoke.mockResolvedValueOnce(2)
  expect(
    await ipc.sshServerCsvExport({
      path: '/output.csv',
      ids: ['one', 'two'],
      includePassword: false,
    })
  ).toBe(2)
  expect(invoke).toHaveBeenLastCalledWith('ssh_server_csv_export', {
    path: '/output.csv',
    ids: ['one', 'two'],
    includePassword: false,
  })
  await ipc.sshBookmarkUpdate({ profileId: 'one', bookmarks: [] })
  expect(invoke).toHaveBeenLastCalledWith('ssh_bookmark_update', {
    profileId: 'one',
    bookmarks: [],
  })
})
