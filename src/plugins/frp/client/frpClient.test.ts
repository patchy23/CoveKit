/**
 * frpClient 纯函数单测：覆盖标题兜底、默认项置顶、绑定优先与缺失项判定。
 */
import { describe, expect, it } from 'vitest'
import type { FrpClient } from '../contracts'
import {
  clientOptions,
  clientTitle,
  effectiveClient,
  hasUsableClient,
  sourceLabelKey,
} from './frpClient'

/** 构造客户端测试数据（只覆盖被测字段，其余给稳定默认值） */
function makeClient(over: Partial<FrpClient>): FrpClient {
  return {
    id: 'id',
    label: 'frpc',
    path: 'C:/frpc.exe',
    source: 'external',
    isDefault: false,
    exists: true,
    ...over,
  }
}

/** 测试用翻译函数：直接回显 key，断言时不必依赖真实文案 */
const t = (key: string): string => key

describe('clientTitle', () => {
  it('有版本号时用版本号', () => {
    expect(clientTitle(makeClient({ version: '0.71.0' }))).toBe('frpc 0.71.0')
  })

  it('没有版本号时退回文件名（定制客户端常见）', () => {
    expect(clientTitle(makeClient({ label: 'frpc-patched' }))).toBe('frpc-patched')
  })
})

describe('clientOptions', () => {
  it('默认项置顶并带上标注', () => {
    const options = clientOptions(
      [makeClient({ id: 'a', version: '0.60.0' }), makeClient({ id: 'b', version: '0.71.0' })],
      'b',
      t
    )
    expect(options[0].value).toBe('b')
    expect(options[0].label).toContain('frp.clientIsDefault')
  })

  it('文件缺失的项保留但标注（用户需要看到绑定的去哪了）', () => {
    const options = clientOptions([makeClient({ id: 'a', exists: false })], undefined, t)
    expect(options).toHaveLength(1)
    expect(options[0].label).toContain('frp.clientMissing')
  })
})

describe('effectiveClient', () => {
  const clients = [
    makeClient({ id: 'a', label: 'a' }),
    makeClient({ id: 'b', label: 'b', isDefault: true }),
  ]

  it('绑定项优先于默认项', () => {
    expect(effectiveClient(clients, 'b', 'a')?.id).toBe('a')
  })

  it('未绑定时回落到默认项', () => {
    expect(effectiveClient(clients, 'b', undefined)?.id).toBe('b')
  })

  it('绑定的 id 在清单里找不到时回落到默认项', () => {
    expect(effectiveClient(clients, 'b', 'ghost')?.id).toBe('b')
  })

  it('没有默认项且未绑定时返回 undefined', () => {
    expect(effectiveClient(clients, undefined, undefined)).toBeUndefined()
  })
})

describe('辅助判定', () => {
  it('来源文案 key 按来源区分', () => {
    expect(sourceLabelKey(makeClient({ source: 'download' }))).toBe('frp.clientSourceDownload')
    expect(sourceLabelKey(makeClient({ source: 'external' }))).toBe('frp.clientSourceExternal')
  })

  it('只有文件存在的客户端才算可用', () => {
    expect(hasUsableClient([makeClient({ exists: false })])).toBe(false)
    expect(hasUsableClient([makeClient({ exists: false }), makeClient({})])).toBe(true)
  })
})
