import { expect, it } from 'vitest'
import { isSystemProcess, isSystemService } from './systemItems'
import type { ProcessInfo, SystemdService } from '../contracts'

const service = (path?: string, hasOverrides = false): SystemdService => ({
  name: 'test.service',
  description: '',
  loadState: 'loaded',
  activeState: 'active',
  subState: 'running',
  enabled: true,
  fragmentPath: path,
  hasOverrides,
})
const process = (command: string, memoryBytes = 1024, user = 'root'): ProcessInfo => ({
  pid: 10,
  command,
  memoryBytes,
  user,
  cpuPercent: 0,
  memoryPercent: 0,
  startedAt: 0,
})

it('只隐藏系统目录中已知的基础服务，保留业务、自定义和来源未知的服务', () => {
  expect(isSystemService(service('/usr/lib/systemd/system/systemd-journald.service'))).toBe(true)
  expect(isSystemService(service('/lib/systemd/system/getty@.service'))).toBe(true)
  for (const name of ['docker', 'containerd', 'nginx', 'postgresql', 'redis', 'systemd-business']) {
    expect(isSystemService(service(`/usr/lib/systemd/system/${name}.service`))).toBe(false)
  }
  expect(isSystemService(service('/etc/systemd/system/cron.service'))).toBe(false)
  expect(isSystemService(service('/lib/systemd/system/cron.service', true))).toBe(false)
  expect(isSystemService(service())).toBe(false)
})

it('识别已知内核线程和系统程序，保留 root 业务进程、僵尸及不确定项', () => {
  expect(isSystemProcess(process('[kworker/0:1-events]', 0))).toBe(true)
  expect(isSystemProcess(process('/usr/lib/systemd/systemd-journald'))).toBe(true)
  expect(isSystemProcess(process('/usr/bin/dbus-daemon --system'))).toBe(true)
  for (const command of [
    '/usr/bin/dockerd',
    '/usr/bin/python app.py',
    '/usr/sbin/nginx',
    '/opt/systemd-journald',
    'systemd-custom',
    '[worker]',
    '[nginx] <defunct>',
  ]) {
    expect(isSystemProcess(process(command, 0))).toBe(false)
  }
  expect(isSystemProcess(process('[kworker/0:1]', 10))).toBe(false)
  expect(isSystemProcess(process('[kworker/0:1]', 0, 'app'))).toBe(false)
})
