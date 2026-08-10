/** SSH 工作区状态：服务器配置、连接竞态、凭证与后端状态事件。 */
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { useUiStore } from '@/stores/ui'
import { ipc, onConnectionStatus } from './ipc'
import { loadProfiles, persistProfiles } from './useSsh'
import type { ServerConnection, ServerProfile } from './contracts'

/** 创建 SSH 主工作区的状态与操作。 */
export function useSshWorkspace() {
  const ui = useUiStore()
  const profiles = ref<ServerProfile[]>(loadProfiles())
  const connections = ref<ServerConnection[]>([])
  const activeProfileId = ref<string | null>(null)
  const connectionAttempts = new Map<string, number>()
  const searchKeyword = ref('')
  const formOpen = ref(false)
  const editingProfile = ref<ServerProfile | null>(null)
  const deleteTarget = ref<ServerProfile | null>(null)

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
  const activeConnection = computed(() =>
    connections.value.find((connection) => connection.profileId === activeProfileId.value)
  )
  const usableConnection = computed(() =>
    activeConnection.value?.status === 'connected' ? activeConnection.value : undefined
  )

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

  async function deleteProfile(id: string) {
    const profile = profiles.value.find((item) => item.id === id)
    if (!profile) return
    const connection = connections.value.find((item) => item.profileId === id)
    connectionAttempts.set(id, (connectionAttempts.get(id) ?? 0) + 1)
    try {
      if (connection?.sessionId) await ipc.sshDisconnect(connection.sessionId)
      await ipc.sshCredentialDelete(id)
    } catch (error) {
      ui.toast(`删除服务器失败：${error}`)
      return
    }
    profiles.value = profiles.value.filter((item) => item.id !== id)
    connections.value = connections.value.filter((item) => item.profileId !== id)
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

  async function connect(profileId: string) {
    const profile = profiles.value.find((item) => item.id === profileId)
    if (!profile) return
    const existing = connections.value.find((item) => item.profileId === profileId)
    if (existing?.status === 'connected' || existing?.status === 'connecting') {
      activeProfileId.value = profileId
      return
    }
    connections.value = connections.value.filter((item) => item.profileId !== profileId)
    connections.value.push({
      profileId,
      sessionId: `pending-${profileId}`,
      status: 'connecting',
      host: profile.host,
    })
    activeProfileId.value = profileId
    const attempt = (connectionAttempts.get(profileId) ?? 0) + 1
    connectionAttempts.set(profileId, attempt)
    try {
      const credentials = await ipc.sshCredentialGet(profileId)
      if (connectionAttempts.get(profileId) !== attempt) return
      const connection = await ipc.sshConnect({
        profile,
        password: credentials.password,
        privateKey: credentials.privateKey,
        passphrase: credentials.passphrase,
      })
      if (connectionAttempts.get(profileId) !== attempt) {
        await ipc.sshDisconnect(connection.sessionId).catch(() => undefined)
        return
      }
      connections.value = connections.value.filter((item) => item.profileId !== profileId)
      connections.value.push(connection)
      activeProfileId.value = profileId
      profile.lastConnectedAt = Date.now()
      persistProfiles(profiles.value)
      ui.toast(`已连接到 ${connection.host ?? profile.host}`)
    } catch (error) {
      if (connectionAttempts.get(profileId) !== attempt) return
      connections.value = connections.value.filter((item) => item.profileId !== profileId)
      ui.toast(`连接失败：${error}`)
    }
  }

  async function disconnect(profileId: string) {
    connectionAttempts.set(profileId, (connectionAttempts.get(profileId) ?? 0) + 1)
    const connection = connections.value.find((item) => item.profileId === profileId)
    if (!connection?.sessionId) return
    try {
      await ipc.sshDisconnect(connection.sessionId)
      connections.value = connections.value.filter((item) => item.profileId !== profileId)
      ui.toast('已断开连接')
    } catch (error) {
      ui.toast(`断开失败：${error}`)
    }
  }

  let unlistenConnection: (() => void) | null = null
  let disposed = false
  onMounted(async () => {
    try {
      const initial = await ipc.sshConnections()
      if (disposed) return
      connections.value = initial
      if (connections.value.length > 0 && !activeProfileId.value) {
        activeProfileId.value = connections.value[0].profileId
      }
    } catch {
      /* 浏览器预览没有 Tauri IPC。 */
    }
    try {
      const stop = await onConnectionStatus((connection) => {
        if (connection.status === 'disconnected') {
          connections.value = connections.value.filter(
            (item) => item.profileId !== connection.profileId
          )
          return
        }
        const index = connections.value.findIndex((item) => item.profileId === connection.profileId)
        if (index >= 0) connections.value[index] = connection
        else connections.value.push(connection)
      })
      if (disposed) stop()
      else unlistenConnection = stop
    } catch {
      /* 浏览器预览没有 Tauri 事件系统。 */
    }
  })
  onUnmounted(() => {
    disposed = true
    unlistenConnection?.()
  })

  return {
    profiles,
    connections,
    activeProfileId,
    searchKeyword,
    filteredProfiles,
    activeConnection,
    usableConnection,
    formOpen,
    editingProfile,
    deleteTarget,
    openAddForm,
    openEditForm,
    showError,
    saveProfile,
    requestDelete,
    confirmDelete,
    connect,
    disconnect,
  }
}
