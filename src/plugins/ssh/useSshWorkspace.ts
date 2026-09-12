/**
 * useSshWorkspace · SSH 主工作区组合门面
 *
 * 组装根：把四个能力域拼成界面唯一入口，自身不持有业务状态。
 * - 服务器配置与分组：`profiles/useSshProfiles`
 * - 连接页签与会话状态机：`connection/useSshConnections`
 * - 主机密钥确认队列：`connection/useHostKeyQueue`
 * - 事件订阅、空闲回收与卸载编排：`workspace/useSshLifecycle`
 *
 * 域间只经端口交互：配置域删除服务器时回调连接域的 `closeConnectionsOf` / `forgetActiveProfile`，
 * 连接域只读配置（`findProfile`）并在连上后回调 `onConnected`，卸载标记由生命周期域唯一写入。
 * 返回面与拆分前保持一致，`index.vue` 与各子组件无需改动。
 */
import { onMounted } from 'vue'
import { useHostKeyQueue } from './connection/useHostKeyQueue'
import { useSshConnections } from './connection/useSshConnections'
import { ipc } from './ipc'
import { useSshProfiles } from './profiles/useSshProfiles'
import { useSshLifecycle } from './workspace/useSshLifecycle'

export type { SshConnectionWorkspace, SshWorkspaceSection } from './connection/useSshConnections'

/**
 * 创建 SSH 主工作区的状态与操作。每个连接页签独占一条 SSH 连接。
 * 端口均为晚绑定回调（只在挂载后的事件与用户操作中执行），因此域之间可互相引用。
 */
export function useSshWorkspace() {
  /* ── 域组装（配置域 → 连接域 → 主机密钥域 → 生命周期域） ── */

  const profilesApi = useSshProfiles({
    /** 删除服务器前关闭其名下工作区并断开会话 */
    closeConnectionsOf: (profileId) => connections.closeConnectionsOf(profileId),
    /** 服务器被删除后清掉指向它的活动页签 */
    forgetActiveProfile: (profileId) => connections.forgetActiveProfile(profileId),
  })

  const connections = useSshConnections({
    /** 占位标题与删除归属判断只读配置列表 */
    findProfile: (profileId) => profilesApi.profiles.value.find((item) => item.id === profileId),
    /** 连接成功：刷新该服务器的最近连接时间 */
    onConnected: (profileId) => profilesApi.touchProfileConnected(profileId),
    /** 卸载标记由生命周期域持有 */
    isDisposed: () => lifecycle.isDisposed(),
  })

  const hostKeys = useHostKeyQueue()

  const lifecycle = useSshLifecycle({
    onConnectionStatusEvent: connections.applyConnectionStatus,
    onConnectStageEvent: connections.applyConnectStage,
    onTerminalClosedEvent: connections.applyTerminalClosed,
    onHostKeyEnqueue: hostKeys.enqueue,
    onActivity: connections.touchBySessionId,
    onIdleExpired: connections.idleDisconnect,
    listWorkspaces: () => connections.connectionWorkspaces.value,
    onDispose: () => {
      // 积压的主机密钥确认统一取消，避免后端握手回调挂到超时；随后断开全部会话
      hostKeys.cancelAll()
      void connections.disconnectAll()
    },
  })

  onMounted(async () => {
    // localStorage 存量配置/分组一次性迁入后端插件库（旧手工凭证由后端迁入 Vault）
    await profilesApi.importLegacyOnce()
    await Promise.all([profilesApi.loadProfiles(), profilesApi.loadGroups()])
    // 不恢复上一次工具实例遗留的后端会话；重新打开 SSH 工具永远从空状态开始。
    try {
      const stale = await ipc.sshConnections()
      await Promise.all(stale.map((connection) => connections.disconnect(connection)))
    } catch {
      /* 浏览器预览没有 Tauri IPC。 */
    }
  })

  /* ── 对界面暴露的公开面（与拆分前逐一对应） ── */

  return {
    /* 配置与分组 */
    profiles: profilesApi.profiles,
    groups: profilesApi.groups,
    expandedIds: profilesApi.expandedIds,
    searchKeyword: profilesApi.searchKeyword,
    filteredProfiles: profilesApi.filteredProfiles,
    formOpen: profilesApi.formOpen,
    editingProfile: profilesApi.editingProfile,
    deleteTarget: profilesApi.deleteTarget,
    openAddForm: profilesApi.openAddForm,
    openEditForm: profilesApi.openEditForm,
    showError: profilesApi.showError,
    saveProfile: profilesApi.saveProfile,
    requestDelete: profilesApi.requestDelete,
    confirmDelete: profilesApi.confirmDelete,
    createGroup: profilesApi.createGroup,
    renameGroup: profilesApi.renameGroup,
    deleteGroup: profilesApi.deleteGroup,
    moveToGroup: profilesApi.moveToGroup,
    toggleGroup: profilesApi.toggleGroup,
    /* 连接工作区 */
    connectionWorkspaces: connections.connectionWorkspaces,
    activeProfileId: connections.activeProfileId,
    openConnection: connections.openConnection,
    closeConnectionWorkspace: connections.closeConnectionWorkspace,
    closeAllWorkspaces: connections.closeAllWorkspaces,
    reconnectWorkspace: connections.reconnectWorkspace,
    handleLinkDead: connections.handleLinkDead,
    touchWorkspace: connections.touchWorkspace,
    /* 主机密钥确认 */
    hostKeyRequest: hostKeys.hostKeyRequest,
    respondHostKey: hostKeys.respondHostKey,
  }
}
