import { beforeEach, expect, it, vi } from 'vitest'
import { effectScope } from 'vue'
import type { Channel } from '@tauri-apps/api/core'
import type { TransferProgress } from '@/core/ipc/contracts'
import { useBackupRequest } from './useBackupRequest'

const api = vi.hoisted(() => ({
  dataTransferPrepare: vi.fn(async () => 'backup-request'),
  dataTransferCancel: vi.fn(async () => ({ cancelled: true })),
}))
vi.mock('@/core/ipc/ipc', () => ({ ipc: api }))
vi.mock('@tauri-apps/api/core', () => ({
  Channel: class {
    onmessage = vi.fn()
  },
}))
beforeEach(() => vi.clearAllMocks())

it('取消早于登记或作用域关闭，不启动 KDF 请求并释放迟到登记', async () => {
  let ready!: (id: string) => void
  api.dataTransferPrepare.mockReturnValueOnce(
    new Promise((resolve) => {
      ready = resolve
    })
  )
  const report = vi.fn()
  const scope = effectScope()
  const request = scope.run(() => useBackupRequest(report))!
  const operation = vi.fn()
  const promise = request.run(operation)
  const rejection = expect(promise).rejects.toMatchObject({ name: 'AbortError' })
  scope.stop()
  ready('late-backup')
  await rejection
  expect(operation).not.toHaveBeenCalled()
  expect(api.dataTransferCancel).toHaveBeenCalledWith('late-backup')
  expect(request.busy.value).toBe(false)
  expect(report).not.toHaveBeenCalled()
})

it('重复确认不创建第二个重任务，等待状态按归属更新，完成后断开回调', async () => {
  const request = useBackupRequest(vi.fn())
  let progress!: Channel<TransferProgress>
  let done!: (value: number) => void
  const pending = request.run((_id, channel) => {
    progress = channel
    return new Promise<number>((resolve) => {
      done = resolve
    })
  })
  await Promise.resolve()
  await expect(request.run(vi.fn())).rejects.toThrow('尚未完成')
  expect(api.dataTransferPrepare).toHaveBeenCalledOnce()
  progress.onmessage({ requestId: 'older', waitingMemory: true, estimatedBytes: 1024 })
  expect(request.waitingMemory.value).toBe(false)
  progress.onmessage({ requestId: 'backup-request', waitingMemory: true, estimatedBytes: 1024 })
  expect(request.waitingMemory.value).toBe(true)
  await request.cancel()
  expect(api.dataTransferCancel).toHaveBeenCalledWith('backup-request')
  expect(request.waitingMemory.value).toBe(false)
  done(12)
  expect(await pending).toBe(12) // 已完成写入仍应如实返回，不伪装回滚。
  progress.onmessage({ requestId: 'backup-request', waitingMemory: true, estimatedBytes: 1024 })
  expect(request.waitingMemory.value).toBe(false)
})

it('释放失败明确反馈，不吞掉已生成的结果', async () => {
  const report = vi.fn()
  const request = useBackupRequest(report)
  api.dataTransferCancel.mockRejectedValueOnce(new Error('释放失败'))
  expect(await request.run(async () => 7)).toBe(7)
  expect(report).toHaveBeenCalledWith(expect.objectContaining({ message: '释放失败' }))
})
