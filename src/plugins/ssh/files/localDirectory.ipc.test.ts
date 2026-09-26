import { afterEach, expect, it, vi } from 'vitest'
import { invokeCommand } from '@/core/ipc/ipc'
import { ipc } from '../ipc'

vi.mock('@/core/ipc/ipc', () => ({ invokeCommand: vi.fn() }))
afterEach(() => vi.clearAllMocks())

it('本地列表登记、取消和读取使用同一请求标识，旧调用仍可省略', async () => {
  await ipc.sshLocalListPrepare()
  expect(invokeCommand).toHaveBeenLastCalledWith('ssh_local_list_prepare', {})
  await ipc.sshLocalList('C:/', 'request')
  expect(invokeCommand).toHaveBeenLastCalledWith('ssh_local_list', {
    path: 'C:/',
    requestId: 'request',
  })
  await ipc.sshLocalListCancel('request')
  expect(invokeCommand).toHaveBeenLastCalledWith('ssh_local_list_cancel', { requestId: 'request' })
  await ipc.sshLocalList('C:/')
  expect(invokeCommand).toHaveBeenLastCalledWith('ssh_local_list', { path: 'C:/' })
})
