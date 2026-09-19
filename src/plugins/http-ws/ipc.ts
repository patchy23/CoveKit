/** 接口调试命令封装；Channel 随页签生命周期停止消费。 */
import { Channel } from '@tauri-apps/api/core'
import { invokeCommand } from '@/core/ipc/ipc'
import type {
  ApiSavePayload,
  HttpRequestPayload,
  Payloads,
  Results,
  SseUpdate,
  WsConnectPayload,
} from './contracts'
function call<K extends keyof Payloads & keyof Results>(command: K, payload: Payloads[K]) {
  return invokeCommand<Payloads[K], Results[K]>(command, payload)
}
export const ipc = {
  apiTreeMove: (payload: Payloads['api_tree_move']) => call('api_tree_move', payload),
  apiMoveGroup: (id: number, groupName: string) => call('api_move_group', { id, groupName }),
  apiGroupMove: (path: string, parent: string) => call('api_group_move', { path, parent }),
  apiGroupList: () => call('api_group_list', {}),
  apiGroupCreate: (name: string, parent: string) => call('api_group_create', { name, parent }),
  httpRequest: (payload: HttpRequestPayload) => call('http_request', { payload }),
  apiSave: (payload: ApiSavePayload) => call('api_save', payload),
  apiList: () => call('api_list', {}),
  apiDelete: (id: number) => call('api_delete', { id }),
  wsConnect: (payload: WsConnectPayload) => call('ws_connect', payload),
  wsSend: (id: string, message: string) => call('ws_send', { id, message }),
  wsRecv: (id: string) => call('ws_recv', { id }),
  wsClose: (id: string) => call('ws_close', { id }),
  wsSessions: () => call('ws_sessions', {}),
  sseStart: (id: string, payload: HttpRequestPayload, receive: (update: SseUpdate) => void) => {
    const onEvent = new Channel<SseUpdate>()
    onEvent.onmessage = receive
    return call('sse_start', { id, payload, onEvent })
  },
  sseStop: (id: string) => call('sse_stop', { id }),
}
