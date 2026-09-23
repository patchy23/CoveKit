import { describe, expect, it, vi, beforeEach } from 'vitest'
import { csvText, parseTable, mapHeaders, previewRows } from './serverCsv'
import { importServers } from './importServers'
import type { ServerProfile } from '../contracts'
const env = vi.hoisted(() => ({ save: vi.fn(), group: vi.fn(), vault: vi.fn(), remove: vi.fn() }))
vi.mock('../ipc', () => ({ ipc: { sshProfileSave: env.save, sshGroupSave: env.group } }))
vi.mock('@/core/ipc/ipc', () => ({ ipc: { vaultSave: env.vault, vaultDelete: env.remove } }))
beforeEach(() => {
  vi.resetAllMocks()
  env.save.mockResolvedValue(undefined)
  env.vault.mockResolvedValue({ id: 'new-secret' })
  env.remove.mockResolvedValue({ ok: true })
})
const profile: ServerProfile = {
  id: 'p1',
  name: 'old',
  host: 'host',
  username: 'root',
  port: 22,
  authMethod: 'password',
  hasLocalAuth: true,
}
function rows(password = '') {
  const t = parseTable(
    csvText([
      ['host', 'username', 'password'],
      ['host', 'root', password],
    ])
  )
  return previewRows(t, mapHeaders(t[0]), [profile])
}
const opts = () => ({
  passwordMode: 'local' as const,
  duplicate: 'update' as const,
  cancelled: () => false,
  onResult: vi.fn(),
})
describe('CSV 导入边界', () => {
  it('保留逗号、换行、引号和密码两端空白', () => {
    const values = [
      ['host', 'username', 'password'],
      ['host', 'root', ' a,"b"\n '],
    ]
    expect(parseTable(csvText(values))).toEqual(values)
    expect(previewRows(values, mapHeaders(values[0]), [])[0].password).toBe(' a,"b"\n ')
  })
  it('接受 Excel 制表符与中文表头，拒绝无效端口和重复行', () => {
    const t = parseTable('主机\t用户名\t端口\nhost\troot\t70000\nother\troot\t22\nother\troot\t22')
    const result = previewRows(t, mapHeaders(t[0]), [])
    expect(result[0].error).toContain('端口')
    expect(result[2].error).toContain('重复')
    expect(() => parseTable('host,user\n"x,y')).toThrow('未闭合')
  })
  it('密钥记录不导入，默认名称和端口只在空值时补全', () => {
    const t = parseTable('host,username,auth_type\nhost,root,privateKey')
    expect(previewRows(t, mapHeaders(t[0]), [])[0]).toMatchObject({
      name: 'root@host',
      port: 22,
      error: expect.stringContaining('仅支持'),
    })
  })
  it('更新时空密码保留原本地认证', async () => {
    await importServers(rows(), [profile], [], opts())
    expect(env.save).toHaveBeenCalledWith(
      expect.objectContaining({
        saveLocal: true,
        password: undefined,
        profile: expect.objectContaining({ id: 'p1' }),
      })
    )
  })
  it('本地保存传入原密码，凭证库失败不回退明文', async () => {
    await importServers(rows(' password '), [profile], [], opts())
    expect(env.save.mock.calls[0][0].password).toBe(' password ')
    env.save.mockClear()
    env.vault.mockRejectedValue(new Error('secret must not leak'))
    const options = { ...opts(), passwordMode: 'vault' as const }
    await importServers(rows('password'), [profile], [], options)
    expect(env.save).not.toHaveBeenCalled()
    expect(JSON.stringify(options.onResult.mock.calls)).not.toContain('secret must not leak')
  })
  it('配置保存失败清理新建凭证，不更改原共享凭证', async () => {
    env.save.mockRejectedValue(new Error('disk full'))
    const options = { ...opts(), passwordMode: 'vault' as const }
    await importServers(rows('password'), [{ ...profile, credentialRef: 'shared' }], [], options)
    expect(env.vault.mock.calls[0][0].id).toBeNull()
    expect(env.remove).toHaveBeenCalledWith('new-secret')
    expect(options.onResult.mock.calls[0][0].status).toBe('失败')
  })
  it('多个匹配禁止自动覆盖，取消后不再创建记录', async () => {
    const list = rows('password')
    list[0].matches = ['p1', 'p2']
    await importServers(list, [profile], [], opts())
    expect(env.save).not.toHaveBeenCalled()
    await importServers(rows('password'), [profile], [], { ...opts(), cancelled: () => true })
    expect(env.save).not.toHaveBeenCalled()
  })
})
