/**
 * database 连接域：连接定义列表、连接/断开、当前选中连接与请求取消标记（epoch）。
 *
 * 本域拥有 connections/activeConnectionId/connecting/connectError/cancelledConnect 五份状态，
 * 对外只给只读快照（connectionList/activeConnectionIdView/activeConnection）与命令函数：
 * 连接成功后的树展开与元数据预取、断开/删除后的元数据失效、删除连接时关闭其页签，
 * 一律经端口委托给 catalog / workspace 域，本域不直读它们的内部状态。
 */
import { computed, onScopeDispose, ref } from 'vue'
import type { ConnConfig, DbConnectionInfo, DriverInstallProgress } from '../contracts'
import { connectionIpc } from '../ipc'
import {
  isOracleDriverInstallCancelled,
  startOracleDriverInstall,
  type OracleDriverInstallScope,
} from './useOracleDriver'

export interface OracleDriverInstallView {
  phase: DriverInstallProgress['phase'] | 'checking'
  downloadedBytes: number
  totalBytes?: number
}

interface ConnectOperation {
  cancelled: boolean
  install?: OracleDriverInstallScope
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error)
}

/** 给 Promise 加超时兜底（后端挂起时前端也能报错收尾） */
export function withTimeout<T>(promise: Promise<T>, ms: number, message: string): Promise<T> {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error(message)), ms)
    promise.then(
      (v) => {
        clearTimeout(timer)
        resolve(v)
      },
      (e) => {
        clearTimeout(timer)
        reject(e)
      }
    )
  })
}

/** 连接域的跨域协作端口（由根门面注入） */
export interface DatabaseConnectionsPorts {
  /** 连接成功：展开该连接节点并预取库/schema 列表（catalog 域） */
  prefetchCatalog: (connId: string) => void
  /** 断开/删除连接：失效该连接的元数据缓存（catalog 域） */
  invalidateCatalogMeta: (connId: string) => void
  /** 删除连接：关闭该连接下的全部页签与页签状态（workspace 域） */
  closeConnectionTabs: (connectionId: string) => void
  /** 应用级错误提示（门面持有 errorHint） */
  showError: (err: unknown) => void
}

