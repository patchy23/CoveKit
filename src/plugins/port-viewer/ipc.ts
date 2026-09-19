/** 端口工具的类型化 IPC 入口。 */
import { invokeCommand } from '@/core/ipc/ipc'
import { commands, type Payloads, type Results } from './contracts'

function call<K extends keyof Payloads & keyof Results>(command: K, payload: Payloads[K]) {
  return invokeCommand<Payloads[K], Results[K]>(command, payload)
}
export const ipc = {
  supported: () => call(commands.supported, {}),
  query: () => call(commands.query, {}),
  terminate: (payload: Payloads['port_viewer_terminate']) => call(commands.terminate, payload),
}
