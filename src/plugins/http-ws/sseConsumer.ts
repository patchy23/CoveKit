import { nextTick } from 'vue'
import type { SseUpdate } from './contracts'

/** 同轮状态更新完成后合并确认，只允许一个确认请求在途，不另设刷新定时器。 */
export function sseConsumer(
  receive: (update: SseUpdate) => void,
  acknowledge: (sequence: number) => Promise<void>,
  stop: () => Promise<void>
) {
  let pending = 0
  let acknowledged = 0
  let running = false
  let ended = false

  async function flush() {
    if (running || ended) return
    running = true
    try {
      while (!ended && pending > acknowledged) {
        await nextTick()
        if (ended) return
        const sequence = pending
        await acknowledge(sequence)
        acknowledged = sequence
      }
    } catch (error) {
      ended = true
      receive({ type: 'error', message: `SSE 消费确认失败：${String(error)}` })
      try {
        await stop()
      } catch (stopError) {
        receive({
          type: 'error',
          message: `SSE 消费确认失败：${String(error)}；停止连接失败：${String(stopError)}`,
        })
      }
    } finally {
      running = false
    }
  }

  return (update: SseUpdate) => {
    if (ended) return
    if (update.type === 'closed' || update.type === 'error') ended = true
    if (update.type !== 'events') {
      receive(update)
      return
    }
    for (const event of update.events) receive({ type: 'event', event })
    pending = Math.max(pending, update.sequence)
    void flush()
  }
}
