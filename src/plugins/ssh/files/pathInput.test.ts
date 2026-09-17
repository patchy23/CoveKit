import { describe, expect, it } from 'vitest'
import { normalizePathInput } from './pathInput'

describe('文件路径输入', () => {
  it('远程绝对路径与根目录保持正斜杠', () => {
    expect(normalizePathInput(' /etc/systemd/system ', '/')).toBe('/etc/systemd/system')
    expect(normalizePathInput('/', '/')).toBe('/')
    expect(normalizePathInput('var/log', '/')).toBe('/var/log')
    expect(normalizePathInput('/tmp/file\\name', '/')).toBe('/tmp/file\\name')
  })
  it('仅本地 Windows 面板转换分隔符', () => {
    expect(normalizePathInput('C:/Users/example', '\\')).toBe('C:\\Users\\example')
    expect(normalizePathInput('\\\\server\\share', '\\')).toBe('\\\\server\\share')
  })
})
