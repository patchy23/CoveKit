import { enableAutoUnmount, flushPromises, mount } from '@vue/test-utils'
import { defineComponent, ref } from 'vue'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import type { ArchiveEvent, ArchiveRequest } from '../contracts'
import { useRemoteArchives } from './useRemoteArchives'
const env = vi.hoisted(() => ({ run: vi.fn(), cancel: vi.fn(), toast: vi.fn() }))
vi.mock('../ipc', () => ({ ipc: { sshArchiveRun: env.run, sshArchiveCancel: env.cancel } }))
vi.mock('@/stores/ui', () => ({ useUiStore: () => ({ toast: env.toast }) }))
enableAutoUnmount(afterEach)
beforeEach(() => {
  vi.resetAllMocks()
  env.cancel.mockResolvedValue(undefined)
})
function setup() {
  let archives!: ReturnType<typeof useRemoteArchives>
  const session = ref('s'),
    refresh = vi.fn()
  mount(
    defineComponent({
      setup() {
        archives = useRemoteArchives(() => session.value, refresh)
        return () => null
      },
    })
  )
  return { archives, session, refresh }
}
const request: ArchiveRequest = {
  operation: 'compress',
  format: 'zip',
  paths: ['/srv/a'],
  output: '/srv/a.zip',
}
it('准备时取消会等登记后发送，收到进程结束前不显示已取消', async () => {
  let resolve!: (event: ArchiveEvent) => void
  env.run.mockImplementation(
    () =>
      new Promise<ArchiveEvent>((done) => {
        resolve = done
      })
  )
  const { archives } = setup()
  const id = archives.start(request)!
  await archives.cancel(id)
  expect(env.cancel).not.toHaveBeenCalled()
  env.run.mock.calls[0][3]({ kind: 'queued' })
  expect(env.cancel).toHaveBeenCalledWith(id)
  env.run.mock.calls[0][3]({ kind: 'result', status: 'cancelled' })
  expect(archives.tasks.value.get(id)?.done).toBe(false)
  resolve({ kind: 'result', status: 'cancelled' })
  await flushPromises()
  expect(archives.tasks.value.get(id)).toMatchObject({ done: true, cancelling: true })
})
it('关闭预览只取消扫描，不影响独立压缩任务', async () => {
  env.run.mockImplementation(() => new Promise(() => {}))
  const { archives } = setup()
  const job = archives.start(request)!
  env.run.mock.calls[0][3]({ kind: 'queued' })
  archives.openPreview('/srv/b.zip')
  env.run.mock.calls[1][3]({ kind: 'queued' })
  const scan = archives.preview.value!.id
  archives.closePreview()
  await flushPromises()
  expect(env.cancel).toHaveBeenCalledWith(scan)
  expect(env.cancel).not.toHaveBeenCalledWith(job)
  expect(archives.tasks.value.get(job)?.done).toBe(false)
})
it('断线结果未知不冒充取消成功，错误可以保留并重试', async () => {
  env.run.mockRejectedValue(new Error('通道关闭，结果未知'))
  const { archives } = setup()
  const id = archives.start(request)!
  await flushPromises()
  expect(archives.tasks.value.get(id)).toMatchObject({
    done: true,
    cancelling: false,
    error: expect.stringContaining('结果未知'),
  })
  archives.retry(id)
  expect(env.run).toHaveBeenCalledTimes(2)
  archives.clearEnded()
  expect(archives.tasks.value.has(id)).toBe(false)
  await flushPromises()
})
