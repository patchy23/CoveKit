/**
 * useSshIdleWatch · 空闲自动断开与活动监听（自 useSshWorkspace 拆出，300 行红线）
 * 终端输出/传输进度视为会话活跃；断开/重连/阶段/主机密钥事件的订阅也集中在此，
 * 回调由调用方注入（工作区状态机保持纯粹）。
 */
import { onMounted, onUnmounted } from 'vue'
import { useSettingsStore } from '@/stores/settings'
import {
  onConnectionStatus,
  onConnectStage,
  onHostKeyVerify,
  onTerminalClosed,
  onTerminalData,
  onTransferProgress,
} from '../ipc'
import type { HostKeyVerifyRequest, ServerConnection } from '../contracts'

export interface SshWatchCallbacks {
  /** 当前全部连接工作区（浅拷贝读取） */
  workspaces: () => Array<{
    id: string
    profileId: string
    connection: ServerConnection
    lastActivityAt: number
  }>
  /** 按 sessionId 找工作区 id */
  findIdBySession: (sessionId: string) => string | null
  /** 连接状态事件 */
  onConnectionStatusEvent: (connection: ServerConnection) => void
  /** 意外断线（主终端通道异常关闭） */
  onLinkDead: (workspaceId: string) => void
  /** 后台活动（终端输出/传输进度） */
  onActivityTouch: (connectionId: string) => void
  /** 终端通道关闭（可能为意外断线） */
  onTerminalClosedUnexpected: (connectionId: string) => void
  /** 主机密钥确认请求入队 */
  onHostKeyEnqueue: (request: HostKeyVerifyRequest) => void
  /** 阶段进度 */
  onConnectStage: (requestId: string, stage: string, status: string) => void
  /** 断开某会话（清理 stale 用） */
  disconnect: (connection: ServerConnection) => Promise<void>
  /** 空闲断开命中 */
  onIdleExpired: (workspaceId: string, minutes: number) => void
}

/** 后台活动节流间隔（5s 内不重复刷新） */
const ACTIVITY_TOUCH_THROTTLE = 5_000

export function useSshIdleWatch(callbacks: SshWatchCallbacks) {
  const settings = useSettingsStore()
  let unlistenConnection: (() => void) | null = null
  let unlistenActivity: (() => void) | null = null
  let unlistenTransfer: (() => void) | null = null
  let unlistenClosed: (() => void) | null = null
  let unlistenStage: (() => void) | null = null
  let unlistenHostKey: (() => void) | null = null
  let idleTimer: ReturnType<typeof setInterval> | null = null
  let disposed = false
  const lastActivityTouch = new Map<string, number>()

  function touchByConnectionId(connectionId: string) {
    const now = Date.now()
    if (now - (lastActivityTouch.get(connectionId) ?? 0) < ACTIVITY_TOUCH_THROTTLE) return
    lastActivityTouch.set(connectionId, now)
    callbacks.onActivityTouch(connectionId)
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
    try {
      const stop = await onConnectionStatus((connection) => {
        if (!disposed) callbacks.onConnectionStatusEvent(connection)
      })
      if (disposed) stop()
      else unlistenConnection = stop
    } catch {
      /* 浏览器预览没有 Tauri 事件系统。 */
    }
    try {
      const stopData = await onTerminalData((d) => touchByConnectionId(d.connectionId))
      const stopTransfer = await onTransferProgress((p) => touchByConnectionId(p.connectionId))
      const stopClosed = await onTerminalClosed((d) => {
        if (!disposed) callbacks.onTerminalClosedUnexpected(d.connectionId)
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
        if (!disposed) callbacks.onConnectStage(s.requestId, s.stage, s.status)
      })
      const stopHostKey = await onHostKeyVerify((request) => {
        if (!disposed) callbacks.onHostKeyEnqueue(request)
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
      const expired = callbacks
        .workspaces()
        .filter(
          (workspace) =>
            workspace.connection.status === 'connected' && workspace.lastActivityAt < cutoff
        )
      for (const workspace of expired) {
        void callbacks.onIdleExpired(workspace.id, minutes)
      }
    }, 30_000)
  })

  onUnmounted(() => {
    disposed = true
    unlistenConnection?.()
    unlistenActivity?.()
    unlistenTransfer?.()
    unlistenClosed?.()
    unlistenStage?.()
    unlistenHostKey?.()
    if (idleTimer) clearInterval(idleTimer)
  })

  return {
    /** 测试钩子：供单测验证 disposed 语义 */
    isDisposed: () => disposed,
  }
}
