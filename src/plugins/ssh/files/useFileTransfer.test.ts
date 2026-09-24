import { mount, enableAutoUnmount, flushPromises } from '@vue/test-utils'
import { defineComponent, ref } from 'vue'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
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
beforeEach(() => vi.clearAllMocks())
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
  await pending
  await flushPromises()
  expect([...transfer.transfers.value.values()][0]).toMatchObject({
    done: true,
    preparing: false,
    error: expect.stringContaining('读取失败'),
  })
})

it('取消请求与成功完成竞态以真实完成结果为准', async () => {
  const { transfer } = setup()
  transfer.applyProgress(progress())
  await transfer.cancelTransfer('up-1')
  transfer.applyProgress(progress('up-1', true))
  expect(transfer.transfers.value.get('up-1')).toMatchObject({ done: true, cancelling: false })
})

it('全部预入队，两项并发，取消全部后不会启动排队项', async () => {
  const { transfer } = setup()
  let sequence = 0
  env.download.mockImplementation(async ({ localPath, remotePath }) => ({
    ...progress('down-' + ++sequence),
    localPath,
    remotePath,
  }))
  for (let i = 0; i < 6; i++)
    await transfer.startTransfer('download', '/local/' + i, '/remote/' + i)
  await flushPromises()
  expect(transfer.transfers.value.size).toBe(6)
  expect(env.download).toHaveBeenCalledTimes(2)
  expect([...transfer.transfers.value.values()].filter((v) => v.queued)).toHaveLength(4)
  transfer.cancelAll()
  expect([...transfer.transfers.value.values()].filter((v) => v.done && v.cancelling)).toHaveLength(
    4
  )
  transfer.applyProgress({ ...progress('down-1', true), error: '已取消' })
  transfer.applyProgress({ ...progress('down-2', true), error: '已取消' })
  await flushPromises()
  expect(env.download).toHaveBeenCalledTimes(2)
  expect(env.cancel).toHaveBeenCalledWith('down-1')
  expect(env.cancel).toHaveBeenCalledWith('down-2')
})
it('同目录及子路径串行，完成后继续排队并默认覆盖', async () => {
  const { transfer } = setup()
  let sequence = 0
  env.download.mockImplementation(async ({ localPath, remotePath }) => ({
    ...progress('down-' + ++sequence),
    localPath,
    remotePath,
  }))
  await transfer.startTransfer('download', '/local/dir', '/remote/dir')
  await transfer.startTransfer('download', '/local/dir/file', '/remote/file')
  await flushPromises()
  expect(env.download).toHaveBeenCalledTimes(1)
  expect(env.download).toHaveBeenCalledWith(expect.objectContaining({ overwrite: true }))
  transfer.applyProgress(progress('down-1', true))
  await flushPromises()
  expect(env.download).toHaveBeenCalledTimes(2)
})
it('取消准备中的任务，在后端返回标识后补发取消', async () => {
  const { transfer } = setup()
  let finish!: (p: FileTransferProgress) => void
  env.download.mockReturnValue(new Promise((resolve) => (finish = resolve)))
  await transfer.startTransfer('download', '/local', '/remote')
  transfer.cancelAll()
  finish(progress('down-late'))
  await flushPromises()
  expect(env.cancel).toHaveBeenCalledWith('down-late')
  expect(transfer.transfers.value.get('down-late')?.cancelling).toBe(true)
})
