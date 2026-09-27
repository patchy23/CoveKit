import { computed, ref, shallowRef } from 'vue'
import type { TtsPhase, TtsProgress } from './contracts'

const labels: Record<TtsPhase, string> = {
  connecting: '正在连接语音服务',
  sending: '正在发送合成请求',
  receiving: '正在接收音频',
  writing: '正在写入音频',
  publishing: '正在保存音频',
}

/** 停滞只是提示；计时器不取消任务、不清空已呈现的音频。 */
export function useTtsProgress() {
  const current = shallowRef<TtsProgress>()
  const stalled = ref(false)
  let timer: ReturnType<typeof setTimeout> | undefined
  let acknowledgedAt = 0
  function schedule() {
    clearTimeout(timer)
    const progress = current.value
    if (!progress) return
    const duration = progress.phase === 'connecting' ? 30_000 : 120_000
    const due = Math.max(progress.lastProgressAt, acknowledgedAt) + duration
    timer = setTimeout(
      () => {
        stalled.value = true
      },
      Math.max(0, due - Date.now())
    )
  }
  function stop() {
    clearTimeout(timer)
    timer = undefined
    current.value = undefined
    stalled.value = false
    acknowledgedAt = 0
  }
  function start(jobId: string) {
    stop()
    current.value = {
      jobId,
      sequence: 0,
      phase: 'connecting',
      lastProgressAt: Date.now(),
      bytes: 0,
    }
    schedule()
  }
  function receive(progress: TtsProgress) {
    if (progress.jobId !== current.value?.jobId || progress.sequence <= current.value.sequence)
      return
    current.value = progress
    stalled.value = false
    acknowledgedAt = 0
    schedule()
  }
  function keepWaiting() {
    acknowledgedAt = Date.now()
    stalled.value = false
    schedule()
  }
  return {
    current,
    stalled,
    start,
    stop,
    receive,
    keepWaiting,
    label: computed(() => (current.value ? labels[current.value.phase] : '')),
  }
}
