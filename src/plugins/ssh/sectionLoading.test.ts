import { enableAutoUnmount, flushPromises, mount } from '@vue/test-utils'
import { reactive, ref } from 'vue'
import { afterEach, expect, it, vi } from 'vitest'
import SshTool from './index.vue'

const fixture = vi.hoisted(() => ({
  loads: [] as string[],
  workspace: {} as Record<string, unknown>,
}))
vi.mock('./useSshWorkspace', () => ({ useSshWorkspace: () => fixture.workspace }))
vi.mock('./toolLifecycle', () => ({ useSshToolLifecycle: () => {} }))
vi.mock('./profiles/ServerList.vue', () => ({ default: { template: '<aside>主机列表</aside>' } }))
vi.mock('./profiles/ServerForm.vue', () => ({ default: { template: '<div />' } }))
vi.mock('./connection/HostKeyDialog.vue', () => ({ default: { template: '<div />' } }))
vi.mock('./connection/KnownHostsDialog.vue', () => ({ default: { template: '<div />' } }))
vi.mock('./terminal/TerminalTab.vue', () => {
  fixture.loads.push('terminal')
  return { __esModule: true, default: { template: '<div data-section="terminal"><input /></div>' } }
})
vi.mock('./files/FileManagerTab.vue', () => {
  fixture.loads.push('files')
  return { __esModule: true, default: { template: '<div data-section="files" />' } }
})
vi.mock('./tunnels/TunnelTab.vue', () => {
  fixture.loads.push('tunnels')
  return { __esModule: true, default: { template: '<div data-section="tunnels" />' } }
})
vi.mock('./monitor/MonitorTab.vue', () => {
  fixture.loads.push('monitor')
  return { __esModule: true, default: { template: '<div data-section="monitor" />' } }
})
vi.mock('./monitor/ServiceTab.vue', () => {
  fixture.loads.push('services')
  return { __esModule: true, default: { template: '<div data-section="services" />' } }
})
vi.mock('./monitor/ProcessTab.vue', () => {
  fixture.loads.push('processes')
  return { __esModule: true, default: { template: '<div data-section="processes" />' } }
})
vi.mock('./docker/DockerTab.vue', () => {
  fixture.loads.push('docker')
  return { __esModule: true, default: { template: '<div data-section="docker" />' } }
})
enableAutoUnmount(afterEach)

it('先显示主机列表，功能页首次进入才加载，返回终端保留实例', async () => {
  const remote = reactive({
    id: 'workspace-1',
    profileId: 'profile-1',
    title: '测试连接',
    connection: { status: 'connected', sessionId: 'session-1' },
    activeSection: 'terminal',
    visitedSections: ['terminal'],
    connectRequest: 1,
  })
  const connections = ref<(typeof remote)[]>([])
  fixture.workspace = {
    profiles: ref([]),
    filteredProfiles: ref([]),
    groups: ref([]),
    expandedIds: ref(new Set()),
    connectionWorkspaces: connections,
    activeProfileId: ref(null),
    searchKeyword: ref(''),
    formOpen: ref(false),
    editingProfile: ref(null),
    deleteTarget: ref(null),
    hostKeyRequest: ref(null),
    touchWorkspace: vi.fn(),
  }
  const wrapper = mount(SshTool, {
    global: {
      stubs: {
        UiTabs: {
          name: 'TabsStub',
          props: ['modelValue', 'items'],
          emits: ['update:modelValue'],
          template: '<nav />',
        },
        ConfirmDialog: true,
        ContextMenu: true,
      },
    },
  })
  await flushPromises()
  expect(wrapper.text()).toContain('主机列表')
  expect(fixture.loads).toEqual([])

  connections.value.push(remote)
  await flushPromises()
  expect(fixture.loads).toEqual(['terminal'])
  const terminalInput = wrapper.get('[data-section="terminal"] input')
  await terminalInput.setValue('保留终端状态')
  const entered = ['terminal']
  let monitorElement: Element | undefined
  for (const section of ['files', 'tunnels', 'monitor', 'services', 'processes', 'docker']) {
    wrapper.findAllComponents({ name: 'TabsStub' })[1].vm.$emit('update:modelValue', section)
    await flushPromises()
    entered.push(section)
    expect(fixture.loads).toEqual(entered)
    expect(wrapper.find(`[data-section="${section}"]`).exists()).toBe(true)
    if (section === 'monitor') monitorElement = wrapper.get('[data-section="monitor"]').element
  }
  wrapper.findAllComponents({ name: 'TabsStub' })[1].vm.$emit('update:modelValue', 'terminal')
  await flushPromises()
  expect(wrapper.get('[data-section="terminal"] input').element).toBe(terminalInput.element)
  expect((terminalInput.element as HTMLInputElement).value).toBe('保留终端状态')
  expect(fixture.loads).toEqual(entered)
  expect(wrapper.get('[data-section="monitor"]').element).toBe(monitorElement)
  expect((monitorElement as HTMLElement).style.display).toBe('none')
  remote.activeSection = 'monitor'
  await flushPromises()
  expect(wrapper.get('[data-section="monitor"]').element).toBe(monitorElement)
  expect((monitorElement as HTMLElement).style.display).not.toBe('none')
  remote.connection.status = 'disconnected'
  await flushPromises()
  expect(wrapper.find('[data-section="monitor"]').exists()).toBe(false)
})
