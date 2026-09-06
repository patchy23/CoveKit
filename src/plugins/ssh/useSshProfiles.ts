/**
 * useSshProfiles · 服务器配置与分组状态（自 useSshWorkspace 拆出，300 行红线）
 * 数据持久化在后端插件库（ssh.db）；localStorage 存量一次性迁移。
 */
import { ref } from 'vue'
import { useUiStore } from '@/stores/ui'
import { ipc } from './ipc'
import { clearLegacySnapshot, readLegacySnapshot } from './useSsh'
import { useServerGroups } from './useServerGroups'
import type { ServerProfile } from './contracts'

export function useSshProfiles() {
  const ui = useUiStore()
  const profiles = ref<ServerProfile[]>([])
  const groupsApi = useServerGroups()
  const { groups, expandedIds } = groupsApi

  async function loadProfiles() {
    try {
      profiles.value = await ipc.sshProfileList()
    } catch {
      /* 浏览器预览没有 IPC */
    }
  }

  /** localStorage 存量一次性迁入后端插件库（旧手工凭证由后端迁移进 Vault 并归档） */
  async function importLegacyOnce() {
    try {
      const existing = await ipc.sshProfileList()
      if (existing.length > 0) {
        // 已迁移过（或用户手动重建过配置）：清掉 localStorage 快照，静默返回
        clearLegacySnapshot()
        return
      }
      const legacy = readLegacySnapshot()
      if (legacy.profiles.length === 0 && legacy.groups.length === 0) return
      const result = await ipc.sshProfileImport(legacy)
      await loadProfiles()
      clearLegacySnapshot()
      ui.toast(
        `已迁移 ${result.importedProfiles} 台服务器` +
          (result.migratedCredentials > 0
            ? `，${result.migratedCredentials} 个凭证已入凭证库`
            : '')
      )
    } catch {
      /* 迁移失败保留 localStorage 快照，下次打开工具重试 */
    }
  }

  async function createGroup(name: string) {
    const group = await groupsApi.createGroup(name)
    ui.toast(`已创建分组「${group.name}」`)
  }

  async function renameGroup(groupId: string, name: string) {
    await groupsApi.renameGroup(groupId, name)
    ui.toast('已重命名分组')
  }

  /** 删除分组：组内连接移回未分组（连接配置本身不删） */
  async function deleteGroup(groupId: string) {
    const group = groups.value.find((g) => g.id === groupId)
    await groupsApi.deleteGroup(groupId)
    for (const p of profiles.value) {
      if (p.groupId === groupId) p.groupId = undefined
    }
    ui.toast(`已删除分组「${group?.name ?? ''}」，组内连接移回未分组`)
  }

  /** 拖拽入组：null = 未分组（组变更走 ssh_profile_save 持久化） */
  async function moveToGroup(profileId: string, groupId: string | null) {
    const profile = profiles.value.find((p) => p.id === profileId)
    if (!profile) return
    const targetName = groupId ? (groups.value.find((g) => g.id === groupId)?.name ?? '') : '未分组'
    if ((profile.groupId ?? null) === groupId) return
    const previous = profile.groupId
    profile.groupId = groupId ?? undefined
    try {
      await ipc.sshProfileSave({ profile, saveCredential: false })
      ui.toast(`已将「${profile.name}」移动到「${targetName}」`)
    } catch (error) {
      profile.groupId = previous
      ui.toast(`移动分组失败：${error}`)
    }
  }

  /** 新增/更新服务器：配置写插件库；勾选保存凭证时后端写入 Vault 并回填 credentialRef */
  async function saveProfile(
    profile: ServerProfile,
    credentials: { password?: string; privateKey?: string; passphrase?: string },
    saveCredential: boolean
  ) {
    const index = profiles.value.findIndex((item) => item.id === profile.id)
    const authChanged = index < 0 || profiles.value[index].authMethod !== profile.authMethod
    const switchedFromVault =
      index >= 0 && Boolean(profiles.value[index].credentialRef) && !profile.credentialRef
    const manualCredentialReady =
      profile.authMethod === 'password'
        ? Boolean(credentials.password)
        : Boolean(credentials.privateKey) &&
          (profile.authMethod !== 'privateKeyWithPassphrase' || Boolean(credentials.passphrase))
    const credentialReady = Boolean(profile.credentialRef) || manualCredentialReady
    if ((index < 0 || authChanged || switchedFromVault) && !credentialReady) {
      ui.toast('新增服务器或切换认证方式时必须填写完整凭证')
      return
    }
    try {
      const saved = await ipc.sshProfileSave({
        profile,
        ...credentials,
        saveCredential: saveCredential && manualCredentialReady,
      })
      const existing = profiles.value.findIndex((item) => item.id === saved.id)
      if (existing >= 0) profiles.value[existing] = saved
      else profiles.value.push(saved)
      ui.toast(
        `${existing >= 0 ? '已更新' : '已添加'}服务器「${saved.name}」` +
          (saveCredential && manualCredentialReady ? '（凭证已入凭证库）' : '')
      )
      return true
    } catch (error) {
      ui.toast(`保存失败：${error}`)
      return false
    }
  }

  async function deleteProfile(id: string) {
    const profile = profiles.value.find((item) => item.id === id)
    if (!profile) return
    try {
      await ipc.sshProfileDelete(id)
    } catch (error) {
      ui.toast(`删除服务器失败：${error}`)
      return
    }
    profiles.value = profiles.value.filter((item) => item.id !== id)
    ui.toast(`已删除服务器「${profile.name}」（凭证保留在凭证库）`)
  }

  /** 刷新最近连接时间（本地列表展示用；后端 touch_last_connected 已同步） */
  function touchProfileConnected(profileId: string) {
    const profile = profiles.value.find((item) => item.id === profileId)
    if (profile) profile.lastConnectedAt = Date.now()
  }

  const toggleGroup = groupsApi.toggleGroup
  return {
    profiles,
    groups,
    expandedIds,
    toggleGroup,
    loadProfiles,
    importLegacyOnce,
    createGroup,
    renameGroup,
    deleteGroup,
    moveToGroup,
    saveProfile,
    deleteProfile,
    touchProfileConnected,
  }
}