export function useDatabaseConnections(ports: DatabaseConnectionsPorts) {
  const connections = ref<DbConnectionInfo[]>([])
  const activeConnectionId = ref('')
  const connecting = ref<Record<string, boolean>>({})
  const connectError = ref<Record<string, string>>({})
  /** 连接取消标记（cancelConnect 置位；连接结果返回后据此丢弃并断开），即一次连接的请求 epoch */
  const cancelledConnect = ref<Record<string, boolean>>({})
  const oracleDriverInstallState = ref<Record<string, OracleDriverInstallView>>({})
  const connectOperations = new Map<string, ConnectOperation>()

  // ──────────────────────────────────────────────────────────────────────
  // 派生与只读视图
  // ──────────────────────────────────────────────────────────────────────

  const activeConnection = computed<DbConnectionInfo | undefined>(() =>
    connections.value.find((c) => c.id === activeConnectionId.value)
  )

  const connectionOptions = computed(() =>
    connections.value.map((c) => ({ value: c.id, label: c.label }))
  )

  /** 只读连接快照：catalog / workspace 只读，写入仍只经本域命令 */
  const connectionList = computed<readonly DbConnectionInfo[]>(() => connections.value)

  /** 只读「当前选中连接 id」视图：workspace 页签上下文回退用 */
  const activeConnectionIdView = computed(() => activeConnectionId.value)

  /** Oracle 驱动安装进度只读视图，供连接列表展示；所有权留在连接域。 */
  const oracleDriverInstall = computed<Readonly<Record<string, OracleDriverInstallView>>>(
    () => oracleDriverInstallState.value
  )

  // ──────────────────────────────────────────────────────────────────────
  // 供 catalog 树交互使用的窄命令
  // ──────────────────────────────────────────────────────────────────────

  /** 记录连接失败/不支持原因（达梦等，树交互写入 connectError） */
  function setConnectError(connId: string, message: string) {
    connectError.value[connId] = message
  }

  /** 选中连接（连接不存在时保持原选择不变） */
  function activateConnection(connId: string) {
    const conn = connections.value.find((c) => c.id === connId)
    if (conn) activeConnectionId.value = connId
  }

  // ──────────────────────────────────────────────────────────────────────
  // 连接管理
  // ──────────────────────────────────────────────────────────────────────

  async function refreshConnections() {
    try {
      connections.value = await connectionIpc.list()
      if (!activeConnectionId.value && connections.value.length) {
        activeConnectionId.value = connections.value[0].id
      }
    } catch (err) {
      ports.showError(err)
    }
  }

  async function connect(conn: DbConnectionInfo) {
    if (connecting.value[conn.id]) return
    const operation: ConnectOperation = { cancelled: false }
    connectOperations.set(conn.id, operation)
    connecting.value[conn.id] = true
    connectError.value[conn.id] = ''
    cancelledConnect.value[conn.id] = false
    try {
      if (conn.dbType === 'oracle') {
        oracleDriverInstallState.value[conn.id] = {
          phase: 'checking',
          downloadedBytes: 0,
        }
        const status = await connectionIpc.driverStatus('oracle')
        if (operation.cancelled) return
        if (!status.ready) {
          const install = startOracleDriverInstall({
            onProgress: (progress) => {
              if (connectOperations.get(conn.id) !== operation || operation.cancelled) return
              oracleDriverInstallState.value[conn.id] = {
                phase: progress.phase,
                downloadedBytes: progress.downloadedBytes,
                ...(progress.totalBytes === undefined ? {} : { totalBytes: progress.totalBytes }),
              }
            },
            onError: (error) => {
              ports.showError(error)
            },
          })
          operation.install = install
          try {
            const installed = await install.promise
            if (operation.cancelled) return
            if (!installed.ready)
              throw new Error(installed.note || 'Oracle agent 驱动安装后仍不可用。')
          } catch (error) {
            if (operation.cancelled || isOracleDriverInstallCancelled(error)) return
            throw error
          } finally {
            operation.install = undefined
            delete oracleDriverInstallState.value[conn.id]
          }
        } else {
          delete oracleDriverInstallState.value[conn.id]
        }
        if (operation.cancelled) return
      }
      // 超时由后端实际建连生命周期收尾，不能只丢弃 IPC Promise 留下迟到会话。
      const info = await connectionIpc.connect(conn.id)
      // 驱动准备或连接过程中被取消：立即断开，避免留下幽灵会话。
      if (operation.cancelled || cancelledConnect.value[conn.id]) {
        cancelledConnect.value[conn.id] = false
        try {
          await connectionIpc.disconnect(conn.id)
        } catch (error) {
          ports.showError(error)
        }
        return
      }
      const index = connections.value.findIndex((c) => c.id === info.id)
      if (index >= 0) connections.value[index] = info
      activeConnectionId.value = info.id
      // 连接成功后展开树节点并预取库/schema 列表（catalog 域）
      ports.prefetchCatalog(info.id)
    } catch (err) {
      const wasCancelled = operation.cancelled || cancelledConnect.value[conn.id]
      cancelledConnect.value[conn.id] = false
      if (wasCancelled) return
      connectError.value[conn.id] = errorMessage(err)
      const index = connections.value.findIndex((c) => c.id === conn.id)
      if (index >= 0)
        connections.value[index] = { ...conn, status: 'offline', error: errorMessage(err) }
      throw err
    } finally {
      if (connectOperations.get(conn.id) === operation) connectOperations.delete(conn.id)
      delete oracleDriverInstallState.value[conn.id]
      connecting.value[conn.id] = false
    }
  }

  /** 取消进行中的连接；完成清理前保持忙碌，避免迟到断开误伤重连。 */
  function cancelConnect(connId: string) {
    const operation = connectOperations.get(connId)
    if (operation) {
      operation.cancelled = true
      operation.install?.cancel()
    }
    cancelledConnect.value[connId] = true
    connectError.value[connId] = ''
    delete oracleDriverInstallState.value[connId]
  }

  async function disconnect(conn: DbConnectionInfo) {
    if (connectOperations.has(conn.id)) cancelConnect(conn.id)
    try {
      await connectionIpc.disconnect(conn.id)
    } catch (error) {
      ports.showError(error)
      await refreshConnections()
      return
    }
    const index = connections.value.findIndex((c) => c.id === conn.id)
    if (index >= 0)
      connections.value[index] = {
        ...conn,
        status: 'offline',
        version: '',
        latencyMs: 0,
        connectedAt: 0,
      }
    // 元数据缓存随断开失效（catalog 域）
    ports.invalidateCatalogMeta(conn.id)
  }

  /** 保存连接配置（可选保存后立即连接） */
  async function saveConnection(config: ConnConfig, password: string, connectAfter = true) {
    await connectionIpc.save(config, password)
    ports.invalidateCatalogMeta(config.id)
    await refreshConnections()
    if (connectAfter) {
      const conn = connections.value.find((c) => c.id === config.id)
      if (conn) await connect(conn)
    }
  }

  async function removeConnection(id: string) {
    if (connectOperations.has(id)) cancelConnect(id)
    // 删除命令自身负责取消与断开；错误必须由调用方展示。
    await connectionIpc.remove(id)
    // 关闭该连接下的页签（workspace 域）并失效元数据（catalog 域）
    ports.closeConnectionTabs(id)
    ports.invalidateCatalogMeta(id)
    await refreshConnections()
    if (!connections.value.some((c) => c.id === activeConnectionId.value)) {
      activeConnectionId.value = connections.value[0]?.id ?? ''
    }
  }

  onScopeDispose(() => {
    for (const operation of connectOperations.values()) {
      operation.cancelled = true
      operation.install?.cancel()
    }
  })

  return {
    // 状态
    connections,
    activeConnectionId,
    connecting,
    connectError,
    // 派生
    activeConnection,
    connectionOptions,
    // 跨域只读视图
    connectionList,
    activeConnectionIdView,
    oracleDriverInstall,
    // 供 catalog 的窄命令
    setConnectError,
    activateConnection,
    // 命令
    connect,
    cancelConnect,
    disconnect,
    saveConnection,
    removeConnection,
    refreshConnections,
  }
}
