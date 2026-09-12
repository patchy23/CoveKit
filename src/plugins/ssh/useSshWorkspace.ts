/** SSH 工作区状态：服务器配置（后端插件库）、分组、多连接工作区、主机密钥确认、分阶段连接与自动重连。 */
import { computed, onMounted, onUnmounted, reactive, ref } from 'vue'
import { useSettingsStore } from '@/stores/settings'
import { useUiStore } from '@/stores/ui'
import {
  ipc,
  onConnectionStatus,
  onConnectStage,
  onHostKeyVerify,
  onTerminalClosed,
  onTerminalData,
  onTransferProgress,
} from './ipc'
import { clearLegacySnapshot, readLegacySnapshot } from './connection/useSsh'
import { useServerGroups } from './profiles/useServerGroups'
import type { HostKeyVerifyRequest, ServerConnection, ServerProfile } from './contracts'

export type SshWorkspaceSection =
  'terminal' | 'files' | 'tunnels' | 'monitor' | 'services' | 'processes' | 'docker'

/** 一个连接页签代表一条独立 SSH 连接，并拥有完整的右侧功能区。 */
export interface SshConnectionWorkspace {
  id: string
  profileId: string
  title: string
  connection: ServerConnection
  /** 本次连接尝试的后端请求 id（首个 connect-stage 事件完成绑定，之后精确匹配进度） */
  connectRequestId?: string
  /** 连接进度文案（connecting/reconnecting 期间由 connect-stage 事件驱动） */
  stageText: string
  /** 显式连接请求计数（驱动 TerminalTab 开启终端通道） */
  connectRequest: number
  /** 断线自动重连已尝试次数（0 = 未在自动重连） */
  reconnectAttempt: number
  /** 重连成功计数（驱动 TerminalTab 保留缓冲并插入分隔线） */
  reconnectTick: number
  activeSection: SshWorkspaceSection
  visitedSections: SshWorkspaceSection[]
  lastActivityAt: number
}

let nextWorkspaceId = 1

/** 断线自动重连退避间隔（1/2/5/10/30 秒，最多 5 次） */
const RECONNECT_DELAYS_MS = [1_000, 2_000, 5_000, 10_000, 30_000]

/** 连接阶段 → 进度文案 */
function stageTextFor(stage: string, status: string): string {
  if (status === 'fail') return '连接失败'
  switch (stage) {
    case 'resolve':
      return '解析主机…'
    case 'tcp':
      return '建立连接…'
    case 'handshake':
      return 'SSH 握手…'
    case 'verify':
      return '校验主机指纹…'
    case 'auth':
      return '认证中…'
    case 'session':
      return '建立会话…'
    default:
      return '连接中…'
  }
}

