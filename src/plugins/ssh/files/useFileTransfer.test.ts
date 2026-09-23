import { mount, enableAutoUnmount } from '@vue/test-utils'
import { defineComponent, ref } from 'vue'
import { afterEach, expect, it, vi } from 'vitest'
import { useFileTransfer } from './useFileTransfer'
import type { FileTransferProgress } from '../contracts'
const env = vi.hoisted(() => ({
  upload: vi.fn(),
  download: vi.fn(),
  cancel: vi.fn(),
  stop: vi.fn(),
}))
vi.mock('../ipc', () => ({
  ipc: {
    sshFileUpload: env.upload,
    sshFileDownloadRecursive: env.download,
    sshTransferCancel: env.cancel,
  },
  onTransferProgress: vi.fn(async () => env.stop),
}))
enableAutoUnmount(afterEach)
function setup() {
  let transfer!: ReturnType<typeof useFileTransfer>
  const connection = ref('s'),
    refresh = vi.fn()
  mount(
    defineComponent({
      setup() {
        transfer = useFileTransfer(() => connection.value, refresh)
        return () => null
      },
    })
  )
  return { transfer, connection, refresh }
}
const progress = (id = 'up-1', done = false): FileTransferProgress => ({
  transferId: id,
  connectionId: 's',
  localPath: '/local',
  remotePath: '/remote',
  transferred: done ? 100 : 0,
  total: 100,
  done,
})
it('首次事件就是完成时仍可见，迟到的开始响应不回退状态', () => {
  const { transfer, refresh } = setup()
  transfer.applyProgress(progress('up-1', true))
  transfer.applyProgress(progress())
  expect(transfer.transfers.value.get('up-1')).toMatchObject({ done: true, transferred: 100 })
  expect(refresh).toHaveBeenCalledTimes(1)
  transfer.applyProgress({ ...progress('up-other'), connectionId: 'other' })
  expect(transfer.transfers.value.size).toBe(1)
})
it('已结束历史有界，活动任务不会随清理删除', () => {
  const { transfer } = setup()
  transfer.applyProgress(progress('up-active'))
  for (let i = 0; i < 220; i++) transfer.applyProgress(progress('up-' + i, true))
  expect(transfer.transfers.value.size).toBe(201)
  transfer.clearEnded()
  expect([...transfer.transfers.value.keys()]).toEqual(['up-active'])
})
it('准备期间可立即看到任务，准备失败保留错误用于重试', async () => {
  const { transfer } = setup()
  let fail!: (e: Error) => void
  env.upload.mockReturnValue(new Promise((_resolve, reject) => (fail = reject)))
  const pending = transfer.startTransfer('upload', '/local', '/remote')
  expect([...transfer.transfers.value.values()][0].preparing).toBe(true)
  fail(new Error('读取失败'))
  await expect(pending).rejects.toThrow('读取失败')
  expect([...transfer.transfers.value.values()][0]).toMatchObject({
    done: true,
    preparing: false,
    error: expect.stringContaining('读取失败'),
  })
})
