import { expect, it, vi } from 'vitest'
import { ipc } from './ipc'
const invoke = vi.hoisted(() => vi.fn())
vi.mock('@/core/ipc/ipc', () => ({ invokeCommand: invoke }))

it('查询与云解析只读命令携带同一取消标识，写命令契约不变', async () => {
  await ipc.dnsReadPrepare()
  await ipc.dnsQuery('example.com', 'A', ['system'], 'q')
  await ipc.dnsDomains('aliyun', 'd')
  await ipc.dnsRecords('aliyun', 'example.com', 1, 50, '', 'r')
  await ipc.dnsReadCancel('q')
  expect(invoke.mock.calls).toEqual([
    ['dns_read_prepare', {}],
    ['dns_query', { domain: 'example.com', rtype: 'A', servers: ['system'], requestId: 'q' }],
    ['dns_domains', { platform: 'aliyun', requestId: 'd' }],
    [
      'dns_records',
      { platform: 'aliyun', domain: 'example.com', page: 1, size: 50, keyword: '', requestId: 'r' },
    ],
    ['dns_read_cancel', { requestId: 'q' }],
  ])
})
