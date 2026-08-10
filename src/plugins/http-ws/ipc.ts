/**
 * HTTP/WS 调试插件 · IPC 封装（本插件命令，独立于框架）
 */
import { invokeCommand } from '@/core/ipc/ipc'
import type {
  ApiRecord,
  HttpRequestPayload,
  HttpResponseResult,
  WsActionResult,
  WsConnectPayload,
  WsSession,
} from './contracts'

export const ipc = {
  httpRequest: (payload: HttpRequestPayload): Promise<HttpResponseResult> =>
    invokeCommand('http_request', { payload }),
  apiSave: (r: {
    id?: number
    kind: 'http' | 'ws'
    name: string
    method: string
    url: string
    params: string
    headers: string
    bodyMode: string
    body: string
  }): Promise<number> => invokeCommand('api_save', r),
  apiList: (): Promise<ApiRecord[]> => invokeCommand('api_list', {}),
  apiDelete: (id: number): Promise<void> => invokeCommand('api_delete', { id }),
  wsConnect: (payload: WsConnectPayload): Promise<WsSession> =>
    invokeCommand('ws_connect', payload),
  wsSend: (id: string, message: string): Promise<WsActionResult> =>
    invokeCommand('ws_send', { id, message }),
  wsRecv: (id: string): Promise<WsSession> => invokeCommand('ws_recv', { id }),
  wsClose: (id: string): Promise<WsActionResult> => invokeCommand('ws_close', { id }),
  wsSessions: (): Promise<WsSession[]> => invokeCommand('ws_sessions', {}),
}
