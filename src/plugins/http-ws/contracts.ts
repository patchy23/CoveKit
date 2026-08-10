/**
 * HTTP/WS 调试插件 · IPC 契约（本插件私有，独立于框架与其它插件）
 * 与 src-tauri/modules/http_ws.rs、modules/api.rs 的 serde 结构同步。
 */

export type HttpMethod = 'GET' | 'POST' | 'PUT' | 'PATCH' | 'DELETE' | 'HEAD' | 'OPTIONS'

/** HTTP 请求载荷 */
export interface HttpRequestPayload {
  method: HttpMethod
  url: string
  headers: [string, string][]
  body?: string
  timeoutMs?: number
}

/** HTTP 响应结果 */
export interface HttpResponseResult {
  ok: boolean
  status: number
  statusText: string
  headers: [string, string][]
  body: string
  bodySize: number
  durationMs: number
  error?: string
}

/** WebSocket 连接请求 */
export interface WsConnectPayload {
  url: string
  headers?: [string, string][]
}

/** WS 消息（方向 + 内容 + 时间） */
export interface WsMessage {
  direction: 'sent' | 'received'
  content: string
  time: number
}

/** WS 会话快照 */
export interface WsSession {
  id: string
  url: string
  connectedAt: number
  open: boolean
  messages: WsMessage[]
}

/** WS 会话操作结果 */
export interface WsActionResult {
  ok: boolean
  message?: string
}

/** 接口列表记录（Params/Headers 为 KvRow 的 JSON 字符串） */
export interface ApiRecord {
  id: number
  type: 'http' | 'ws'
  name: string
  method: string
  url: string
  params: string
  headers: string
  bodyMode: string
  body: string
  updatedAt: string
}

/** 命令清单（本插件命令的唯一出处） */
export const commands = {
  httpRequest: 'http_request',
  apiSave: 'api_save',
  apiList: 'api_list',
  apiDelete: 'api_delete',
  wsConnect: 'ws_connect',
  wsSend: 'ws_send',
  wsRecv: 'ws_recv',
  wsClose: 'ws_close',
  wsSessions: 'ws_sessions',
} as const

/** 命令入参 */
export type Payloads = {
  http_request: { payload: HttpRequestPayload }
  api_save: {
    id?: number
    kind: 'http' | 'ws'
    name: string
    method: string
    url: string
    params: string
    headers: string
    bodyMode: string
    body: string
  }
  api_list: Record<string, never>
  api_delete: { id: number }
  ws_connect: WsConnectPayload
  ws_send: { id: string; message: string }
  ws_recv: { id: string }
  ws_close: { id: string }
  ws_sessions: Record<string, never>
}

/** 命令返回 */
export type Results = {
  http_request: HttpResponseResult
  api_save: number
  api_list: ApiRecord[]
  api_delete: void
  ws_connect: WsSession
  ws_send: WsActionResult
  ws_recv: WsSession
  ws_close: WsActionResult
  ws_sessions: WsSession[]
}
