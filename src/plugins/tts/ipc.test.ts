import { expect, it, vi } from 'vitest'
import { invokeCommand } from '@/core/ipc/ipc'
import { ipc } from './ipc'

vi.mock('@/core/ipc/ipc', () => ({ invokeCommand: vi.fn() }))
vi.mock('@tauri-apps/api/core', () => ({
  Channel: class {
    onmessage?: (value: unknown) => void
  },
}))

it('登记、合成、取消使用同一 jobId 和 camelCase 命令入参', async () => {
  vi.mocked(invokeCommand).mockResolvedValueOnce('owned-job')
  const jobId = await ipc.ttsPrepare()
  const payload = { jobId, text: '中文', voice: 'voice', rate: 0, pitch: 0 }
  await ipc.ttsSynthesize(payload)
  await ipc.ttsCancel(jobId)
  await ipc.ttsDiscard(jobId)
  expect(invokeCommand).toHaveBeenNthCalledWith(1, 'tts_prepare', {})
  expect(invokeCommand).toHaveBeenNthCalledWith(2, 'tts_synthesize', payload)
  expect(invokeCommand).toHaveBeenNthCalledWith(3, 'tts_cancel', { jobId })
  expect(invokeCommand).toHaveBeenNthCalledWith(4, 'tts_discard', { jobId })
})

it('进展使用请求专属 Channel，命令结束后释放消费回调', async () => {
  let resolve!: (value: unknown) => void
  vi.mocked(invokeCommand).mockReturnValueOnce(
    new Promise((r) => {
      resolve = r
    })
  )
  const receive = vi.fn()
  const result = ipc.ttsSynthesize({ jobId: 'progress-job', text: '中文', voice: 'voice' }, receive)
  const payload = vi.mocked(invokeCommand).mock.lastCall![1] as {
    jobId: string
    onProgress: { onmessage: (value: unknown) => void }
  }
  const update = {
    jobId: 'progress-job',
    sequence: 1,
    phase: 'connecting',
    lastProgressAt: 1,
    bytes: 0,
  }
  payload.onProgress.onmessage(update)
  expect(receive).toHaveBeenCalledWith(update)
  resolve({ ok: false, bytes: 0 })
  await result
  payload.onProgress.onmessage(update)
  expect(receive).toHaveBeenCalledOnce()
})
