/**
 * useSshConnections · SSH 会话工作区状态所有者
 *
 * 职责：连接页签（工作区）列表与活动页签、连接/取消/手动重连/意外断线自动重连、
 * 后台活动节流与空闲断开，以及后端事件（连接状态、阶段进度、终端通道关闭、后台活动）到工作区的路由。
 *
 * 不持有服务器配置与主机密钥队列：配置经 {@link SshConnectionPorts.findProfile} 只读获取，
 * 连接成功经 {@link SshConnectionPorts.onConnected} 通知配置域刷新最近连接时间；
 * 卸载标记由生命周期域唯一写入，本域只经 {@link SshConnectionPorts.isDisposed} 读取，
 * 用于丢弃卸载后晚到的连接结果（防会话泄漏）。
 */
import { reactive, ref } from 'vue'
import { useUiStore } from '@/stores/ui'
import { ipc } from '../ipc'
import type { ConnectStage, ServerConnection, ServerProfile } from '../contracts'

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

/** 连接页签右侧功能区（工作区内部子页签） */
export type SshWorkspaceSection =
  'terminal' | 'files' | 'tunnels' | 'monitor' | 'services' | 'processes' | 'docker' | 'compose'

/** 连接域需要的外部能力（均由组装根注入，本域不反向读配置域/生命周期域内部状态） */
export interface SshConnectionPorts {
  /** 只读查配置（占位标题、删除服务器时关闭其名下连接） */
  findProfile: (profileId: string) => ServerProfile | undefined
  /** 连接成功：通知配置域刷新该服务器的最近连接时间 */
  onConnected: (profileId: string) => void
  /** 组件是否已卸载（生命周期域唯一写入） */
  isDisposed: () => boolean
}

/** 连接页签 id 序号（同一毫秒内连开多个页签也不重复） */
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

