/** 文件占用查询的类型化命令入口。 */
import { invokeCommand } from '@/core/ipc/ipc'
import { commands, type Payloads, type Results } from './contracts'

function call<K extends keyof Payloads & keyof Results>(command: K, payload: Payloads[K]) {
  return invokeCommand<Payloads[K], Results[K]>(command, payload)
}

export const ipc = {
  supported: () => call(commands.supported, {}),
  query: (path: string) => call(commands.query, { path }),
}
