import { expect, it, vi } from 'vitest'
import { ipc } from './ipc'

const invoke = vi.hoisted(() => vi.fn())
vi.mock('@/core/ipc/ipc', () => ({ invokeCommand: invoke }))

it('打开日志目录使用无路径和会话参数的 owner 命令', async () => {
  invoke.mockResolvedValueOnce(undefined)
  await ipc.terminalLogOpenDir()
  expect(invoke).toHaveBeenCalledWith('ssh_terminal_log_open_dir', {})
})

it('服务和容器日志基线经过原 owner 命令完整传递', async () => {
  const common = { connectionId: 'c', lines: 2000, previousFingerprint: 'fingerprint' }
  await ipc.sshServiceLogs({ ...common, serviceName: 'sshd' })
  expect(invoke).toHaveBeenLastCalledWith('ssh_service_logs', { ...common, serviceName: 'sshd' })
  await ipc.sshDockerLogs({ ...common, containerId: 'container' })
  expect(invoke).toHaveBeenLastCalledWith('ssh_docker_logs', {
    ...common,
    containerId: 'container',
  })
})
