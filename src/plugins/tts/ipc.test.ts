import { expect, it, vi } from 'vitest'
import { invokeCommand } from '@/core/ipc/ipc'
import { ipc } from './ipc'

vi.mock('@/core/ipc/ipc', () => ({ invokeCommand: vi.fn() }))

it('登记、合成、取消使用同一 jobId 和 camelCase 命令入参', async () => {
  vi.mocked(invokeCommand).mockResolvedValueOnce('owned-job')
  const jobId = await ipc.ttsPrepare()
  const payload = { jobId, text: '中文', voice: 'voice', rate: 0, pitch: 0 }
  await ipc.ttsSynthesize(payload)
  await ipc.ttsCancel(jobId)
  expect(invokeCommand).toHaveBeenNthCalledWith(1, 'tts_prepare', {})
  expect(invokeCommand).toHaveBeenNthCalledWith(2, 'tts_synthesize', payload)
  expect(invokeCommand).toHaveBeenNthCalledWith(3, 'tts_cancel', { jobId })
})