/** 创建 SSH 主工作区的状态与操作。每个连接页签独占一条 SSH 连接。 */
export function useSshWorkspace() {
  const ui = useUiStore()
  const settings = useSettingsStore()
  const profiles = ref<ServerProfile[]>([])
  const connectionWorkspaces = ref<SshConnectionWorkspace[]>([])
  const activeProfileId = ref<string | null>(null)
  const searchKeyword = ref('')
  const formOpen = ref(false)
  const editingProfile = ref<ServerProfile | null>(null)
  const deleteTarget = ref<ServerProfile | null>(null)
  /**
   * 主机密钥确认请求队列：并发首连多台主机时排队逐个确认（单值会覆盖导致先到的永远挂起）。
   * hostKeyRequest 暴露队首给 UI；卸载时对积压请求统一 cancel。
   */
  const hostKeyQueue = ref<HostKeyVerifyRequest[]>([])
  const hostKeyRequest = computed(() => hostKeyQueue.value[0] ?? null)

  const pendingConnections = new Set<Promise<unknown>>()

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
      if (result.legacyCredentialsFailed) {
        ui.toast(
          `已迁移 ${result.importedProfiles} 台服务器；旧凭证未能自动迁移，请编辑服务器重新保存凭证`
        )
      } else {
        ui.toast(
          `已迁移 ${result.importedProfiles} 台服务器` +
            (result.migratedCredentials > 0
              ? `，${result.migratedCredentials} 个凭证已入凭证库`
              : '')
        )
      }
    } catch (error) {
      // 迁移失败保留 localStorage 快照，下次打开工具重试；错误必须可见（曾静默导致"数据消失"假象）
      console.error('[ssh] 存量迁移失败:', error)
      ui.toast(`存量配置迁移失败：${error}`)
    }
  }

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

  /**
   * 保存服务器：配置写插件库；勾选保存凭证时后端把手工凭证写入 Vault 并回填 credentialRef
   * （同 profile upsert 同一条 Vault 条目，不产生重复）。
   */
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
      formOpen.value = false
    } catch (error) {
      ui.toast(`保存失败：${error}`)
    }
  }

  async function disconnectConnection(connection: ServerConnection) {
    if (!connection.sessionId) return
    await ipc.sshDisconnect(connection.sessionId).catch(() => undefined)
  }

  async function closeConnectionWorkspace(id: string) {
    const workspace = connectionWorkspaces.value.find((item) => item.id === id)
    if (!workspace) return
    connectionWorkspaces.value = connectionWorkspaces.value.filter((item) => item.id !== id)
    await disconnectConnection(workspace.connection)
  }

  /** 一键清理：断开并关闭全部连接工作区（右侧页签条「关闭全部会话」） */
  async function closeAllWorkspaces() {
    const count = connectionWorkspaces.value.length
    for (const workspace of [...connectionWorkspaces.value]) {
      await disconnectConnection(workspace.connection)
    }
    connectionWorkspaces.value = []
    ui.toast(`已关闭全部 ${count} 个会话`)
  }

  async function deleteProfile(id: string) {
    const profile = profiles.value.find((item) => item.id === id)
    if (!profile) return
    const related = connectionWorkspaces.value.filter((item) => item.profileId === id)
    connectionWorkspaces.value = connectionWorkspaces.value.filter((item) => item.profileId !== id)
    try {
      await Promise.all(related.map((workspace) => disconnectConnection(workspace.connection)))
      await ipc.sshProfileDelete(id)
    } catch (error) {
      ui.toast(`删除服务器失败：${error}`)
      return
    }
    profiles.value = profiles.value.filter((item) => item.id !== id)
    if (activeProfileId.value === id) activeProfileId.value = null
    ui.toast(`已删除服务器「${profile.name}」（凭证保留在凭证库）`)
  }

  function requestDelete(profile: ServerProfile) {
    deleteTarget.value = profile
  }

  async function confirmDelete() {
    if (!deleteTarget.value) return
    await deleteProfile(deleteTarget.value.id)
    deleteTarget.value = null
  }

  /** 连接成功后为工作区生成不重复标题（「名称」「名称 2」…） */
  function dedupeTitle(profileName: string, profileId: string): string {
    const usedTitles = new Set(
      connectionWorkspaces.value
        .filter((workspace) => workspace.profileId === profileId)
        .map((workspace) => workspace.title)
    )
    let title = profileName
    let number = 1
    while (usedTitles.has(title)) {
      number += 1
      title = `${profileName} ${number}`
    }
    return title
  }

  /** 双击服务器建立一条新连接：先建占位工作区承接分阶段进度，连接成功后填入会话。 */
  async function openConnection(profileId: string): Promise<SshConnectionWorkspace | undefined> {
    const profile = profiles.value.find((item) => item.id === profileId)
    if (!profile) return undefined
    activeProfileId.value = profileId
    // reactive 包裹：闭包后续对 workspace 的赋值（connected/connectRequest 等）必须触发响应式更新，
    // 否则页签状态点与终端 props 要等下一次任意重渲染才刷新（曾致"开第二个连接第一个才变绿"）
    const workspace = reactive<SshConnectionWorkspace>({
      id: `ssh-workspace-${Date.now()}-${nextWorkspaceId++}`,
      profileId,
      title: dedupeTitle(profile.name, profileId),
      connection: { profileId, sessionId: '', status: 'connecting' },
      stageText: '准备连接…',
      connectRequest: 0,
      reconnectAttempt: 0,
      reconnectTick: 0,
      activeSection: 'terminal',
      visitedSections: ['terminal'],
      lastActivityAt: Date.now(),
    })
    connectionWorkspaces.value.push(workspace)
    const request = ipc.sshConnect({ profileId }).finally(() => pendingConnections.delete(request))
    pendingConnections.add(request)
    try {
      const outcome = await request
      if (disposed || !connectionWorkspaces.value.includes(workspace)) {
        // 组件卸载，或占位页签在连接期间被用户关闭：会话无人持有，立即断开防泄漏
        if (outcome.ok && outcome.connection) await disconnectConnection(outcome.connection)
        removeWorkspace(workspace.id)
        return undefined
      }
      if (!outcome.ok || !outcome.connection) {
        removeWorkspace(workspace.id)
        const error = outcome.error
        ui.toast(error ? `${error.message}（${error.code}）` : '连接失败')
        return undefined
      }
      workspace.connection = outcome.connection
      workspace.connectRequestId = outcome.requestId
      workspace.connectRequest += 1
      workspace.stageText = ''
      workspace.lastActivityAt = Date.now()
      profile.lastConnectedAt = Date.now()
      return workspace
    } catch (error) {
      removeWorkspace(workspace.id)
      if (!disposed) ui.toast(`连接失败：${error}`)
      return undefined
    }
  }

  function removeWorkspace(id: string) {
    connectionWorkspaces.value = connectionWorkspaces.value.filter((item) => item.id !== id)
  }

  function touchWorkspace(id: string) {
    const workspace = connectionWorkspaces.value.find((item) => item.id === id)
    if (workspace?.connection.status === 'connected') workspace.lastActivityAt = Date.now()
  }

  /**
   * 意外断线（终端通道异常关闭）后的自动重连：指数退避 1/2/5/10/30 秒，最多 5 次。
   * 用户手动断开或关闭页签会移出 reconnecting 状态，从而中止后续尝试。
   */
  function handleLinkDead(workspaceId: string) {
    const workspace = connectionWorkspaces.value.find((item) => item.id === workspaceId)
    if (!workspace || workspace.connection.status !== 'connected') return
    if (!settings.getToolSetting<boolean>('ssh', 'autoReconnect', true)) {
      workspace.connection = {
        ...workspace.connection,
        status: 'disconnected',
        error: '连接已断开',
      }
      ui.toast(`连接「${workspace.title}」已断开`)
      return
    }
    scheduleAutoReconnect(workspace)
  }

  function scheduleAutoReconnect(workspace: SshConnectionWorkspace) {
    if (workspace.reconnectAttempt >= RECONNECT_DELAYS_MS.length) {
      workspace.connection = {
        ...workspace.connection,
        status: 'disconnected',
        error: '自动重连失败次数过多',
      }
      workspace.reconnectAttempt = 0
      ui.toast(`「${workspace.title}」自动重连失败次数过多，已停止（可手动重连）`)
      return
    }
    const delay = RECONNECT_DELAYS_MS[workspace.reconnectAttempt]
    workspace.reconnectAttempt += 1
    workspace.connection = { ...workspace.connection, status: 'reconnecting' }
    workspace.stageText = `${delay / 1000} 秒后自动重连（第 ${workspace.reconnectAttempt} 次）…`
    window.setTimeout(() => {
      if (
        !connectionWorkspaces.value.includes(workspace) ||
        workspace.connection.status !== 'reconnecting'
      ) {
        return // 用户已手动断开或关闭页签
      }
      void attemptReconnect(workspace)
    }, delay)
  }

  /** 执行一次重连：成功则刷新会话并让终端保留缓冲打分隔线；失败继续退避重试 */
  async function attemptReconnect(workspace: SshConnectionWorkspace) {
    try {
      const outcome = await ipc.sshReconnect(workspace.connection.sessionId)
      if (!connectionWorkspaces.value.includes(workspace)) {
        if (outcome.ok && outcome.connection) await disconnectConnection(outcome.connection)
        return
      }
      if (outcome.ok && outcome.connection) {
        workspace.connection = outcome.connection
        workspace.reconnectTick += 1
        workspace.reconnectAttempt = 0
        workspace.stageText = ''
        workspace.lastActivityAt = Date.now()
        ui.toast(`连接「${workspace.title}」已恢复`)
        return
      }
      const message = outcome.error?.message ?? '重连失败'
      if (outcome.error?.code === 'SESSION_NOT_FOUND') {
        // 句柄已被后端回收（多为空闲自动断开）：ssh_reconnect 无从下手，按配置整条重建
        if (await reconnectByProfile(workspace)) scheduleAutoReconnect(workspace)
        return
      }
      if (outcome.error?.code === 'PROFILE_NOT_FOUND') {
        workspace.connection = {
          ...workspace.connection,
          status: 'disconnected',
          error: message,
        }
        ui.toast(message)
        return
      }
      scheduleAutoReconnect(workspace)
    } catch {
      // IPC 抛错（多为网络断）与业务失败一致走退避；上限由 scheduleAutoReconnect 自终止
      scheduleAutoReconnect(workspace)
    }
  }

  /**
   * 旧会话句柄已被后端回收时的整条重连（空闲自动断开、后端重启、其它入口断开）。
   * ssh_reconnect 需要一个仍在会话表里的旧句柄，句柄没了只能按 profileId 重新 ssh_connect，
   * 再把新会话挂回原工作区：终端保留缓冲，reconnectTick 递增走「换通道 + 分隔线」分支。
   * 返回 null 表示成功，否则为失败说明（调用方决定是报错还是继续退避）。
   */
  async function reconnectByProfile(workspace: SshConnectionWorkspace): Promise<string | null> {
    try {
      const outcome = await ipc.sshConnect({ profileId: workspace.profileId })
      if (disposed || !connectionWorkspaces.value.includes(workspace)) {
        if (outcome.ok && outcome.connection) await disconnectConnection(outcome.connection)
        return null
      }
      if (outcome.ok && outcome.connection) {
        workspace.connection = outcome.connection
        workspace.connectRequestId = outcome.requestId
        workspace.reconnectTick += 1
        workspace.reconnectAttempt = 0
        workspace.stageText = ''
        workspace.lastActivityAt = Date.now()
        ui.toast(`连接「${workspace.title}」已恢复`)
        return null
      }
      return outcome.error?.message ?? '重新连接失败'
    } catch (error) {
      return String(error)
    }
  }

  /** 手动重连（终端「重连」按钮 / 页签操作）：保留工作区与终端缓冲 */
  async function reconnectWorkspace(id: string) {
    const workspace = connectionWorkspaces.value.find((item) => item.id === id)
    if (!workspace || ['connected', 'reconnecting'].includes(workspace.connection.status)) return
    const disconnected = workspace.connection
    workspace.connection = { ...disconnected, status: 'reconnecting' }
    workspace.stageText = '重新连接…'
    workspace.reconnectAttempt = 0
    try {
      const outcome = await ipc.sshReconnect(disconnected.sessionId)
      if (disposed || !connectionWorkspaces.value.includes(workspace)) {
        if (outcome.ok && outcome.connection) await disconnectConnection(outcome.connection)
        return
      }
      if (outcome.ok && outcome.connection) {
        workspace.connection = outcome.connection
        workspace.reconnectTick += 1
        workspace.stageText = ''
        workspace.lastActivityAt = Date.now()
        ui.toast(`连接「${workspace.title}」已恢复`)
        return
      }
      const error = outcome.error
      if (error?.code === 'SESSION_NOT_FOUND') {
        // 句柄已被后端回收（空闲自动断开等）：改按 profileId 整条重建，成功后保留终端缓冲
        const failure = await reconnectByProfile(workspace)
        if (!failure) return
        workspace.connection = { ...disconnected, status: 'disconnected', error: failure }
        ui.toast(`重新连接失败：${failure}`)
        return
      }
      workspace.connection = {
        ...disconnected,
        status: 'disconnected',
        error: error?.message ?? '重连失败',
      }
      ui.toast(error ? `${error.message}（${error.code}）` : '重新连接失败')
    } catch (error) {
      workspace.connection = { ...disconnected, status: 'disconnected', error: String(error) }
      ui.toast(`重新连接失败：${error}`)
    }
  }

  /* ── 主机密钥人工确认 ── */

  /** 应答队首确认请求；取消时后端连接失败，占位工作区由 openConnection 清理 */
  async function respondHostKey(decision: 'trustOnce' | 'trustSave' | 'cancel' | 'replace') {
    const request = hostKeyQueue.value.shift()
    if (!request) return
    await ipc.sshHostKeyRespond({ requestId: request.requestId, decision }).catch(() => undefined)
  }

  async function cleanupAll() {
    const connections = connectionWorkspaces.value.map((workspace) => workspace.connection)
    connectionWorkspaces.value = []
    await Promise.all(connections.map(disconnectConnection))
  }

  let unlistenConnection: (() => void) | null = null
  let unlistenActivity: (() => void) | null = null
  let unlistenTransfer: (() => void) | null = null
  let unlistenClosed: (() => void) | null = null
  let unlistenStage: (() => void) | null = null
  let unlistenHostKey: (() => void) | null = null
  let idleTimer: ReturnType<typeof setInterval> | null = null
  let disposed = false

  /** 后台活动节流表（connectionId → 上次活动时间戳），5s 内不重复刷新 */
  const ACTIVITY_TOUCH_THROTTLE = 5_000
  const lastActivityTouch = new Map<string, number>()

  /** 后台活动（终端输出含输入回显、文件传输进度）也算会话活跃——防止长任务/看日志时被空闲断开误杀 */
  function touchByConnectionId(connectionId: string) {
    const now = Date.now()
    if (now - (lastActivityTouch.get(connectionId) ?? 0) < ACTIVITY_TOUCH_THROTTLE) return
    lastActivityTouch.set(connectionId, now)
    const workspace = connectionWorkspaces.value.find(
      (item) => item.connection.sessionId === connectionId
    )
    if (workspace?.connection.status === 'connected') workspace.lastActivityAt = now
  }

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
    // localStorage 存量配置/分组一次性迁入后端插件库（旧手工凭证由后端迁入 Vault）
    await importLegacyOnce()
    await Promise.all([loadProfiles(), groupsApi.load()])

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
        // 自动重连等待期忽略旧会话的 Disconnected 推送，避免覆盖 reconnecting 状态
        if (
          connection.status === 'disconnected' &&
          workspace.connection.status === 'reconnecting'
        ) {
          return
        }
        workspace.connection = connection
      })
      if (disposed) stop()
      else unlistenConnection = stop
    } catch {
      /* 浏览器预览没有 Tauri 事件系统。 */
    }
    // 后台活动监听：终端有输出（含打字回显）/ 传输在进行 → 刷新对应工作区活跃时间
    try {
      const stopData = await onTerminalData((d) => touchByConnectionId(d.connectionId))
      const stopTransfer = await onTransferProgress((p) => touchByConnectionId(p.connectionId))
      // 意外断线检测：主终端通道异常关闭（本地主动关闭不经过此路径）→ 触发自动重连
      const stopClosed = await onTerminalClosed((d) => {
        const workspace = connectionWorkspaces.value.find(
          (item) =>
            item.connection.sessionId === d.connectionId && item.connection.status === 'connected'
        )
        if (workspace) handleLinkDead(workspace.id)
      })
      if (disposed) {
        stopData()
        stopTransfer()
        stopClosed()
      } else {
        unlistenActivity = stopData
        unlistenTransfer = stopTransfer
        unlistenClosed = stopClosed
      }
    } catch {
      /* 浏览器预览没有 Tauri 事件系统。 */
    }
    try {
      const stopStage = await onConnectStage((s) => {
        // 先按已绑定的 requestId 精确匹配；未绑定的占位工作区用「同 profile + 进行中」
        // 完成首绑（并发同 profile 双连时各自绑定，之后不再串台）
        let workspace = connectionWorkspaces.value.find(
          (item) => item.connectRequestId === s.requestId
        )
        if (!workspace) {
          workspace = connectionWorkspaces.value.find(
            (item) =>
              item.profileId === s.profileId &&
              !item.connectRequestId &&
              ['connecting', 'reconnecting'].includes(item.connection.status)
          )
          if (workspace) workspace.connectRequestId = s.requestId
        }
        if (workspace) workspace.stageText = stageTextFor(s.stage, s.status)
      })
      const stopHostKey = await onHostKeyVerify((request) => {
        hostKeyQueue.value.push(request)
      })
      if (disposed) {
        stopStage()
        stopHostKey()
      } else {
        unlistenStage = stopStage
        unlistenHostKey = stopHostKey
      }
    } catch {
      /* 浏览器预览没有 Tauri 事件系统。 */
    }

    if (disposed) return
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
    // 积压的主机密钥确认统一取消，避免后端握手回调挂到超时
    for (const pending of hostKeyQueue.value) {
      void ipc
        .sshHostKeyRespond({ requestId: pending.requestId, decision: 'cancel' })
        .catch(() => undefined)
    }
    hostKeyQueue.value = []
    unlistenConnection?.()
    unlistenActivity?.()
    unlistenTransfer?.()
    unlistenClosed?.()
    unlistenStage?.()
    unlistenHostKey?.()
    if (idleTimer) clearInterval(idleTimer)
    void cleanupAll()
  })

  return {
    profiles,
    groups,
    expandedIds,
    connectionWorkspaces,
    activeProfileId,
    searchKeyword,
    filteredProfiles,
    formOpen,
    editingProfile,
    deleteTarget,
    hostKeyRequest,
    respondHostKey,
    openAddForm,
    openEditForm,
    showError,
    saveProfile,
    requestDelete,
    confirmDelete,
    openConnection,
    closeConnectionWorkspace,
    closeAllWorkspaces,
    reconnectWorkspace,
    handleLinkDead,
    touchWorkspace,
    createGroup,
    renameGroup,
    deleteGroup,
    moveToGroup,
    toggleGroup,
  }
}
