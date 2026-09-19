import { expect, it } from 'vitest'
import type { PortEntry } from './contracts'
import {
  addressText,
  copyEntry,
  endpointOf,
  entryKey,
  filterEntries,
  filterError,
  type PortFilter,
} from './entries'

const entry: PortEntry = {
  protocol: 'TCP',
  family: 'IPv4',
  localAddress: '0.0.0.0',
  localPort: 80,
  remoteAddress: null,
  remotePort: null,
  pid: 420,
  state: 'LISTEN',
  startedAt: '134029000000000000',
  processName: 'Node.EXE',
  executablePath: 'C:\\程序\\node.exe',
  detailError: null,
}
const filters: PortFilter = { search: '', by: 'port', protocol: 'all', view: 'listeners' }

it('本地端口精确匹配，保留同端口不同协议和地址族，不匹配远端端口', () => {
  const rows = [
    entry,
    { ...entry, protocol: 'UDP' as const, state: null },
    { ...entry, family: 'IPv6' as const, localAddress: '::' },
    { ...entry, localPort: 8080 },
    { ...entry, localPort: 9000, remotePort: 80, state: 'ESTABLISHED' },
  ]
  expect(filterEntries(rows, { ...filters, search: ' 80 ', view: 'all' })).toEqual(rows.slice(0, 3))
  expect(filterEntries(rows, { ...filters, search: '80', protocol: 'UDP' })).toEqual([rows[1]])
})

it('默认只显示 TCP 监听和 UDP 绑定，全部连接保留 PID 0 的系统记录', () => {
  const udp = { ...entry, protocol: 'UDP' as const, state: null }
  const system = { ...entry, state: 'TIME_WAIT', pid: 0, startedAt: null }
  expect(filterEntries([entry, udp, system], filters)).toEqual([entry, udp])
  expect(filterEntries([entry, udp, system], { ...filters, view: 'all' })).toHaveLength(3)
})

it('进程名称不区分大小写，PID 按完整数字匹配，缺失详情不会隐藏端口查询结果', () => {
  const unknown = {
    ...entry,
    pid: 4200,
    processName: null,
    startedAt: null,
    detailError: '权限不足',
  }
  expect(filterEntries([entry, unknown], { ...filters, by: 'process', search: 'NODE' })).toEqual([
    entry,
  ])
  expect(filterEntries([entry, unknown], { ...filters, by: 'pid', search: '420' })).toEqual([entry])
  expect(filterEntries([entry, unknown], { ...filters, search: '80' })).toHaveLength(2)
})

it.each(['0', '-1', '65536', '8.5', '8e2', '80abc'])('拒绝无效端口 %s', (search) => {
  expect(filterError({ ...filters, search })).toContain('1–65535')
  expect(filterEntries([entry], { ...filters, search })).toEqual([])
})

it('空条件列出全部，端口和 PID 上界有效，越界 PID 有提示', () => {
  expect(filterError(filters)).toBe('')
  expect(filterEntries([entry], filters)).toEqual([entry])
  expect(filterError({ ...filters, search: '65535' })).toBe('')
  expect(filterError({ ...filters, by: 'pid', search: '4294967295' })).toBe('')
  expect(filterError({ ...filters, by: 'pid', search: '4294967296' })).toContain('PID')
})

it('IPv6 与作用域完整复制，关闭契约只包含端点身份', () => {
  const row = {
    ...entry,
    family: 'IPv6' as const,
    localAddress: 'fe80::1%7',
    state: 'ESTABLISHED',
    remoteAddress: '::1',
    remotePort: 9000,
  }
  expect(addressText(row.localAddress, 80)).toBe('[fe80::1%7]:80')
  expect(copyEntry(row)).toContain('远端：[::1]:9000')
  expect(copyEntry(row)).toContain('C:\\程序\\node.exe')
  expect(endpointOf(row)).toEqual({
    protocol: 'TCP',
    family: 'IPv6',
    localAddress: 'fe80::1%7',
    localPort: 80,
    remoteAddress: '::1',
    remotePort: 9000,
    pid: 420,
  })
  expect(entryKey(row)).not.toBe(entryKey({ ...row, startedAt: '134029000000000001' }))
})
