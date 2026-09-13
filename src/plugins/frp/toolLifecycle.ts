/**
 * FRP 工具的资源生命周期（可靠性 T10-4）
 *
 * 关闭策略（本插件自定）：**关闭工具页签会停止由本工具启动的 frpc 进程**。
 * v1 边界就是「不常驻」——页签关了进程还留着，用户既看不到也管不了。
 * 仍有进程在跑时先询问；停止失败的档案逐条报给用户，不假装停掉了。
 *
 * 后台常驻属于明确未实现范围（需要托盘常驻与进程守护），不在这里偷偷做。
 */
import { ipc } from './ipc'
import { useToolLifecycle } from '@/core/lifecycle'

/** 仍在跑或正在启动的状态（关闭页签时需要停止、界面按此显示运行标记） */
const LIVE_STATES = ['running', 'starting']

/** 该档案是否仍占用进程（关闭协商与运行标记共用同一判定，避免两处口径不一致） */
export function isFrpLive(state: string): boolean {
  return LIVE_STATES.includes(state)
}

export function useFrpToolLifecycle() {
  useToolLifecycle('frp', {
    owner: 'frp.processes',
    prepare: async () => {
      const states = await ipc.status()
      const live = states.filter((item) => isFrpLive(item.state))
      if (live.length === 0) return null
      return `${live.length} 个 frpc 进程正在运行，关闭后会被停止`
    },
    dispose: async () => {
      const states = await ipc.status()
      const failures: string[] = []
      for (const item of states) {
        if (!isFrpLive(item.state)) continue
        try {
          await ipc.stop(item.fileName)
        } catch (error) {
          failures.push(
            `${item.fileName}：${error instanceof Error ? error.message : String(error)}`
          )
        }
      }
      if (failures.length > 0) throw new Error(`停止失败（${failures.join('；')}）`)
    },
  })
}
