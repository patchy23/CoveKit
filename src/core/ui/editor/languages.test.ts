/**
 * detectLanguage 单测：纯函数覆盖常见扩展名、特例文件名、路径与未知回退。
 */
import { describe, expect, it } from 'vitest'
import { detectLanguage } from './languages'

describe('detectLanguage', () => {
  it('按扩展名识别常见语言', () => {
    expect(detectLanguage('data.json').id).toBe('json')
    expect(detectLanguage('config.yaml').id).toBe('yaml')
    expect(detectLanguage('config.yml').id).toBe('yaml')
    expect(detectLanguage('query.sql').id).toBe('sql')
    expect(detectLanguage('main.py').id).toBe('python')
    expect(detectLanguage('index.html').id).toBe('html')
    expect(detectLanguage('style.css').id).toBe('css')
    expect(detectLanguage('deploy.sh').id).toBe('shell')
    expect(detectLanguage('note.md').id).toBe('markdown')
    expect(detectLanguage('app.ts').id).toBe('typescript')
    expect(detectLanguage('app.js').id).toBe('javascript')
  })

  it('大小写不敏感，且支持带目录的完整路径', () => {
    expect(detectLanguage('CONFIG.JSON').id).toBe('json')
    expect(detectLanguage('/etc/nginx/nginx.conf').id).toBe('nginx')
    expect(detectLanguage('C:\\Users\\patchy\\.ssh\\authorized_keys').id).toBe('plaintext')
    expect(detectLanguage('src/core/ui/editor/theme.ts').id).toBe('typescript')
  })

  it('识别特例文件名（含无扩展名与点开头文件）', () => {
    expect(detectLanguage('Dockerfile').id).toBe('dockerfile')
    expect(detectLanguage('Makefile').id).toBe('makefile')
    expect(detectLanguage('.env').id).toBe('ini')
    expect(detectLanguage('.env.local').id).toBe('ini')
    expect(detectLanguage('/etc/hosts').id).toBe('plaintext')
  })

  it('未知或无文件名时回退纯文本', () => {
    expect(detectLanguage().id).toBe('plaintext')
    expect(detectLanguage('').id).toBe('plaintext')
    expect(detectLanguage('   ').id).toBe('plaintext')
    expect(detectLanguage('unknown.zzz').id).toBe('plaintext')
    expect(detectLanguage('noextension').id).toBe('plaintext')
    expect(detectLanguage('.gitignore').id).toBe('plaintext')
    expect(detectLanguage('trailing.').id).toBe('plaintext')
  })

  it('标签始终为非空中文或语言名（供状态栏展示）', () => {
    for (const name of ['a.json', 'a.sql', 'a.unknown', 'Dockerfile', '']) {
      expect(detectLanguage(name).label.length).toBeGreaterThan(0)
    }
    expect(detectLanguage('a.json').label).toBe('JSON')
    expect(detectLanguage('a.unknown').label).toBe('纯文本')
  })

  it('显式语言优先于文件名识别', () => {
    expect(detectLanguage('a.txt', 'json')).toEqual({ id: 'json', label: 'JSON' })
    expect(detectLanguage('', 'sql')).toEqual({ id: 'sql', label: 'SQL' })
  })

  it("language 为 'auto' 或空串时按文件名识别", () => {
    expect(detectLanguage('a.json', 'auto').id).toBe('json')
    expect(detectLanguage('a.json', '').id).toBe('json')
  })
})
