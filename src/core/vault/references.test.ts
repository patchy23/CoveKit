import { describe, expect, it } from 'vitest'
import { countCredentialReferences, normalizeCredentialReferences } from './references'

describe('Vault 浏览器侧引用登记', () => {
  it('去除空值并保留多个 profile 的重复引用', () => {
    expect(normalizeCredentialReferences([' a ', '', 'a', 'b'])).toEqual(['a', 'a', 'b'])
  })

  it('跨 scope 汇总同一凭证引用数', () => {
    expect(
      countCredentialReferences(
        {
          ssh: ['credential-1', 'credential-1', 'credential-2'],
          another: ['credential-1'],
        },
        'credential-1'
      )
    ).toBe(3)
  })
})
