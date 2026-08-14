/** SSH 工作区状态：服务器配置、多连接工作区、凭证与工具生命周期清理。 */
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { useSettingsStore } from '@/stores/settings'
import { useUiStore } from '@/stores/ui'
import { ipc, onConnectionStatus } from './ipc'
import { loadProfiles, persistProfiles } from './useSsh'
import type { ServerConnection, ServerProfile } from './contracts'

export type SshWorkspaceSection =
  'terminal' | 'files' | 'monitor' | 'services' | 'processes' | 'docker'

/** 一个连接页签代表一条独立 SSH 连接，并拥有完整的右侧功能区。 */
export interface SshConnectionWorkspace {
  id: string
  profileId: string
  title: string
  connection: ServerConnection
  activeSection: SshWorkspaceSection
  visitedSections: SshWorkspaceSection[]
  lastActivityAt: number
}

let nextWorkspaceId = 1

/** 创建 SSH 主工作区的状态与操作。每个连接页签独占一条 SSH 连接。 */
export function useSshWorkspace() {
  const ui = useUiStore()
  const settings = useSettingsStore()
  const profiles = ref<ServerProfile[]>(loadProfiles())
  const connectionWorkspaces = ref<SshConnectionWorkspace[]>([])
  const activeProfileId = ref<string | null>(null)
  const searchKeyword = ref('')
  const formOpen = ref(false)
  const editingProfile = ref<ServerProfile | null>(null)
  const deleteTarget = ref<ServerProfile | null>(null)
  const pendingConnections = new Set<Promise<ServerConnection>>()

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

  function showError(message: string) {
    ui.toast(message)
  }

  async function saveProfile(
    profile: ServerProfile,
    credentials: { password?: string; privateKey?: string; passphrase?: string }
  ) {
    const index = profiles.value.findIndex((item) => item.id === profile.id)
    const authChanged = index < 0 || profiles.value[index].authMethod !== profile.authMethod
    const credentialReady =
      profile.authMethod === 'password'
        ? Boolean(credentials.password)
        : Boolean(credentials.privateKey) &&
          (profile.authMethod !== 'privateKeyWithPassphrase' || Boolean(credentials.passphrase))
    if (authChanged && !credentialReady) {
      ui.toast('新增服务器或切换认证方式时必须填写完整凭证')
      return
    }
    try {
      if (credentials.password || credentials.privateKey || credentials.passphrase) {
        await ipc.sshCredentialSave({ profile, ...credentials })
      }
    } catch (error) {
      ui.toast(`凭证保存失败：${error}`)
      return
    }
    if (index >= 0) {
      profiles.value[index] = profile
      ui.toast(`已更新服务器「${profile.name}」`)
    } else {
      profiles.value.push(profile)
      ui.toast(`已添加服务器「${profile.name}」`)
    }
    persistProfiles(profiles.value)
    formOpen.value = false
  }

  async function disconnectConnection(connection: ServerConnection) {
    await ipc.sshDisconnect(connection.sessionId).catch(() => undefined)
  }

  async function closeConnectionWorkspace(id: string) {
    const workspace = connectionWorkspaces.value.find((item) => item.id === id)
    if (!workspace) return
    connectionWorkspaces.value = connectionWorkspaces.value.filter((item) => item.id !== id)
    await disconnectConnection(workspace.connection)
  }

  async function deleteProfile(id: string) {
    const profile = profiles.value.find((item) => item.id === id)
    if (!profile) return
    const related = connectionWorkspaces.value.filter((item) => item.profileId === id)
    connectionWorkspaces.value = connectionWorkspaces.value.filter((item) => item.profileId !== id)
    try {
      await Promise.all(related.map((workspace) => disconnectConnection(workspace.connection)))
      await ipc.sshCredentialDelete(id)
    } catch (error) {
      ui.toast(`删除服务器失败：${error}`)
      return
    }
    profiles.value = profiles.value.filter((item) => item.id !== id)
    if (activeProfileId.value === id) activeProfileId.value = null
    persistProfiles(profiles.value)
    ui.toast(`已删除服务器「${profile.name}」`)
  }

  function requestDelete(profile: ServerProfile) {
    deleteTarget.value = profile
  }

  async function confirmDelete() {
    if (!deleteTarget.value) return
    await deleteProfile(deleteTarget.value.id)
    deleteTarget.value = null
  }

  /** 双击服务器始终建立一条新连接，并返回一个完整连接工作区。 */
  async function openConnection(profileId: string): Promise<SshConnectionWorkspace | undefined> {
    const profile = profiles.value.find((item) => item.id === profileId)
    if (!profile) return undefined
    activeProfileId.value = profileId
    try {
      const credentials = await ipc.sshCredentialGet(profileId)
      const request = ipc.sshConnect({
        profile,
        password: credentials.password,
        privateKey: credentials.privateKey,
        passphrase: credentials.passphrase,
      })
      pendingConnections.add(request)
      const connection = await request.finally(() => pendingConnections.delete(request))
      if (disposed) {
        await disconnectConnection(connection)
        return undefined
      }
      const usedTitles = new Set(
        connectionWorkspaces.value
          .filter((workspace) => workspace.profileId === profileId)
          .map((workspace) => workspace.title)
      )
      let workspaceNumber = 1
      let title = profile.name
      while (usedTitles.has(title)) {
        workspaceNumber += 1
        title = `${profile.name} ${workspaceNumber}`
      }
      const workspace: SshConnectionWorkspace = {
        id: `ssh-workspace-${Date.now()}-${nextWorkspaceId++}`,
        profileId,
        title,
        connection,
        activeSection: 'terminal',
        visitedSections: ['terminal'],
        lastActivityAt: Date.now(),
      }
      connectionWorkspaces.value.push(workspace)
      profile.lastConnectedAt = Date.now()
      persistProfiles(profiles.value)
      return workspace
    } catch (error) {
      if (!disposed) ui.toast(`连接失败：${error}`)
      return undefined
    }
  }

  function touchWorkspace(id: string) {
    const workspace = connectionWorkspaces.value.find((item) => item.id === id)
    if (workspace?.connection.status === 'connected') workspace.lastActivityAt = Date.now()
  }

  /** 使用已保存的配置与凭证恢复断开的工作区，工作区 id 与页签保持不变。 */
  async function reconnectWorkspace(id: string) {
    const workspace = connectionWorkspaces.value.find((item) => item.id === id)
    if (!workspace || ['connected', 'reconnecting'].includes(workspace.connection.status)) return
    const profile = profiles.value.find((item) => item.id === workspace.profileId)
    if (!profile) return
    const disconnected = workspace.connection
    workspace.connection = { ...disconnected, status: 'reconnecting', error: undefined }
    try {
      const credentials = await ipc.sshCredentialGet(profile.id)
      const connection = await ipc.sshReconnect(disconnected.sessionId, {
        profile,
        password: credentials.password,
        privateKey: credentials.privateKey,
        passphrase: credentials.passphrase,
      })
      if (disposed || !connectionWorkspaces.value.includes(workspace)) {
        await disconnectConnection(connection)
        return
      }
      workspace.connection = connection
      workspace.lastActivityAt = Date.now()
      ui.toast(`连接「${workspace.title}」已恢复`)
    } catch (error) {
      workspace.connection = { ...disconnected, status: 'disconnected', error: String(error) }
      ui.toast(`重新连接失败：${error}`)
    }
  }

  async function cleanupAll() {
    const connections = connectionWorkspaces.value.map((workspace) => workspace.connection)
    connectionWorkspaces.value = []
    await Promise.all(connections.map(disconnectConnection))
  }

  let unlistenConnection: (() => void) | null = null
  let idleTimer: ReturnType<typeof setInterval> | null = null
  let disposed = false

  onMounted(async () => {
    // v2 将历史默认值 30 分钟一次性迁移为 10 分钟；之后仍允许用户自行修改。
    if (!settings.getToolSetting('ssh', 'idleDisconnectV2', false)) {
      try {
        await settings.setToolSetting('ssh', 'idleDisconnectMinutes', '10')
        await settings.setToolSetting('ssh', 'idleDisconnectV2', true)
      } catch {
        /* 设置持久化失败不应阻断 SSH 会话初始化，本次仍使用 10 分钟回退值。 */
      }
    }
    // 不恢复上一次工具实例遗留的后端会话；重新打开 SSH 工具永远从空状态开始。
    try {
      const stale = await ipc.sshConnections()
      await Promise.all(stale.map(disconnectConnection))
    } catch {
      /* 浏览器预览没有 Tauri IPC。 */
    }
    try {
      const stop = await onConnectionStatus((connection) => {
        const workspace = connectionWorkspaces.value.find(
          (item) => item.connection.sessionId === connection.sessionId
        )
        if (!workspace) return
        workspace.connection = connection
      })
      if (disposed) stop()
      else unlistenConnection = stop
    } catch {
      /* 浏览器预览没有 Tauri 事件系统。 */
    }

    idleTimer = setInterval(() => {
      const minutes = Number(settings.getToolSetting('ssh', 'idleDisconnectMinutes', '10'))
      if (!Number.isFinite(minutes) || minutes <= 0) return
      const cutoff = Date.now() - minutes * 60_000
      const expired = connectionWorkspaces.value.filter(
        (workspace) =>
          workspace.connection.status === 'connected' && workspace.lastActivityAt < cutoff
      )
      for (const workspace of expired) {
        workspace.connection = { ...workspace.connection, status: 'disconnected' }
        void disconnectConnection(workspace.connection)
        ui.toast(`连接「${workspace.title}」空闲超过 ${minutes} 分钟，已自动断开`)
      }
    }, 30_000)
  })

  onUnmounted(() => {
    disposed = true
    unlistenConnection?.()
    if (idleTimer) clearInterval(idleTimer)
    void cleanupAll()
  })

  return {
    profiles,
    connectionWorkspaces,
    activeProfileId,
    searchKeyword,
    filteredProfiles,
    formOpen,
    editingProfile,
    deleteTarget,
    openAddForm,
    openEditForm,
    showError,
    saveProfile,
    requestDelete,
    confirmDelete,
    openConnection,
    closeConnectionWorkspace,
    reconnectWorkspace,
    touchWorkspace,
  }
}
