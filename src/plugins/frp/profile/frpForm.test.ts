/**
 * frpForm 单测：parsed ⇄ 表单往返不丢未知字段、缺失字段默认值、代理增删行、空 parsed 不崩、基本校验
 */
import { describe, expect, it } from 'vitest'
import { tokenCredentialId, tokenReference } from './frpCredential'
import {
  emptyFormModel,
  emptyProxy,
  mergeFormModel,
  toFormModel,
  validateFormModel,
} from './frpForm'

/** 断言工具：取回写结果里的代理数组（避免测试里堆 as any） */
function proxiesOf(value: Record<string, unknown>): Record<string, unknown>[] {
  const list = value.proxies
  return Array.isArray(list) ? (list as Record<string, unknown>[]) : []
}

describe('代理类型切换', () => {
  it.each(['http', 'https', 'stcp'])('TCP 切到 %s 时删除旧远端端口并保留公共配置', (type) => {
    const parsed = {
      serverAddr: 'example.test',
      serverPort: 7000,
      proxies: [
        {
          name: 'test',
          type: 'tcp',
          localPort: 8033,
          remotePort: 35230,
          transport: { useCompression: true },
        },
      ],
    }
    const model = toFormModel(parsed)
    model.proxies[0].type = type
    model.proxies[0].remotePort = -1
    expect(validateFormModel(model)).toBe('')
    const proxy = proxiesOf(mergeFormModel(parsed, model))[0]
    expect(proxy).not.toHaveProperty('remotePort')
    expect(proxy.localPort).toBe(8033)
    expect(proxy.transport).toEqual({ useCompression: true })
  })

  it.each(['tcp', 'udp', 'http', 'https', 'stcp'])('%s 只写入适用的表单专属字段', (type) => {
    const parsed = {
      proxies: [
        {
          name: 'test',
          type: 'http',
          remotePort: 35230,
          customDomains: ['example.test'],
          secretKey: 'test-key',
        },
      ],
    }
    const model = toFormModel(parsed)
    model.proxies[0].type = type
    const proxy = proxiesOf(mergeFormModel(parsed, model))[0]
    expect(proxy.remotePort).toBe(['tcp', 'udp'].includes(type) ? 35230 : undefined)
    expect(proxy.customDomains).toEqual(
      ['http', 'https'].includes(type) ? ['example.test'] : undefined
    )
    expect(proxy.secretKey).toBe(type === 'stcp' ? 'test-key' : undefined)
  })
})

describe('frpForm · 读取', () => {
  it('缺失字段填默认值，已知字段原样读出', () => {
    const model = toFormModel({
      serverAddr: 'frps.example.com',
      serverPort: 7001,
      auth: { method: 'token', token: 'secret' },
      transport: { protocol: 'kcp', poolCount: 4, tls: { enable: true, serverName: 'x' } },
      log: { level: 'debug' },
      proxies: [
        { name: 'ssh', type: 'tcp', localIP: '127.0.0.1', localPort: 22, remotePort: 6000 },
      ],
    })
    expect(model.serverAddr).toBe('frps.example.com')
    expect(model.serverPort).toBe(7001)
    expect(model.protocol).toBe('kcp')
    expect(model.poolCount).toBe(4)
    expect(model.tlsEnable).toBe(true)
    expect(model.logLevel).toBe('debug')
    expect(model.proxies).toHaveLength(1)
    expect(model.proxies[0].remotePort).toBe(6000)
  })

  it('空对象与异常结构不崩，一律给默认值', () => {
    const model = toFormModel({})
    expect(model).toEqual(emptyFormModel())
    const weird = toFormModel({
      auth: 'not-an-object',
      transport: ['nope'],
      proxies: 'nope',
      serverPort: 'not-a-number',
    })
    expect(weird.authMethod).toBe('token')
    expect(weird.protocol).toBe('tcp')
    expect(weird.proxies).toEqual([])
    // 解析不出端口时回落到默认 7000，而不是 0
    expect(weird.serverPort).toBe(7000)
  })

  it('customDomains 数组 → 逗号分隔文本；enabled 缺省为 true', () => {
    const model = toFormModel({
      proxies: [
        { name: 'web', type: 'http', customDomains: ['a.example.com', 'b.example.com'] },
        { name: 'off', type: 'tcp', enabled: false },
      ],
    })
    expect(model.proxies[0].customDomains).toBe('a.example.com, b.example.com')
    expect(model.proxies[0].enabled).toBe(true)
    expect(model.proxies[1].enabled).toBe(false)
  })
})

