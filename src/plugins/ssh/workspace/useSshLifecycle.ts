/**
 * useSshLifecycle · SSH 会话生命周期与空闲回收所有者
 *
 * 职责：注册五路后端事件订阅（连接状态、终端输出、传输进度、连接阶段、主机密钥确认）、
 * 卸载时的退订与清理编排（含「卸载先于订阅完成」时晚到的退订句柄）、空闲断开定时器与后台活动节流。
 * 事件到状态的映射逻辑不在本域：回调由组装根注入，本域只负责订阅生命周期与时间驱动。
 *
 * 卸载标记由本域唯一写入（{@link useSshLifecycle} 返回的 isDisposed），连接域只读，
 * 用于丢弃卸载后晚到的连接结果。
 */
import { onMounted, onUnmounted, watch } from 'vue'
import { isTauri } from '@tauri-apps/api/core'
import { useSettingsStore } from '@/stores/settings'
import { useUiStore } from '@/stores/ui'
import {
  onConnectionStatus,
  onConnectStage,
  onHostKeyVerify,
  onTerminalData,
  onTransferProgress,
} from '../ipc'
import type { ConnectStage, HostKeyVerifyRequest, ServerConnection } from '../contracts'

/** 组装根注入的事件回调与清理动作（生命周期域不直接持有工作区状态） */
export interface SshLifecyclePorts {
  /** 连接状态推送 */
  onConnectionStatusEvent: (connection: ServerConnection) => void
  /** 连接阶段进度 */
  onConnectStageEvent: (stage: ConnectStage) => void
  /** 主机密钥确认请求入队 */
  onHostKeyEnqueue: (request: HostKeyVerifyRequest) => void
  /** 后台活动（终端输出含输入回显、传输进度）刷新指定会话活跃时间 */
  onActivity: (connectionId: string) => void
  /** 空闲超时：断开指定工作区 */
  onIdleExpired: (workspaceId: string, minutes: number) => void
  /** 当前连接工作区快照（空闲扫描只读） */
  listWorkspaces: () => Array<{
    id: string
    connection: ServerConnection
    lastActivityAt: number
  }>
  /** 卸载清理：取消积压确认请求、断开全部会话 */
  onDispose: () => void
}

/** 后台活动节流间隔：同一会话 5s 内不重复刷新活跃时间 */
const ACTIVITY_TOUCH_THROTTLE = 5_000

/** 空闲扫描周期（分钟阈值由设置决定，扫描固定 30s 一次） */
const IDLE_SCAN_INTERVAL_MS = 30_000

export function useSshLifecycle(ports: SshLifecyclePorts) {
  const settings = useSettingsStore()
  const ui = useUiStore()
  const unlisteners = new Set<() => void>()
  let idleTimer: ReturnType<typeof setInterval> | null = null
  let disposed = false
  /** 后台活动节流表（connectionId → 上次活动时间戳） */
  const lastActivityTouch = new Map<string, number>()
  let connectionIds = new Set<string>()
  // 只观察会话身份，输出刷新活跃时间不会触发全表重建；关闭后晚到事件不能重新填回。
  watch(
    () => ports.listWorkspaces().map((workspace) => workspace.connection.sessionId),
    (ids) => {
      connectionIds = new Set(ids)
      for (const id of lastActivityTouch.keys()) {
        if (!connectionIds.has(id)) lastActivityTouch.delete(id)
      }
    },
    { immediate: true, flush: 'sync' }
  )

  function release(stop: () => void) {
    try {
      stop()
    } catch (error) {
      ui.toast(`SSH 事件退订失败：${String(error)}`)
    }
  }

  /** 每条订阅独立取得并立即接管，下一条失败不会丢失此前的释放责任。 */
  async function subscribe(label: string, register: () => Promise<() => void>) {
    if (disposed) return
    try {
      const stop = await register()
      if (disposed) release(stop)
      else unlisteners.add(stop)
    } catch (error) {
      if (!disposed && isTauri()) ui.toast(`SSH ${label}订阅失败：${String(error)}`)
    }
  }

  /** 后台活动也算会话活跃——防止长任务、看日志、传文件时被空闲断开误杀 */
  function touchByConnectionId(connectionId: string) {
    if (disposed || !connectionIds.has(connectionId)) return
    const now = Date.now()
    if (now - (lastActivityTouch.get(connectionId) ?? 0) < ACTIVITY_TOUCH_THROTTLE) return
    lastActivityTouch.set(connectionId, now)
    ports.onActivity(connectionId)
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
    await subscribe('连接状态', () =>
      onConnectionStatus((connection) => {
        if (!disposed) ports.onConnectionStatusEvent(connection)
      })
    )
    // 通道关闭由 TerminalTab 按终端 ID 与用途判断并上报 linkDead；不能据连接 ID 将容器退出视为断线。
    // 后台活动监听：终端有输出（含打字回显）/ 传输在进行 → 刷新对应工作区活跃时间
    await subscribe('终端活动', () => onTerminalData((d) => touchByConnectionId(d.connectionId)))
    await subscribe('传输活动', () =>
      onTransferProgress((p) => touchByConnectionId(p.connectionId))
    )
    await subscribe('连接阶段', () =>
      onConnectStage((stage) => {
        if (!disposed) ports.onConnectStageEvent(stage)
      })
    )
    await subscribe('主机密钥', () =>
      onHostKeyVerify((request) => {
        if (!disposed) ports.onHostKeyEnqueue(request)
      })
    )

    if (disposed) return
    idleTimer = setInterval(() => {
      const minutes = Number(settings.getToolSetting('ssh', 'idleDisconnectMinutes', '10'))
      if (!Number.isFinite(minutes) || minutes <= 0) return
      const cutoff = Date.now() - minutes * 60_000
      const expired = ports
        .listWorkspaces()
        .filter(
          (workspace) =>
            workspace.connection.status === 'connected' && workspace.lastActivityAt < cutoff
        )
      for (const workspace of expired) ports.onIdleExpired(workspace.id, minutes)
    }, IDLE_SCAN_INTERVAL_MS)
  })

  onUnmounted(() => {
    disposed = true
    for (const stop of unlisteners) release(stop)
    unlisteners.clear()
    lastActivityTouch.clear()
    connectionIds.clear()
    if (idleTimer) clearInterval(idleTimer)
    ports.onDispose()
  })

  return {
    /** 组件是否已卸载：连接域据此丢弃晚到的连接结果（本域是唯一写入方） */
    isDisposed: () => disposed,
  }
}
