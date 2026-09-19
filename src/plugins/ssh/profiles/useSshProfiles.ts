/**
 * useSshProfiles · 服务器配置与分组状态的唯一所有者
 *
 * 职责：服务器列表（后端插件库 `ssh.db`）、分组与折叠、搜索过滤、新增/编辑表单状态、
 * 删除确认目标、localStorage 存量一次性迁移。连接、会话与主机密钥不在本域，
 * 需要「删除前关掉该服务器名下连接」这类动作时经 {@link SshProfilePorts} 端口回调给连接域。
 *
 * 数据来源唯一：列表只来自 `ssh_profile_list`，本地 localStorage 只作为一次性迁移来源，
 * 迁移后清空快照，不做第二份副本。
 */
import { computed, onScopeDispose, ref } from 'vue'
import { onSpaceDataChanged } from '@/core/ipc/spaceEvents'
import { useUiStore } from '@/stores/ui'
import { ipc } from '../ipc'
import { useServerGroups } from './useServerGroups'
import type { UiCollectionMove } from '@/core/ui'
import { serverTreeDestination, UNGROUPED_DROP_KEY } from './serverTree'
import type { ServerProfile } from '../contracts'

/** 手工输入的认证；后端按保存方式写入本地库、Vault，或仅用于内存连接。 */
export interface SshProfileCredentials {
  password?: string
  privateKey?: string
  passphrase?: string
}

/**
 * 配置域对连接域的端口：配置域不持有连接工作区与选中态，删除服务器时经这两个回调
 * 关闭它名下的会话并清理选中态。默认实现为空操作，便于本模块独立使用。
 */
export interface SshProfilePorts {
  /** 删除服务器前关闭其名下连接工作区并断开会话 */
  closeConnectionsOf: (profileId: string) => Promise<void>
  /** 服务器被删除后清掉指向它的选中态（页签不再指向已不存在的配置） */
  forgetActiveProfile: (profileId: string) => void
  onSaved?: (profile: ServerProfile, credentials: SshProfileCredentials) => void
}

const NOOP_PORTS: SshProfilePorts = {
  closeConnectionsOf: async () => undefined,
  forgetActiveProfile: () => undefined,
}