describe('frpForm · 写回', () => {
  it('凭证往返只保存引用，选择凭证会替换手工 Token 与互斥的 tokenSource', () => {
    const parsed = {
      serverAddr: 'example.test',
      auth: { token: 'old', tokenSource: { type: 'file' } },
    }
    const model = toFormModel(parsed)
    model.authCredentialId = '凭证-id'
    const merged = mergeFormModel(parsed, model)
    expect(merged.auth).toEqual({ method: 'token', token: tokenReference('凭证-id') })
    expect(tokenReference('凭证-id')).toBe('{{ .Envs.COVEKIT_FRP_TOKEN_e587ade8af812d6964 }}')
    expect(tokenCredentialId(tokenReference('凭证-id'))).toBe('凭证-id')
    const restored = toFormModel(merged)
    expect(restored.authCredentialId).toBe('凭证-id')
    expect(restored.authToken).toBe('')
    restored.authCredentialId = ''
    restored.authToken = 'manual'
    expect(mergeFormModel(merged, restored).auth).toEqual({ method: 'token', token: 'manual' })
  })

  it('表单改地址时保留 TLS 证书和未修改的外部 Token 来源', () => {
    const parsed = {
      serverAddr: 'old',
      auth: { tokenSource: { type: 'file', file: { path: 'token.txt' } } },
      transport: {
        tls: { enable: true, certFile: 'client.pem', keyFile: 'key.pem', trustedCaFile: 'ca.pem' },
      },
    }
    const model = toFormModel(parsed)
    model.serverAddr = 'new'
    const merged = mergeFormModel(parsed, model)
    expect((merged.auth as Record<string, unknown>).tokenSource).toEqual(parsed.auth.tokenSource)
    expect((merged.transport as Record<string, unknown>).tls).toEqual(parsed.transport.tls)
  })
  it('未知顶层段落与代理内未知字段原样保留', () => {
    const parsed: Record<string, unknown> = {
      serverAddr: 'a.example.com',
      serverPort: 7000,
      customX: { keep: 'me' },
      includes: ['./confd'],
      proxies: [
        {
          name: 'p1',
          type: 'tcp',
          localPort: 8080,
          remotePort: 6080,
          healthCheck: { type: 'tcp' },
        },
      ],
    }
    const model = toFormModel(parsed)
    model.serverAddr = 'b.example.com'
    model.proxies[0].remotePort = 7080
    const next = mergeFormModel(parsed, model)

    expect(next.customX).toEqual({ keep: 'me' })
    expect(next.includes).toEqual(['./confd'])
    expect(next.serverAddr).toBe('b.example.com')
    const proxies = proxiesOf(next)
    expect(proxies[0].healthCheck).toEqual({ type: 'tcp' })
    expect(proxies[0].remotePort).toBe(7080)
  })

  it('清空的字段被删除而不是写成空串（TOML 无 null，空串会污染配置）', () => {
    const parsed: Record<string, unknown> = {
      serverAddr: 'a.example.com',
      user: 'someone',
      auth: { method: 'token', token: 't' },
      proxies: [{ name: 'p1', type: 'tcp', remotePort: 6000, localIP: '10.0.0.1' }],
    }
    const model = toFormModel(parsed)
    model.user = ''
    model.authToken = ''
    model.proxies[0].localIP = ''
    const next = mergeFormModel(parsed, model)

    expect('user' in next).toBe(false)
    expect(next.auth).toEqual({ method: 'token' })
    expect('localIP' in proxiesOf(next)[0]).toBe(false)
    expect(proxiesOf(next)[0].remotePort).toBe(6000)
  })

  it('代理增删行：新增行写入、清空到零行则移除 proxies', () => {
    const parsed: Record<string, unknown> = { serverAddr: 'a.example.com', proxies: [] }
    const model = toFormModel(parsed)
    model.proxies.push({ ...emptyProxy(), name: 'new', type: 'http', localPort: 3000 })
    const withNew = mergeFormModel(parsed, model)
    expect(proxiesOf(withNew)).toHaveLength(1)
    expect(proxiesOf(withNew)[0].name).toBe('new')

    model.proxies = []
    const cleared = mergeFormModel(parsed, model)
    expect('proxies' in cleared).toBe(false)
  })

  it('customDomains 文本 → 数组（空白与空项被丢弃）', () => {
    const parsed: Record<string, unknown> = { proxies: [{ name: 'web', type: 'http' }] }
    const model = toFormModel(parsed)
    model.proxies[0].customDomains = ' a.example.com , ,b.example.com , '
    const next = mergeFormModel(parsed, model)
    expect(proxiesOf(next)[0].customDomains).toEqual(['a.example.com', 'b.example.com'])
  })

  it('enabled 默认不写入，仅在显式关闭时落 false', () => {
    const parsed: Record<string, unknown> = { proxies: [{ name: 'p', type: 'tcp' }] }
    const model = toFormModel(parsed)
    expect('enabled' in proxiesOf(mergeFormModel(parsed, model))[0]).toBe(false)
    model.proxies[0].enabled = false
    expect(proxiesOf(mergeFormModel(parsed, model))[0].enabled).toBe(false)
  })

  it('往返幂等：读出来再写回不改变配置语义', () => {
    const parsed: Record<string, unknown> = {
      serverAddr: 'a.example.com',
      serverPort: 7000,
      auth: { method: 'token', token: 't' },
      transport: { tls: { enable: true } },
      proxies: [{ name: 'p', type: 'tcp', localIP: '127.0.0.1', localPort: 22, remotePort: 6000 }],
    }
    const once = mergeFormModel(parsed, toFormModel(parsed))
    const twice = mergeFormModel(once, toFormModel(once))
    expect(twice).toEqual(once)
  })
})

