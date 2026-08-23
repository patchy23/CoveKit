import { describe, expect, it } from 'vitest'
import enUS from './locales/en-US'
import zhCN from './locales/zh-CN'
import { i18n, setLocale } from '.'

function keys(value: object, prefix = ''): string[] {
  return Object.entries(value).flatMap(([key, child]) => {
    const path = prefix ? `${prefix}.${key}` : key
    return typeof child === 'object' ? keys(child, path) : path
  })
}

describe('i18n', () => {
  it('中英语言包键保持一致', () => {
    expect(keys(enUS).sort()).toEqual(keys(zhCN).sort())
  })

  it('切换语言时同步文档语言', () => {
    setLocale('en-US')
    expect(i18n.global.locale.value).toBe('en-US')
    expect(document.documentElement.lang).toBe('en-US')
    setLocale('zh-CN')
  })
})
