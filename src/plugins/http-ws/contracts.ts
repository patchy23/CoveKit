/** 接口调试的权威传输契约，与 Rust http_ws/models 及 persistence/models 同步。 */
export type ApiKind = 'http' | 'sse' | 'ws'
export type HttpMethod = 'GET' | 'POST' | 'PUT' | 'PATCH' | 'DELETE' | 'HEAD' | 'OPTIONS'
export type BodyMode = 'none' | 'json' | 'text' | 'form'
export interface RequestAuth {
  mode: 'none' | 'basic' | 'bearer'
  credentialId: string
  /** 临时值仅参与当前请求，保存接口时移除。 */
  username?: string
  secret?: string
}
export interface HttpRequestPayload {
  method: HttpMethod
  url: string
  headers: [string, string][]
  body?: string
  timeoutMs?: number
  auth?: RequestAuth
}
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
export interface WsConnectPayload {
  url: string
  timeoutMs?: number
  headers?: [string, string][]
  auth?: RequestAuth
}
export interface WsMessage {
  seq: number
  direction: 'sent' | 'received'
  content: string
  time: number
}
export interface WsSession {
  id: string
  url: string
  connectedAt: number
  open: boolean
  messages: WsMessage[]
  dropped: number
  error?: string | null
}
export interface WsActionResult {
  ok: boolean
  message?: string
}
export interface SseEvent {
  event: string
  id: string
  data: string
  retry?: number | null
}
/** SSE 状态由有界解析器通过专属 Tauri Channel 推送，不保存响应历史。 */
export type SseUpdate =
  | { type: 'connected'; status: number; headers: [string, string][] }
  | { type: 'event'; event: SseEvent }
  | { type: 'closed' }
  | { type: 'error'; message: string }
export interface ApiRecord {
  id: number
  type: ApiKind
  name: string
  method: string
  url: string
  params: string
  headers: string
  bodyMode: string
  body: string
  groupName: string
  /** 仅包含模式与凭证引用，不包含临时秘密。 */
  options: string
  updatedAt: string
}
export interface ApiSavePayload {
  id?: number
  kind: ApiKind
  name: string
  method: string
  url: string
  params: string
  headers: string
  bodyMode: string
  body: string
  groupName: string
  options: string
}
export const commands = {
  apiGroupList: 'api_group_list',
  apiGroupCreate: 'api_group_create',
  httpRequest: 'http_request',
  apiSave: 'api_save',
  apiList: 'api_list',
  apiDelete: 'api_delete',
  wsConnect: 'ws_connect',
  wsSend: 'ws_send',
  wsRecv: 'ws_recv',
  wsClose: 'ws_close',
  wsSessions: 'ws_sessions',
  sseStart: 'sse_start',
  sseStop: 'sse_stop',
} as const
export type Payloads = {
  api_group_list: Record<string, never>
  api_group_create: { name: string; parent: string }
  http_request: { payload: HttpRequestPayload }
  api_save: ApiSavePayload
  api_list: Record<string, never>
  api_delete: { id: number }
  ws_connect: WsConnectPayload
  ws_send: { id: string; message: string }
  ws_recv: { id: string }
  ws_close: { id: string }
  ws_sessions: Record<string, never>
  sse_start: {
    id: string
    payload: HttpRequestPayload
    onEvent: import('@tauri-apps/api/core').Channel<SseUpdate>
  }
  sse_stop: { id: string }
}
export type Results = {
  api_group_list: string[]
  api_group_create: string
  http_request: HttpResponseResult
  api_save: number
  api_list: ApiRecord[]
  api_delete: void
  ws_connect: WsSession
  ws_send: WsActionResult
  ws_recv: WsSession
  ws_close: WsActionResult
  ws_sessions: WsSession[]
  sse_start: void
  sse_stop: void
}