describe('frpForm · 校验', () => {
  it.each([NaN, Infinity, 1.5, 0, -1, 65536])('拒绝无效端口 %s', (port) => {
    const model = { ...emptyFormModel(), serverAddr: 'example.test', serverPort: port }
    expect(validateFormModel(model)).toBe('frp.formServerPortInvalid')
    model.serverPort = 7000
    model.proxies = [{ ...emptyProxy(), name: 'p', localPort: 22, remotePort: port }]
    expect(validateFormModel(model)).toBe('frp.formProxyPortInvalid')
    model.proxies[0].remotePort = 6000
    model.proxies[0].localPort = port
    expect(validateFormModel(model)).toBe('frp.formProxyPortInvalid')
  })
  it('缺服务器地址 / 端口越界 / 代理名缺失 / tcp 缺远端端口均可被拦下', () => {
    const model = emptyFormModel()
    model.serverAddr = ''
    expect(validateFormModel(model)).toBe('frp.formServerRequired')

    model.serverAddr = 'a.example.com'
    model.serverPort = 70000
    expect(validateFormModel(model)).toBe('frp.formServerPortInvalid')

    model.serverPort = 7000
    model.proxies = [{ ...emptyProxy(), name: '' }]
    expect(validateFormModel(model)).toBe('frp.formProxyNameRequired')

    model.proxies = [{ ...emptyProxy(), name: 'p', type: 'tcp', remotePort: null }]
    expect(validateFormModel(model)).toBe('frp.formProxyRemotePortRequired')

    // http 类型不需要远端端口
    model.proxies = [{ ...emptyProxy(), name: 'p', type: 'http', remotePort: null }]
    expect(validateFormModel(model)).toBe('')
  })
})