export function useSshConnections(ports: SshConnectionPorts) {
  const ui = useUiStore()
  const connectionWorkspaces = ref<SshConnectionWorkspace[]>([])
  const activeProfileId = ref<string | null>(null)

  /** 进行中的连接请求：仅用于跟踪生命周期，避免页面关闭时请求悬空无归属 */
  const pendingConnections = new Set<Promise<unknown>>()

  /** 断开一个后端会话（无 sessionId 的占位工作区无会话可断） */
  async function disconnect(connection: ServerConnection) {
    if (!connection.sessionId) return
    await ipc.sshDisconnect(connection.sessionId).catch(() => undefined)
  }

  function removeWorkspace(id: string) {
    connectionWorkspaces.value = connectionWorkspaces.value.filter((item) => item.id !== id)
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
    const profile = ports.findProfile(profileId)
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
      if (ports.isDisposed() || !connectionWorkspaces.value.includes(workspace)) {
        // 组件卸载，或占位页签在连接期间被用户关闭：会话无人持有，立即断开防泄漏
        if (outcome.ok && outcome.connection) await disconnect(outcome.connection)
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
      ports.onConnected(profileId)
      return workspace
    } catch (error) {
      removeWorkspace(workspace.id)
      if (!ports.isDisposed()) ui.toast(`连接失败：${error}`)
      return undefined
    }
  }

  /** 关闭单个连接页签：先移出列表再断开会话（关闭期间晚到的连接结果不会重开页签） */
  async function closeConnectionWorkspace(id: string) {
    const workspace = connectionWorkspaces.value.find((item) => item.id === id)
    if (!workspace) return
    removeWorkspace(id)
    await disconnect(workspace.connection)
  }

  /** 一键清理：断开并关闭全部连接工作区（右侧页签条「关闭全部会话」） */
  async function closeAllWorkspaces() {
    const count = connectionWorkspaces.value.length
    for (const workspace of [...connectionWorkspaces.value]) {
      await disconnect(workspace.connection)
    }
    connectionWorkspaces.value = []
    ui.toast(`已关闭全部 ${count} 个会话`)
  }

  /** 卸载清理：先清空列表（同步），再断开全部会话——晚到的连接结果因此无人认领而被丢弃 */
  async function disconnectAll() {
    const connections = connectionWorkspaces.value.map((workspace) => workspace.connection)
    connectionWorkspaces.value = []
    await Promise.all(connections.map(disconnect))
  }

  /** 删除服务器前关闭其名下全部工作区并断开会话（配置域经端口调用） */
  async function closeConnectionsOf(profileId: string) {
    const related = connectionWorkspaces.value.filter((item) => item.profileId === profileId)
    connectionWorkspaces.value = connectionWorkspaces.value.filter(
      (item) => item.profileId !== profileId
    )
    await Promise.all(related.map((workspace) => disconnect(workspace.connection)))
  }

  /** 服务器被删除后清掉指向它的活动页签（配置域经端口调用） */
  function forgetActiveProfile(profileId: string) {
    if (activeProfileId.value === profileId) activeProfileId.value = null
  }

  /** 用户操作（终端键入、切换子页签等）刷新活跃时间，避免操作中被空闲断开 */
  function touchWorkspace(id: string) {
    const workspace = connectionWorkspaces.value.find((item) => item.id === id)
    if (workspace?.connection.status === 'connected') workspace.lastActivityAt = Date.now()
  }

  /** 后台活动（终端输出含输入回显、文件传输进度）按 sessionId 刷新活跃时间 */
  function touchBySessionId(connectionId: string) {
    const workspace = connectionWorkspaces.value.find(
      (item) => item.connection.sessionId === connectionId
    )
    if (workspace?.connection.status === 'connected') workspace.lastActivityAt = Date.now()
  }

  /** 空闲超时：置为已断开并回收会话（文案含标题与分钟数，提示发生在连接域） */
  function idleDisconnect(workspaceId: string, minutes: number) {
    const workspace = connectionWorkspaces.value.find((item) => item.id === workspaceId)
    if (!workspace || workspace.connection.status !== 'connected') return
    workspace.connection = { ...workspace.connection, status: 'disconnected' }
    void disconnect(workspace.connection)
    ui.toast(`连接「${workspace.title}」空闲超过 ${minutes} 分钟，已自动断开`)
  }

  /**
   * 意外断线（终端通道异常关闭）后的自动重连：指数退避 1/2/5/10/30 秒，最多 5 次。
   * 始终开启（2026-09-14 起不再提供关闭开关）；用户手动断开或关闭页签会移出
   * reconnecting 状态，从而中止后续尝试。
   */
  function handleLinkDead(workspaceId: string) {
    const workspace = connectionWorkspaces.value.find((item) => item.id === workspaceId)
    if (!workspace || workspace.connection.status !== 'connected') return
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
        if (outcome.ok && outcome.connection) await disconnect(outcome.connection)
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
      if (ports.isDisposed() || !connectionWorkspaces.value.includes(workspace)) {
        if (outcome.ok && outcome.connection) await disconnect(outcome.connection)
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
      if (ports.isDisposed() || !connectionWorkspaces.value.includes(workspace)) {
        if (outcome.ok && outcome.connection) await disconnect(outcome.connection)
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

  /* ── 后端事件 → 工作区路由（订阅由生命周期域持有，回调注入到本域） ── */

  /** 后端连接状态推送：占位工作区在会话建立前也可能收到，故按 sessionId 精确匹配，找不到即忽略 */
  function applyConnectionStatus(connection: ServerConnection) {
    const workspace = connectionWorkspaces.value.find(
      (item) => item.connection.sessionId === connection.sessionId
    )
    if (!workspace) return
    // 自动重连等待期忽略旧会话的 Disconnected 推送，避免覆盖 reconnecting 状态
    if (connection.status === 'disconnected' && workspace.connection.status === 'reconnecting') {
      return
    }
    workspace.connection = connection
  }

  /** 连接阶段进度：先按已绑定的 requestId 精确匹配；未绑定的占位工作区用「同 profile + 进行中」完成首绑 */
  function applyConnectStage(stage: ConnectStage) {
    let workspace = connectionWorkspaces.value.find(
      (item) => item.connectRequestId === stage.requestId
    )
    if (!workspace) {
      workspace = connectionWorkspaces.value.find(
        (item) =>
          item.profileId === stage.profileId &&
          !item.connectRequestId &&
          ['connecting', 'reconnecting'].includes(item.connection.status)
      )
      if (workspace) workspace.connectRequestId = stage.requestId
    }
    if (workspace) workspace.stageText = stageTextFor(stage.stage, stage.status)
  }

  return {
    connectionWorkspaces,
    activeProfileId,
    openConnection,
    closeConnectionWorkspace,
    closeAllWorkspaces,
    disconnectAll,
    disconnect,
    closeConnectionsOf,
    forgetActiveProfile,
    reconnectWorkspace,
    handleLinkDead,
    touchWorkspace,
    touchBySessionId,
    idleDisconnect,
    applyConnectionStatus,
    applyConnectStage,
  }
}
