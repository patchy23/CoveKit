import { flushPromises, shallowMount } from '@vue/test-utils'
import { createPinia } from 'pinia'
import { beforeEach, expect, it, vi } from 'vitest'
import { UiCodeEditor, UiSearchInput, UiSelect } from '@/core/ui'
import type { ServerConnection, SystemdService } from '../contracts'
import ServiceTab from './ServiceTab.vue'
import ServiceConfigDialog from './ServiceConfigDialog.vue'

const mock = vi.hoisted(() => ({ sshServiceList: vi.fn(), sshServiceConfig: vi.fn() }))
vi.mock('../ipc', () => ({ ipc: mock }))

const connection = (id: string): ServerConnection => ({
  profileId: 'profile',
  sessionId: id,
  status: 'connected',
})
const services: SystemdService[] = [
  {
    name: 'nginx.service',
    description: 'Web gateway',
    activeState: 'active',
    loadState: 'loaded',
    subState: 'running',
    enabled: true,
  },
  {
    name: 'worker.service',
    description: 'Background jobs',
    activeState: 'inactive',
    loadState: 'loaded',
    subState: 'dead',
    enabled: false,
  },
]
const globalOptions = { plugins: [createPinia()], renderStubDefaultSlot: true }

beforeEach(() => {
  vi.clearAllMocks()
  mock.sshServiceList.mockResolvedValue(services)
})

it('服务名称和描述搜索与状态组合，状态恢复不会缺失其他服务', async () => {
  const wrapper = shallowMount(ServiceTab, {
    props: { connection: connection('one') },
    global: globalOptions,
  })
  await flushPromises()
  wrapper.findComponent(UiSearchInput).vm.$emit('update:modelValue', ' WEB ')
  await flushPromises()
  expect(wrapper.findAll('tbody tr')).toHaveLength(1)
  expect(wrapper.find('tbody').text()).toContain('nginx.service')
  wrapper.findComponent(UiSearchInput).vm.$emit('update:modelValue', '')
  wrapper.findComponent(UiSelect).vm.$emit('update:modelValue', 'inactive')
  await flushPromises()
  expect(wrapper.find('tbody').text()).toContain('worker.service')
  wrapper.findComponent(UiSelect).vm.$emit('update:modelValue', 'all')
  await flushPromises()
  expect(wrapper.findAll('tbody tr')).toHaveLength(2)
  expect(mock.sshServiceList).toHaveBeenCalledWith({ connectionId: 'one', filter: 'all' })
  wrapper.unmount()
})

it('配置查看拒绝旧服务晚到结果，连接切换后错误可见且不保留旧内容', async () => {
  let completeOld!: (value: string) => void
  mock.sshServiceConfig.mockImplementationOnce(
    () =>
      new Promise<string>((resolve) => {
        completeOld = resolve
      })
  )
  const wrapper = shallowMount(ServiceConfigDialog, {
    props: { connectionId: 'one', serviceName: 'old.service' },
    global: globalOptions,
  })
  mock.sshServiceConfig.mockResolvedValueOnce('# new.service\n[Service]\nExecStart=/bin/new')
  await wrapper.setProps({ serviceName: 'new.service' })
  await flushPromises()
  completeOld('old configuration')
  await flushPromises()
  expect(wrapper.findComponent(UiCodeEditor).props('modelValue')).toContain('ExecStart=/bin/new')
  expect(wrapper.findComponent(UiCodeEditor).props('readonly')).toBe(true)
  mock.sshServiceConfig.mockRejectedValueOnce(new Error('Permission denied'))
  await wrapper.setProps({ connectionId: 'two' })
  await flushPromises()
  expect(wrapper.text()).toContain('Permission denied')
  expect(wrapper.findComponent(UiCodeEditor).exists()).toBe(false)
  wrapper.unmount()
})
