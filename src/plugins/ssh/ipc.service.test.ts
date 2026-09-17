import { expect, it, vi } from 'vitest'
import { ipc } from './ipc'

const invoke = vi.hoisted(() => vi.fn())
vi.mock('@/core/ipc/ipc', () => ({ invokeCommand: invoke }))

it('服务配置调用与 Rust 命令使用同一名称和 camelCase 载荷', async () => {
  invoke.mockResolvedValueOnce('# /etc/systemd/system/example.service\n[Service]')
  const content = await ipc.sshServiceConfig({
    connectionId: 'session',
    serviceName: 'example.service',
  })
  expect(invoke).toHaveBeenCalledWith('ssh_service_config', {
    connectionId: 'session',
    serviceName: 'example.service',
  })
  expect(content).toContain('[Service]')
})
