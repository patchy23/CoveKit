import { enableAutoUnmount, flushPromises, shallowMount } from '@vue/test-utils'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import ComposeContainers from './ComposeContainers.vue'
import DockerTable from '../docker/DockerTable.vue'
import type { ComposeProject, ServerConnection } from '../contracts'

const mocks = vi.hoisted(() => ({ action: vi.fn() }))
vi.mock('../ipc', () => ({ ipc: { sshComposeAction: mocks.action } }))
vi.mock('../monitor/logWindows', () => ({ useLogWindows: () => vi.fn() }))
vi.mock('../terminal/TerminalTab.vue', () => ({ default: { template: '<div />' } }))
enableAutoUnmount(afterEach)
beforeEach(() => mocks.action.mockReset().mockResolvedValue(response('current')))
const connection: ServerConnection = { sessionId: 'one', profileId: 'p', status: 'connected' }
const project: ComposeProject = { name: 'app', status: 'running', configFiles: ['/a.yml'] }
function response(name: string) {
  return {
    exitCode: 0,
    stdout: JSON.stringify([{ ID: name, Name: name, State: 'running' }]),
    stderr: '',
  }
}
function deferred() {
  let resolve!: (value: ReturnType<typeof response>) => void
  const promise = new Promise<ReturnType<typeof response>>((done) => {
    resolve = done
  })
  return { promise, resolve }
}

it('同一编排重复快照更新只保留一次追补，并立即展示首份有效结果', async () => {
  const first = deferred()
  const tail = deferred()
  mocks.action.mockReturnValueOnce(first.promise).mockReturnValueOnce(tail.promise)
  const wrapper = shallowMount(ComposeContainers, { props: { connection, project, busy: false } })
  for (let i = 0; i < 8; i++) await wrapper.setProps({ project: { ...project } })
  expect(mocks.action).toHaveBeenCalledTimes(1)
  first.resolve(response('first'))
  await flushPromises()
  expect(mocks.action).toHaveBeenCalledTimes(2)
  expect(wrapper.getComponent(DockerTable).props('containers')[0].name).toBe('first')
  tail.resolve(response('tail'))
  await flushPromises()
  expect(wrapper.getComponent(DockerTable).props('containers')[0].name).toBe('tail')
})

it('切换配置立即启动新查询，旧结果不得覆盖新编排或再追补', async () => {
  const old = deferred()
  mocks.action.mockReturnValueOnce(old.promise)
  const wrapper = shallowMount(ComposeContainers, { props: { connection, project, busy: false } })
  await wrapper.setProps({ project: { ...project } })
  await wrapper.setProps({ project: { ...project, configFiles: ['/b.yml'] } })
  await flushPromises()
  expect(mocks.action).toHaveBeenCalledTimes(2)
  old.resolve(response('old'))
  await flushPromises()
  expect(wrapper.getComponent(DockerTable).props('containers')[0].name).toBe('current')
  expect(mocks.action).toHaveBeenCalledTimes(2)
})

it('卸载后不再执行排队的编排刷新', async () => {
  const old = deferred()
  mocks.action.mockReturnValueOnce(old.promise)
  const wrapper = shallowMount(ComposeContainers, { props: { connection, project, busy: false } })
  await wrapper.setProps({ project: { ...project } })
  wrapper.unmount()
  old.resolve(response('old'))
  await flushPromises()
  expect(mocks.action).toHaveBeenCalledTimes(1)
})
