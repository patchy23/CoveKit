import { describe, expect, it } from 'vitest'
import { chmodMenuVisible, chmodNeedsRiskAck, deleteMenuVisible } from './sshPolicy'

describe('sshPolicy 删除菜单可见性', () => {
  it('系统目录本体与子树隐藏', () => {
    expect(deleteMenuVisible('/')).toBe(false)
    expect(deleteMenuVisible('/etc')).toBe(false)
    expect(deleteMenuVisible('/etc/nginx/nginx.conf')).toBe(false)
    expect(deleteMenuVisible('/sbin/sshd')).toBe(false)
  })

  it('自定义路径放行（含根下自建目录）', () => {
    expect(deleteMenuVisible('/mydata')).toBe(true)
    expect(deleteMenuVisible('/home/user/tmp')).toBe(true)
    expect(deleteMenuVisible('/data/logs')).toBe(true)
  })
})

describe('sshPolicy chmod 菜单可见性', () => {
  it('系统目录本体隐藏、内部文件放行', () => {
    expect(chmodMenuVisible('/etc')).toBe(false)
    expect(chmodMenuVisible('/usr')).toBe(false)
    expect(chmodMenuVisible('/sbin/sshd')).toBe(true)
    expect(chmodMenuVisible('/etc/nginx/nginx.conf')).toBe(true)
  })

  it('虚拟文件系统整树隐藏', () => {
    expect(chmodMenuVisible('/proc/1/status')).toBe(false)
    expect(chmodMenuVisible('/sys/kernel')).toBe(false)
    expect(chmodMenuVisible('/dev/null')).toBe(false)
  })

  it('系统目录内递归需风险确认', () => {
    expect(chmodNeedsRiskAck('/var/www', true)).toBe(true)
    expect(chmodNeedsRiskAck('/var/www', false)).toBe(false)
    expect(chmodNeedsRiskAck('/home/app', true)).toBe(false)
  })
})
