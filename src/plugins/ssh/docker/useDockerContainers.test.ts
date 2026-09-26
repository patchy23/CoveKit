import { flushPromises, mount, type VueWrapper } from '@vue/test-utils'
import { defineComponent, h, ref } from 'vue'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { useDockerContainers } from './useDockerContainers'
import type { DockerContainer } from '../contracts'

const list = vi.hoisted(() => vi.fn())
vi.mock('../ipc', () => ({ ipc: { sshDockerList: list } }))
let wrapper: VueWrapper | undefined
beforeEach(() => vi.resetAllMocks())
afterEach(() => {
  wrapper?.unmount()
  wrapper = undefined
})
function deferred() {
  let resolve!: (value: DockerContainer[]) => void
  let reject!: (reason: Error) => void
  const promise = new Promise<DockerContainer[]>((yes, no) => {
    resolve = yes
    reject = no
  })
  return { resolve, reject, promise }
}
function setup() {
  const id = ref<string | undefined>('a')
  const report = vi.fn()
  let result!: ReturnType<typeof useDockerContainers>
  wrapper = mount(
    defineComponent({
      setup() {
        result = useDockerContainers(() => id.value, report)
        return () => h('div')
      },
    })
  )
  return { ...result, id, report }
}

it('重复点击只追加一次读取，操作后刷新不能接受操作前的快照', async () => {
  const old = deferred(),
    fresh = deferred()
  list.mockReturnValueOnce(old.promise).mockReturnValueOnce(fresh.promise)
  const { refresh, containers } = setup()
  const waiting = Array.from({ length: 100 }, () => refresh())
  expect(list).toHaveBeenCalledTimes(1)
  old.resolve([{ id: 'old' } as DockerContainer])
  await flushPromises()
  expect(list).toHaveBeenCalledTimes(2)
  expect(containers.value).toEqual([])
  fresh.resolve([{ id: 'new' } as DockerContainer])
  await Promise.all(waiting)
  expect(containers.value[0]?.id).toBe('new')
})

it('切换连接立即请求，不等待旧连接；同 ID 切回也拒绝旧结果', async () => {
  const old = deferred(),
    other = deferred(),
    back = deferred()
  list
    .mockReturnValueOnce(old.promise)
    .mockReturnValueOnce(other.promise)
    .mockReturnValueOnce(back.promise)
  const { id, containers, report } = setup()
  id.value = 'b'
  id.value = 'a'
  expect(list.mock.calls.map(([value]) => value)).toEqual(['a', 'b', 'a'])
  back.resolve([{ id: 'current' } as DockerContainer])
  await flushPromises()
  old.resolve([{ id: 'expired' } as DockerContainer])
  other.reject(new Error('旧连接失败'))
  await flushPromises()
  expect(containers.value[0]?.id).toBe('current')
  expect(report).not.toHaveBeenCalled()
})

it('卸载取消待发刷新且晚到结果不写回，后续调用不启动请求', async () => {
  const old = deferred()
  list.mockReturnValue(old.promise)
  const { refresh, containers } = setup()
  const waiting = refresh()
  wrapper!.unmount()
  wrapper = undefined
  old.resolve([{ id: 'expired' } as DockerContainer])
  await waiting
  await refresh()
  expect(containers.value).toEqual([])
  expect(list).toHaveBeenCalledTimes(1)
})

it('有效请求失败报告一次，下一次刷新可以恢复', async () => {
  list.mockRejectedValueOnce(new Error('断开')).mockResolvedValueOnce([])
  const { refresh, report } = setup()
  await flushPromises()
  expect(report).toHaveBeenCalledWith('容器列表加载失败：Error: 断开')
  await refresh()
  expect(list).toHaveBeenCalledTimes(2)
  expect(report).toHaveBeenCalledTimes(1)
})
