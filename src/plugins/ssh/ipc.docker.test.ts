import { expect, it, vi } from 'vitest'
import { ipc } from './ipc'
const invoke = vi.hoisted(() => vi.fn())
vi.mock('@/core/ipc/ipc', () => ({ invokeCommand: invoke }))

it('Docker 全量查询保持原载荷，编排查询只增加项目过滤', async () => {
  invoke.mockResolvedValue([])
  await ipc.sshDockerList('session')
  expect(invoke).toHaveBeenLastCalledWith('ssh_docker_list', { connectionId: 'session' })
  await ipc.sshDockerList('session', 'my-app')
  expect(invoke).toHaveBeenLastCalledWith('ssh_docker_list', {
    connectionId: 'session',
    composeProject: 'my-app',
  })
})