export function useSshProfiles(ports: SshProfilePorts = NOOP_PORTS) {
  const ui = useUiStore()
  const profiles = ref<ServerProfile[]>([])
  const treeMoving = ref(false)
  let pendingTreeWrites = 0
  async function libraryWrite(action: () => Promise<void>) {
    if (treeMoving.value) {
      ui.toast('目录正在移动，请稍后重试')
      return
    }
    pendingTreeWrites++
    try {
      await action()
    } finally {
      pendingTreeWrites--
    }
  }
  const searchKeyword = ref('')
  const formOpen = ref(false)
  /** 表单当前编辑的副本（新建为 null）；保存失败时保留，用户可继续编辑 */
  const editingProfile = ref<ServerProfile | null>(null)
  const deleteTarget = ref<ServerProfile | null>(null)

  /* ── 分组（后端 ssh.db 持久化；折叠状态不持久化，重开工具默认全折叠） ── */
  const groupsApi = useServerGroups()
  const { groups, expandedIds, toggleGroup } = groupsApi

  async function loadProfiles() {
    try {
      profiles.value = await ipc.sshProfileList()
    } catch {
      /* 浏览器预览没有 IPC */
    }
  }

  /** 拉取分组（后端 ssh.db；折叠状态不持久化，重开工具默认全折叠） */
  async function loadGroups() {
    await groupsApi.load()
  }

  // 合并/覆盖导入或快照还原后原地刷新（不重启生效）；作用域销毁即解绑
  const offSpaceDataChanged = onSpaceDataChanged((datasets) => {
    if (datasets.includes('*') || datasets.some((dataset) => dataset.startsWith('ssh.'))) {
      void loadProfiles()
      void loadGroups()
    }
  })
  onScopeDispose(offSpaceDataChanged)

  /** 新建分组 */
  async function createGroup(name: string) {
    const group = await groupsApi.createGroup(name)
    ui.toast(`已创建分组「${group.name}」`)
  }

  /** 重命名分组 */
  async function renameGroup(groupId: string, name: string) {
    await groupsApi.renameGroup(groupId, name)
    ui.toast('已重命名分组')
  }

  /** 删除分组：组内连接移回未分组（连接配置本身不删） */
  async function deleteGroup(groupId: string) {
    const group = groups.value.find((g) => g.id === groupId)
    await groupsApi.deleteGroup(groupId)
    for (const profile of profiles.value) {
      if (profile.groupId === groupId) profile.groupId = undefined
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

  async function moveTree(move: UiCollectionMove) {
    if (treeMoving.value || pendingTreeWrites) {
      ui.toast('配置正在保存或移动，请稍后重试')
      return
    }
    const payload = serverTreeDestination(move, profiles.value)
    if (!payload) return
    treeMoving.value = true
    try {
      await ipc.sshTreeMove(payload)
      // 写入成功后才更新列表，失败不会留下虚假的新顺序。
      const [nextProfiles, nextGroups] = await Promise.all([
        ipc.sshProfileList(),
        ipc.sshGroupList(),
      ])
      profiles.value = nextProfiles
      groups.value = nextGroups
      if (payload.kind === 'profile')
        expandedIds.value = new Set([...expandedIds.value, payload.parent || UNGROUPED_DROP_KEY])
      ui.toast('位置已保存')
    } catch (error) {
      ui.toast(`移动或刷新失败：${error}`)
    } finally {
      treeMoving.value = false
    }
  }

  /** 关键词过滤（名称/主机/用户名，大小写不敏感） */
  const filteredProfiles = computed(() => {
    const keyword = searchKeyword.value.trim().toLowerCase()
    if (!keyword) return profiles.value
    return profiles.value.filter(
      (profile) =>
        profile.name.toLowerCase().includes(keyword) ||
        profile.host.toLowerCase().includes(keyword) ||
        profile.username.toLowerCase().includes(keyword)
    )
  })

  function openAddForm() {
    editingProfile.value = null
    formOpen.value = true
  }

  function openEditForm(profile: ServerProfile) {
    editingProfile.value = { ...profile }
    formOpen.value = true
  }

  /** 表单校验等本地错误提示（服务器表单的 @error 出口） */
  function showError(message: string) {
    ui.toast(message)
  }

  /**
   * 保存服务器：配置写插件库；认证按选择保存在本地库或 Vault，也可不保存。
   * （同 profile upsert 同一条 Vault 条目，不产生重复）。
   * 新增或切换认证方式必须带齐凭证：缺凭证时拒绝提交且不发 IPC。
   */
  async function saveProfile(
    profile: ServerProfile,
    credentials: SshProfileCredentials,
    saveCredential: boolean,
    saveLocal = false
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
    if (saveCredential && !manualCredentialReady) {
      ui.toast('保存到凭证库需要重新填写完整的密码或私钥')
      return
    }
    if ((index < 0 || authChanged || switchedFromVault) && !credentialReady) {
      ui.toast('新增服务器或切换认证方式时必须填写完整凭证')
      return
    }
    try {
      const saved = await ipc.sshProfileSave({
        profile,
        ...credentials,
        saveCredential: saveCredential && manualCredentialReady,
        saveLocal,
      })
      const existing = profiles.value.findIndex((item) => item.id === saved.id)
      ports.onSaved?.(saved, credentials)
      if (existing >= 0) profiles.value[existing] = saved
      else profiles.value.push(saved)
      ui.toast(
        `${existing >= 0 ? '已更新' : '已添加'}服务器「${saved.name}」` +
          (saveLocal
            ? '（认证信息已本地保存）'
            : saveCredential && manualCredentialReady
              ? '（凭证已入凭证库）'
              : '')
      )
      formOpen.value = false
    } catch (error) {
      // 保存失败：表单与编辑数据保留，用户可改完重试
      ui.toast(`保存失败：${error}`)
    }
  }

  /**
   * 删除服务器：先经端口关闭其名下连接并断开会话，再删配置。
   * 后端删除失败时提示可见且配置保留（不伪装成功）。
   */
  async function deleteProfile(id: string) {
    const profile = profiles.value.find((item) => item.id === id)
    if (!profile) return
    try {
      await ports.closeConnectionsOf(id)
      await ipc.sshProfileDelete(id)
    } catch (error) {
      ui.toast(`删除服务器失败：${error}`)
      return
    }
    profiles.value = profiles.value.filter((item) => item.id !== id)
    ports.forgetActiveProfile(id)
    ui.toast(
      `已删除服务器「${profile.name}」${profile.credentialRef ? '（凭证保留在凭证库）' : ''}`
    )
  }

  function requestDelete(profile: ServerProfile) {
    deleteTarget.value = profile
  }

  async function confirmDelete() {
    if (!deleteTarget.value) return
    await deleteProfile(deleteTarget.value.id)
    deleteTarget.value = null
  }

  /** 刷新最近连接时间（本地列表展示用；后端 touch_last_connected 已同步） */
  function touchProfileConnected(profileId: string) {
    const profile = profiles.value.find((item) => item.id === profileId)
    if (profile) profile.lastConnectedAt = Date.now()
  }

  return {
    profiles,
    groups,
    expandedIds,
    searchKeyword,
    filteredProfiles,
    formOpen,
    editingProfile,
    deleteTarget,
    openAddForm,
    openEditForm,
    toggleGroup,
    loadProfiles,
    loadGroups,
    createGroup: (...args: Parameters<typeof createGroup>) =>
      libraryWrite(() => createGroup(...args)),
    renameGroup: (...args: Parameters<typeof renameGroup>) =>
      libraryWrite(() => renameGroup(...args)),
    deleteGroup: (...args: Parameters<typeof deleteGroup>) =>
      libraryWrite(() => deleteGroup(...args)),
    moveToGroup,
    moveTree,
    treeMoving,
    saveProfile: (...args: Parameters<typeof saveProfile>) =>
      libraryWrite(() => saveProfile(...args)),
    requestDelete,
    confirmDelete: () => libraryWrite(confirmDelete),
    showError,
    touchProfileConnected,
  }
}
