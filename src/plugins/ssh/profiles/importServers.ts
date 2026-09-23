/** 顺序执行导入，单条失败可报告；不自动连接、不把凭证失败降级为明文。 */
import { ipc } from '../ipc'
import { ipc as coreIpc } from '@/core/ipc/ipc'
import type { ServerProfile, SshGroup } from '../contracts'
import type { ImportRow } from './serverCsv'
export interface ImportResult {
  row: number
  status: '成功' | '跳过' | '失败'
  message: string
}
export type PasswordMode = 'local' | 'vault' | 'none'
export async function importServers(
  rows: ImportRow[],
  profiles: ServerProfile[],
  groups: SshGroup[],
  options: {
    passwordMode: PasswordMode
    duplicate: 'skip' | 'update' | 'new'
    groupId?: string
    cancelled: () => boolean
    onResult: (r: ImportResult) => void
  }
) {
  const knownGroups = [...groups]
  for (const row of rows) {
    if (options.cancelled()) break
    const report = (status: ImportResult['status'], message: string) =>
      options.onResult({ row: row.row, status, message })
    if (row.error) {
      report('跳过', row.error)
      continue
    }
    if (row.matches.length && options.duplicate === 'skip') {
      report('跳过', '已存在相同连接')
      continue
    }
    if (row.matches.length > 1 && options.duplicate === 'update') {
      report('失败', '匹配到多个服务器，请选择新增或先处理重复记录')
      continue
    }
    const existing =
      options.duplicate === 'update' ? profiles.find((p) => p.id === row.matches[0]) : undefined
    let createdCredential: string | undefined
    try {
      let groupId = options.groupId ?? existing?.groupId ?? undefined
      if (!options.groupId && row.group) {
        const matches = knownGroups.filter((g) => g.name === row.group)
        if (matches.length > 1) {
          report('失败', '同名分组不唯一，请选择目标分组')
          continue
        }
        if (matches.length) groupId = matches[0].id
        else {
          const group = { id: crypto.randomUUID(), name: row.group, sortOrder: knownGroups.length }
          await ipc.sshGroupSave(group)
          knownGroups.push(group)
          groupId = group.id
        }
      }
      const preserve = !!existing && row.password === ''
      const profile: ServerProfile = {
        ...existing,
        id: existing?.id ?? crypto.randomUUID(),
        name: row.name,
        host: row.host,
        port: row.port,
        username: row.username,
        authMethod: preserve ? existing.authMethod : 'password',
        credentialRef: preserve ? existing.credentialRef : undefined,
        hasLocalAuth: preserve ? existing.hasLocalAuth : false,
        groupId,
        remark: row.remark,
      }
      let saveLocal = preserve ? !!existing.hasLocalAuth : false
      if (!preserve && row.password && options.passwordMode === 'vault') {
        const saved = await coreIpc.vaultSave({
          id: null,
          name: row.name,
          kind: 'password',
          fields: { type: 'password', username: row.username, password: row.password },
          note: 'SSH 批量导入',
        })
        createdCredential = saved.id
        profile.credentialRef = saved.id
      } else if (!preserve && row.password && options.passwordMode === 'local') saveLocal = true
      await ipc.sshProfileSave({
        profile,
        saveCredential: false,
        saveLocal,
        password: saveLocal && !preserve ? row.password : undefined,
      })
      createdCredential = undefined
      report('成功', existing ? '已更新' : '已新增')
    } catch {
      let message = '保存失败，请检查凭证库是否可用或本地存储权限'
      if (createdCredential) {
        try {
          const result = await coreIpc.vaultDelete(createdCredential)
          if (!result.ok) message += '；临时凭证清理失败，请在凭证管理中检查本次新增条目'
        } catch {
          message += '；临时凭证清理失败，请在凭证管理中检查本次新增条目'
        }
      }
      report('失败', message)
    }
  }
}
