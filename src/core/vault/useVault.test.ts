/**
 * useVault 纯函数单测（筛选/搜索、表单建模、校验、payload 构建、主秘密提取）
 */
import { describe, expect, it } from 'vitest'
import type { Credential, CredentialSummary } from '@/core/ipc/contracts'
import {
  CREDENTIAL_KINDS,
  emptyFormState,
  fieldsFromFormState,
  filterCredentials,
  formFieldsFor,
  formStateFromCredential,
  formatTimestamp,
  KIND_LABEL,
  payloadFromFormState,
  primarySecret,
  validateFormState,
} from './useVault'

/** 测试用摘要工厂 */
function summary(partial: Partial<CredentialSummary>): CredentialSummary {
  return {
    id: 'id',
    name: '凭证',
    kind: 'password',
    masked: 'root',
    note: '',
    createdAt: 0,
    updatedAt: 0,
    ...partial,
  }
}

describe('类型元数据', () => {
  it('5 种类型全覆盖且有中文名', () => {
    expect(CREDENTIAL_KINDS).toHaveLength(5)
    for (const k of CREDENTIAL_KINDS) expect(KIND_LABEL[k]).toBeTruthy()
  })
})

describe('筛选与搜索', () => {
  const list = [
    summary({ id: '1', name: '生产 MySQL', kind: 'password', masked: 'root', note: '主库' }),
    summary({ id: '2', name: '腾讯云 CAM', kind: 'access-key-pair', masked: 'AKI****xyz' }),
    summary({ id: '3', name: '跳板机', kind: 'ssh-key', masked: 'ubuntu', note: '生产' }),
  ]

  it('全部类型 + 空搜索 = 原列表', () => {
    expect(filterCredentials(list, 'all', '')).toHaveLength(3)
  })

  it('按类型过滤', () => {
    expect(filterCredentials(list, 'ssh-key', '').map((c) => c.id)).toEqual(['3'])
  })

  it('搜索匹配名称/备注/掩码（大小写不敏感）', () => {
    expect(filterCredentials(list, 'all', 'mysql').map((c) => c.id)).toEqual(['1'])
    expect(filterCredentials(list, 'all', 'AKI').map((c) => c.id)).toEqual(['2'])
    expect(filterCredentials(list, 'all', '生产').map((c) => c.id)).toEqual(['1', '3'])
  })

  it('类型与搜索联合过滤', () => {
    expect(filterCredentials(list, 'password', '生产').map((c) => c.id)).toEqual(['1'])
    expect(filterCredentials(list, 'api-token', '生产')).toHaveLength(0)
  })
})

describe('表单建模', () => {
  it('字段表按类型分派', () => {
    expect(formFieldsFor('password').map((f) => f.key)).toEqual(['username', 'password'])
    expect(formFieldsFor('ssh-key').map((f) => f.key)).toEqual([
      'username',
      'privateKey',
      'passphrase',
    ])
    expect(formFieldsFor('api-token').map((f) => f.key)).toEqual(['token'])
    expect(formFieldsFor('custom')).toEqual([])
    // 秘密字段标记
    expect(formFieldsFor('password')[1].secret).toBe(true)
    expect(formFieldsFor('ssh-key')[1].multiline).toBe(true)
    expect(formFieldsFor('ssh-key')[2].optional).toBe(true)
  })

  it('空表单带一行自定义条目', () => {
    const s = emptyFormState('custom')
    expect(s.entries).toEqual([{ key: '', value: '', secret: true }])
  })

  it('从凭证明文还原表单（编辑场景）', () => {
    const credential: Credential = {
      id: 'c1',
      name: '跳板机',
      kind: 'ssh-key',
      fields: { type: 'ssh-key', username: 'ubuntu', privateKey: 'KEY', passphrase: null },
      note: '生产',
      createdAt: 1,
      updatedAt: 2,
    }
    const s = formStateFromCredential(credential)
    expect(s.name).toBe('跳板机')
    expect(s.values.username).toBe('ubuntu')
    expect(s.values.privateKey).toBe('KEY')
    // passphrase null 不写入 values（保持空串）
    expect(s.values.passphrase).toBe('')
  })

  it('校验：名称空 / 必填缺 / custom 空键名', () => {
    expect(validateFormState(emptyFormState())).toBe('凭证名称不能为空')

    const s = emptyFormState()
    s.name = 'x'
    expect(validateFormState(s)).toBe('用户名不能为空')
    s.values.username = 'root'
    expect(validateFormState(s)).toBe('密码不能为空')
    s.values.password = 'p'
    expect(validateFormState(s)).toBeNull()

    const c = emptyFormState('custom')
    c.name = 'x'
    expect(validateFormState(c)).toBe('自定义凭证至少需要一个字段')
    c.entries = [{ key: ' ', value: 'v', secret: false }]
    expect(validateFormState(c)).toBe('自定义字段的键名不能为空')
    c.entries = [{ key: 'k', value: 'v', secret: false }]
    expect(validateFormState(c)).toBeNull()
  })

  it('payload 构建：trim、可选字段空串转 null、custom 过滤空行', () => {
    const s = emptyFormState('ssh-key')
    s.name = ' 跳板机 '
    s.values.username = ' ubuntu '
    s.values.privateKey = 'KEY'
    s.values.passphrase = ''
    const p = payloadFromFormState(s)
    expect(p.id).toBeNull()
    expect(p.name).toBe('跳板机')
    expect(p.fields).toEqual({
      type: 'ssh-key',
      username: 'ubuntu',
      privateKey: 'KEY',
      passphrase: null,
    })

    const c = emptyFormState('custom')
    c.name = 'x'
    c.entries = [
      { key: ' a ', value: '1', secret: false },
      { key: '', value: '', secret: true },
    ]
    expect(fieldsFromFormState(c)).toEqual({
      type: 'custom',
      entries: [{ key: 'a', value: '1', secret: false }],
    })
  })

  it('fields 的 type 标签与 kind 一致', () => {
    for (const kind of CREDENTIAL_KINDS) {
      const s = emptyFormState(kind)
      expect(fieldsFromFormState(s).type).toBe(kind)
    }
  })
})

describe('主秘密提取（复制值）', () => {
  it('各类型取对应秘密字段', () => {
    const base = { id: 'x', name: 'x', note: '', createdAt: 0, updatedAt: 0 }
    expect(
      primarySecret({
        ...base,
        kind: 'password',
        fields: { type: 'password', username: 'u', password: 'pw' },
      })
    ).toBe('pw')
    expect(
      primarySecret({ ...base, kind: 'api-token', fields: { type: 'api-token', token: 'tok' } })
    ).toBe('tok')
    expect(
      primarySecret({
        ...base,
        kind: 'access-key-pair',
        fields: { type: 'access-key-pair', accessKeyId: 'AKID', accessKeySecret: 'SEC' },
      })
    ).toBe('SEC')
    // custom 优先秘密条目
    expect(
      primarySecret({
        ...base,
        kind: 'custom',
        fields: {
          type: 'custom',
          entries: [
            { key: 'host', value: 'h', secret: false },
            { key: 'key', value: 'k', secret: true },
          ],
        },
      })
    ).toBe('k')
  })
})

describe('时间格式化', () => {
  it('秒级时间戳 → YYYY-MM-DD HH:mm', () => {
    expect(formatTimestamp(0)).toMatch(/^\d{4}-\d{2}-\d{2} \d{2}:\d{2}$/)
    expect(formatTimestamp(1780000000)).toMatch(/^\d{4}-\d{2}-\d{2} \d{2}:\d{2}$/)
  })
})
