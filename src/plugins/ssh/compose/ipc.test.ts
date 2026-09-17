import { expect, it, vi } from 'vitest'
import { ipc } from '../ipc'

const invoke = vi.hoisted(() => vi.fn())
vi.mock('@/core/ipc/ipc', () => ({ invokeCommand: invoke }))

it('Compose 的三个命令名称与顶层 camelCase 载荷保持 Rust 契约', async () => {
  const project = {
    name: 'app',
    status: 'running(1)',
    configFiles: ['/opt/app/compose.yaml', '/opt/app/prod.yaml'],
  }
  invoke.mockResolvedValueOnce([project])
  expect(await ipc.sshComposeList('session')).toEqual([project])
  expect(invoke).toHaveBeenLastCalledWith('ssh_compose_list', { connectionId: 'session' })
  const result = { exitCode: 1, stdout: '', stderr: 'failed' }
  invoke.mockResolvedValueOnce(result)
  expect(await ipc.sshComposeAction({ connectionId: 'session', project, action: 'up' })).toEqual(
    result
  )
  expect(invoke).toHaveBeenLastCalledWith('ssh_compose_action', {
    connectionId: 'session',
    project,
    action: 'up',
  })
  await ipc.sshComposeCreate({
    connectionId: 'session',
    remotePath: '/opt/new/compose.yaml',
    content: 'services: {}',
  })
  expect(invoke).toHaveBeenLastCalledWith('ssh_compose_create', {
    connectionId: 'session',
    remotePath: '/opt/new/compose.yaml',
    content: 'services: {}',
  })
})
