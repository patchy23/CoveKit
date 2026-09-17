import { expect, it, vi } from 'vitest'
import { ipc } from '../ipc'

const invoke = vi.hoisted(() => vi.fn())
vi.mock('@/core/ipc/ipc', () => ({ invokeCommand: invoke }))
vi.mock('@tauri-apps/api/core', () => ({
  Channel: class {
    onmessage: (value: [boolean, number[]]) => void = () => {}
  },
}))

it('Compose 命令名称与顶层 camelCase 载荷保持 Rust 契约', async () => {
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
  invoke.mockResolvedValueOnce('/home/remote')
  expect(await ipc.sshComposeHome('session')).toBe('/home/remote')
  expect(invoke).toHaveBeenLastCalledWith('ssh_compose_home', { connectionId: 'session' })
})

it('执行流按 stdout 与 stderr 独立解码跨包 UTF-8，完成后忽略迟到回调', async () => {
  let finish!: (result: unknown) => void
  invoke.mockImplementationOnce(
    () =>
      new Promise((resolve) => {
        finish = resolve
      })
  )
  const chunks: string[] = []
  const pending = ipc.sshComposeStream(
    {
      connectionId: 's',
      project: { name: 'p', status: '', configFiles: ['/p.yml'], workingDir: '/srv/p' },
      action: 'up',
    },
    (text) => chunks.push(text)
  )
  const channel = invoke.mock.calls.at(-1)![1].progress
  const bytes = [...new TextEncoder().encode('完成')]
  channel.onmessage([false, bytes.slice(0, 2)])
  channel.onmessage([true, [...new TextEncoder().encode('提示')]])
  channel.onmessage([false, bytes.slice(2)])
  finish({ exitCode: 0, stdout: '完成', stderr: '提示' })
  await pending
  expect(chunks.join('')).toBe('提示完成')
  channel.onmessage([false, bytes])
  expect(chunks.join('')).toBe('提示完成')
})
