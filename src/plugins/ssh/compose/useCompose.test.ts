/* eslint-disable vue/one-component-per-file -- 独立挂载状态夹具与提供日志宿主的组件夹具 */
import { enableAutoUnmount, flushPromises, mount, shallowMount } from '@vue/test-utils'
import { defineComponent, h, ref } from 'vue'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import type { ComposeOutput, ComposeProject, ServerConnection } from '../contracts'
import { useCompose } from './useCompose'
import { COMPOSE_TEMPLATE, mergeComposeProjects, validateComposeDraft } from './composeProjects'
import ComposeTab from './ComposeTab.vue'
import ComposeContainers from './ComposeContainers.vue'
import DockerTable from '../docker/DockerTable.vue'
import { provideLogWindows } from '../monitor/logWindows'
import TerminalTab from '../terminal/TerminalTab.vue'
import ComposeCreateDialog from './ComposeCreateDialog.vue'
import ComposeDirectoryPicker from './ComposeDirectoryPicker.vue'
import ComposeProjectList from './ComposeProjectList.vue'
import {
  UiButton,
  UiCodeEditor,
  UiTableExpandableRow,
  UiModal,
  UiSelect,
  UiSearchInput,
  UiIconButton,
  UiInput,
} from '@/core/ui'
import { UiConfirmDialog } from '@/core/ui'

const env = vi.hoisted(() => ({
  sshComposeList: vi.fn(),
  sshComposeAction: vi.fn(),
  sshComposeStream: vi.fn(),
  sshComposeHome: vi.fn(),
  sshComposeCreate: vi.fn(),
  sshEditOpen: vi.fn(),
  sshEditSave: vi.fn(),
  sshFileList: vi.fn(),
  sshDockerList: vi.fn(),
  setToolSetting: vi.fn(),
}))
vi.mock('../ipc', () => ({ ipc: env }))
vi.mock('@/stores/settings', () => ({
  useSettingsStore: () => ({
    getToolSetting: (_tool: string, _key: string, fallback: unknown) => fallback,
    setToolSetting: env.setToolSetting,
  }),
}))
enableAutoUnmount(afterEach)
const project: ComposeProject = {
  name: 'app',
  status: 'running(1)',
  configFiles: ['/opt/app/compose.yaml', '/opt/app/override.yaml'],
}
const file = (content = 'services: {}', modifiedAt = 1000) => ({
  ok: true,
  content,
  modifiedAt,
  path: project.configFiles[0],
  encoding: 'UTF-8',
  size: content.length,
})
function setup() {
  const connection = ref<ServerConnection>({
    profileId: 'profile',
    sessionId: 'one',
    status: 'connected',
  })
  let api!: ReturnType<typeof useCompose>
  const wrapper = mount(
    defineComponent({
      setup() {
        api = useCompose(
          () => connection.value,
          () => 'profile',
          'workspace'
        )
        return () => h('div')
      },
    })
  )
  return { api, wrapper, connection }
}
beforeEach(() => {
  vi.resetAllMocks()
  env.sshComposeList.mockResolvedValue([project])
  env.sshEditOpen.mockResolvedValue(file())
  env.setToolSetting.mockResolvedValue(undefined)
  env.sshComposeCreate.mockResolvedValue(undefined)
  env.sshEditSave.mockResolvedValue({ ok: true })
  env.sshComposeStream.mockImplementation((payload) => env.sshComposeAction(payload))
  env.sshComposeHome.mockResolvedValue('/home/test')
  env.sshComposeAction.mockResolvedValue({
    exitCode: 0,
    stdout: JSON.stringify({
      ID: 'new',
      Name: 'new-container',
      Image: 'nginx',
      State: 'running',
      Status: 'Up 1 hour',
      Ports: '80/tcp',
    }),
    stderr: '',
  })
})

it('以 Docker 查询为权威合并路径记录，失败保留列表并报错', async () => {
  expect(mergeComposeProjects([project], [{ ...project, status: 'old' }])).toEqual([project])
  const { api } = setup()
  await flushPromises()
  expect(api.projects.value).toEqual([project])
  env.sshComposeList.mockRejectedValueOnce(new Error('Docker unavailable'))
  await api.refresh()
  expect(api.projects.value).toEqual([project])
  expect(api.listError.value).toContain('Docker unavailable')
})

it('切换配置文件后忽略迟到读取结果，保留多文件合并顺序', async () => {
  const { api } = setup()
  let finish!: (result: ReturnType<typeof file>) => void
  env.sshEditOpen.mockImplementationOnce(
    () =>
      new Promise((resolve) => {
        finish = resolve
      })
  )
  const old = api.open(project)
  await api.open(project, project.configFiles[1])
  finish(file('old'))
  await old
  expect(api.content.value).toBe('services: {}')
  expect(api.filePath.value).toBe(project.configFiles[1])
  expect(api.selected.value?.configFiles).toEqual(project.configFiles)
})

it('保存传递文件版本，冲突不清空草稿也不部署', async () => {
  const { api } = setup()
  await api.open(project)
  api.content.value = 'edited'
  env.sshEditSave.mockResolvedValueOnce({ ok: false, conflict: true })
  await api.save()
  expect(env.sshEditSave).toHaveBeenCalledWith('one', project.configFiles[0], 'edited', 1000)
  expect(api.content.value).toBe('edited')
  expect(api.dirty.value).toBe(true)
  expect(api.error.value).toContain('本次未覆盖')
  await api.run('up')
  expect(env.sshComposeAction).not.toHaveBeenCalled()
})

it('新建只写 YAML 并记住路径，不自动部署，保存后可显式执行', async () => {
  const { api } = setup()
  api.create()
  api.draftName.value = 'new-app'
  api.filePath.value = '/opt/new/compose.yaml'
  env.sshEditOpen.mockResolvedValueOnce(file(COMPOSE_TEMPLATE))
  await api.save()
  expect(env.sshComposeCreate).toHaveBeenCalledWith({
    connectionId: 'one',
    remotePath: '/opt/new/compose.yaml',
    content: COMPOSE_TEMPLATE,
  })
  expect(env.setToolSetting.mock.calls[0][2]).toEqual({
    profile: [
      {
        name: 'new-app',
        status: '未部署',
        configFiles: ['/opt/new/compose.yaml'],
        workingDir: '/opt/new',
      },
    ],
  })
  expect(api.isNew.value).toBe(false)
  expect(api.dirty.value).toBe(false)
  expect(env.sshComposeAction).not.toHaveBeenCalled()
  env.sshComposeAction.mockResolvedValueOnce({
    exitCode: 1,
    stdout: '',
    stderr: 'invalid configuration',
  })
  await api.run('config')
  expect(api.error.value).toContain('退出码 1')
  expect(api.output.value?.stderr).toBe('invalid configuration')
})

it('重连保留草稿但失效旧版本，不允许无版本覆盖', async () => {
  const { api, connection } = setup()
  await api.open(project)
  api.content.value = 'draft'
  connection.value = { ...connection.value, sessionId: 'two' }
  await flushPromises()
  await api.save()
  expect(api.content.value).toBe('draft')
  expect(env.sshEditSave).not.toHaveBeenCalled()
  expect(api.error.value).toContain('文件版本未知')
})

it('卸载后操作结果不回填、不重新查询列表', async () => {
  const { api, wrapper } = setup()
  await api.open(project)
  let finish!: (value: { exitCode: number; stdout: string; stderr: string }) => void
  env.sshComposeAction.mockImplementationOnce(
    () =>
      new Promise((resolve) => {
        finish = resolve
      })
  )
  const pending = api.run('up')
  wrapper.unmount()
  finish({ exitCode: 0, stdout: 'done', stderr: '' })
  await pending
  expect(api.output.value).toBeNull()
  expect(env.sshComposeList).toHaveBeenCalledTimes(1)
})

it('新建校验项目名和绝对 YAML 路径', () => {
  expect(validateComposeDraft('app', '/opt/a b/compose.yaml')).toBe('')
  expect(validateComposeDraft('APP', '/opt/compose.yaml')).not.toBe('')
  expect(validateComposeDraft('app', 'compose.yaml')).not.toBe('')
  expect(validateComposeDraft('app', '/opt/bad\ncompose.yaml')).not.toBe('')
})

it('新建保存后关闭页面，不在路径记忆完成后继续访问旧连接', async () => {
  const { api, wrapper } = setup()
  api.create()
  api.draftName.value = 'new'
  api.filePath.value = '/opt/new.yaml'
  let finish!: () => void
  env.setToolSetting.mockImplementationOnce(
    () =>
      new Promise<void>((resolve) => {
        finish = resolve
      })
  )
  const pending = api.save()
  await flushPromises()
  wrapper.unmount()
  finish()
  await pending
  expect(env.sshEditOpen).not.toHaveBeenCalled()
})

it('编辑弹窗关闭须确认，取消保留草稿并向连接页上报未保存状态', async () => {
  const wrapper = shallowMount(ComposeTab, {
    props: {
      connection: { profileId: 'profile', sessionId: 'one', status: 'connected' },
      profileId: 'profile',
      workspaceId: 'ui',
    },
    global: {
      renderStubDefaultSlot: true,
      stubs: { ComposeProjectList: false, UiTableExpandableRow: false },
    },
  })
  await flushPromises()
  const buttons = () => wrapper.findAllComponents(UiButton)
  buttons()
    .find((button) => button.text() === '编辑')!
    .vm.$emit('click')
  await flushPromises()
  wrapper.getComponent(UiCodeEditor).vm.$emit('update:modelValue', 'unsaved draft')
  await flushPromises()
  wrapper
    .findAllComponents(UiModal)
    .find((modal) => modal.props('title') === 'app · 编辑')!
    .vm.$emit('close')
  await flushPromises()
  const confirmation = wrapper
    .findAllComponents(UiConfirmDialog)
    .find((dialog) => dialog.props('title') === '放弃未保存的修改')!
  expect(confirmation.props('open')).toBe(true)
  confirmation.vm.$emit('close')
  await flushPromises()
  expect(wrapper.getComponent(UiCodeEditor).props('modelValue')).toBe('unsaved draft')
  expect(wrapper.emitted('state')?.at(-1)).toEqual([{ dirty: true, busy: false }])
})

it('草稿校验传递当前文件与内容，不写盘，运行操作仍拒绝脏草稿', async () => {
  const { api } = setup()
  await api.open({ ...project, workingDir: '/srv/data' }, project.configFiles[1])
  api.content.value = 'services: { web: { image: nginx } }'
  env.sshComposeAction.mockResolvedValue({ exitCode: 0, stdout: '', stderr: '' })
  await api.run('config')
  expect(env.sshComposeStream.mock.calls[0][0]).toMatchObject({
    project: { workingDir: '/srv/data', configFiles: project.configFiles },
    draftPath: project.configFiles[1],
    draftContent: api.content.value,
  })
  expect(api.dirty.value).toBe(true)
  expect(env.sshEditSave).not.toHaveBeenCalled()
  await api.run('up')
  expect(env.sshComposeStream).toHaveBeenCalledTimes(1)
})

it('运行过程中显示输出，重连后的迟到片段与完成结果不污染当前页面', async () => {
  const { api, connection } = setup()
  await api.open(project)
  let finish!: (result: { exitCode: number; stdout: string; stderr: string }) => void
  let chunk!: (text: string) => void
  env.sshComposeStream.mockImplementation((_payload, onChunk) => {
    chunk = onChunk
    return new Promise((resolve) => {
      finish = resolve
    })
  })
  const pending = api.run('update')
  chunk('Pulling image')
  expect(api.liveOutput.value).toBe('Pulling image')
  connection.value = { ...connection.value, sessionId: 'two' }
  await flushPromises()
  chunk('late')
  finish({ exitCode: 0, stdout: 'late', stderr: '' })
  await pending
  expect(api.liveOutput.value).toBe('Pulling image')
  expect(api.output.value).toBeNull()
})

it('刷新不覆盖当前文件集合，拆除后保留路径并显示未部署', async () => {
  const { api } = setup()
  await api.open({ ...project, workingDir: '/srv/original' })
  api.content.value = 'draft'
  env.sshComposeList.mockResolvedValue([])
  await api.refresh()
  expect(api.selected.value).toMatchObject({
    status: '未部署',
    workingDir: '/srv/original',
    configFiles: project.configFiles,
  })
  expect(api.content.value).toBe('draft')
})

it('默认目录使用远程主目录，取消编辑恢复已加载内容', async () => {
  const { api } = setup()
  expect(await api.defaultDirectory()).toBe('/home/test/compose')
  await api.open(project)
  api.content.value = 'draft'
  api.discard()
  expect(api.content.value).toBe('services: {}')
  expect(api.dirty.value).toBe(false)
})

it('编辑弹窗直接编辑，保存遇到冲突时保留草稿且不会启动容器', async () => {
  const wrapper = shallowMount(ComposeTab, {
    props: {
      connection: { profileId: 'profile', sessionId: 'one', status: 'connected' },
      profileId: 'profile',
      workspaceId: 'ui-save',
    },
    global: {
      renderStubDefaultSlot: true,
      stubs: { ComposeProjectList: false, UiTableExpandableRow: false },
    },
  })
  const click = async (text: string) => {
    wrapper
      .findAllComponents(UiButton)
      .find((b) => b.text().includes(text))!
      .vm.$emit('click')
    await flushPromises()
  }
  await flushPromises()
  await click('编辑')
  expect(wrapper.getComponent(UiCodeEditor).props('readonly')).toBe(false)
  wrapper.getComponent(UiCodeEditor).vm.$emit('update:modelValue', 'new draft')
  await flushPromises()
  env.sshEditSave.mockResolvedValueOnce({ ok: false, conflict: true })
  wrapper.getComponent(UiCodeEditor).vm.$emit('save')
  await flushPromises()
  expect(env.sshComposeStream).not.toHaveBeenCalled()
  expect(wrapper.getComponent(UiCodeEditor).props('modelValue')).toBe('new draft')
  expect(wrapper.text()).toContain('本次未覆盖')
})

it('切换编排后迟到的容器列表不覆盖新项目', async () => {
  let finish!: (result: ComposeOutput) => void
  env.sshComposeAction.mockImplementationOnce(
    () =>
      new Promise((resolve) => {
        finish = resolve
      })
  )
  const currentProject = ref(project)
  let logWindows!: ReturnType<typeof provideLogWindows>
  const host = mount(
    defineComponent({
      setup() {
        logWindows = provideLogWindows(() => [{ sessionId: 'one', title: '测试连接' }])
        return () =>
          h(ComposeContainers, {
            project: currentProject.value,
            connection: { profileId: 'profile', sessionId: 'one', status: 'connected' },
            busy: false,
          })
      },
    }),
    { global: { stubs: { TerminalTab: true } } }
  )
  const wrapper = host.getComponent(ComposeContainers)
  currentProject.value = { ...project, name: 'new' }
  await flushPromises()
  finish({
    exitCode: 0,
    stdout: JSON.stringify({ ID: 'old', Name: 'old-container', State: 'running' }),
    stderr: '',
  })
  await flushPromises()
  expect(wrapper.text()).toContain('new-container')
  expect(wrapper.text()).not.toContain('old-container')
  const table = wrapper.getComponent(DockerTable)
  expect(table.props('inspectOnly')).toBe(true)
  expect(table.props('containers')[0]).toMatchObject({ uptime: '1 hour', ports: '80/tcp' })
  expect(table.findAllComponents(UiButton).map((b) => b.text())).toEqual(['日志', '终端'])
  table.vm.$emit('logs', table.props('containers')[0])
  await flushPromises()
  expect(logWindows.windows.value[0]).toMatchObject({
    targetId: 'new',
    kind: 'docker',
    connectionId: 'one',
  })
  table.vm.$emit('terminal', table.props('containers')[0])
  await flushPromises()
  expect(wrapper.getComponent(TerminalTab).props('dockerContainerId')).toBe('new')
})

it('编辑真实弹窗可写入 YAML，关闭后列表保留搜索和滚动位置', async () => {
  env.sshEditOpen.mockResolvedValue(file('services:\n  web:\n    image: nginx:stable\n'))
  const wrapper = mount(ComposeTab, {
    attachTo: document.body,
    props: {
      connection: { profileId: 'profile', sessionId: 'one', status: 'connected' },
      profileId: 'profile',
      workspaceId: 'yaml-render',
    },
    global: { stubs: { ComposeContainers: true } },
  })
  await flushPromises()
  wrapper.getComponent(UiSearchInput).vm.$emit('update:modelValue', 'app')
  const scroll = wrapper.get('[data-testid="compose-list-scroll"]').element
  scroll.scrollTop = 120
  wrapper.getComponent(ComposeProjectList).vm.$emit('edit', project)
  await flushPromises()
  const content = document.querySelector('[role="dialog"] .cm-content')!
  expect(content.textContent).toContain('nginx:stable')
  expect(content.getAttribute('contenteditable')).toBe('true')
  expect(wrapper.findComponent(ComposeContainers).exists()).toBe(false)
  ;(
    document.querySelector('[role="dialog"] button[aria-label="关闭"]') as HTMLButtonElement
  ).click()
  await flushPromises()
  expect(wrapper.findComponent(UiCodeEditor).exists()).toBe(false)
  expect(wrapper.getComponent(UiSearchInput).props('modelValue')).toBe('app')
  expect(scroll.scrollTop).toBe(120)
})

it('切换 YAML 文件保护草稿，确认放弃后才读取新文件', async () => {
  const wrapper = shallowMount(ComposeTab, {
    props: {
      connection: { profileId: 'profile', sessionId: 'one', status: 'connected' },
      profileId: 'profile',
      workspaceId: 'switch-file',
    },
    global: { renderStubDefaultSlot: true },
  })
  await flushPromises()
  wrapper.getComponent(ComposeProjectList).vm.$emit('edit', project)
  await flushPromises()
  wrapper.getComponent(UiCodeEditor).vm.$emit('update:modelValue', 'unsaved')
  await flushPromises()
  wrapper.getComponent(UiSelect).vm.$emit('update:modelValue', project.configFiles[1])
  await flushPromises()
  const confirm = wrapper
    .findAllComponents(UiConfirmDialog)
    .find((c) => c.props('title') === '放弃未保存的修改')!
  expect(confirm.props('open')).toBe(true)
  expect(env.sshEditOpen).toHaveBeenCalledTimes(1)
  confirm.vm.$emit('confirm')
  await flushPromises()
  expect(env.sshEditOpen).toHaveBeenLastCalledWith('one', project.configFiles[1])
})

it('查看容器行内展开，单行切换与收起卸载面板，不读取 YAML', async () => {
  const other = { ...project, name: 'other' }
  env.sshComposeList.mockResolvedValue([project, other])
  const wrapper = mount(ComposeTab, {
    props: {
      connection: { profileId: 'profile', sessionId: 'one', status: 'connected' },
      profileId: 'profile',
      workspaceId: 'view-containers',
    },
    global: { stubs: { ComposeContainers: true } },
  })
  await flushPromises()
  const rows = wrapper.findAllComponents(UiTableExpandableRow)
  expect(wrapper.findComponent(ComposeContainers).exists()).toBe(false)
  await rows[0]!.get('button').trigger('click')
  expect(wrapper.getComponent(ComposeContainers).props('project')).toEqual(project)
  expect(wrapper.findComponent(UiCodeEditor).exists()).toBe(false)
  expect(env.sshEditOpen).not.toHaveBeenCalled()
  await rows[1]!.get('button').trigger('click')
  expect(wrapper.findAllComponents(ComposeContainers)).toHaveLength(1)
  expect(wrapper.getComponent(ComposeContainers).props('project').name).toBe('other')
  await rows[1]!.get('button').trigger('click')
  expect(wrapper.findComponent(ComposeContainers).exists()).toBe(false)
})

it('容器直接按项目查询，缺少 YAML 路径仍可查看，失败不显示零容器', async () => {
  env.sshComposeAction.mockResolvedValueOnce({
    exitCode: 1,
    stdout: '',
    stderr: 'permission denied',
  })
  const wrapper = mount(ComposeContainers, {
    props: {
      project: { ...project, configFiles: [] },
      connection: { profileId: 'profile', sessionId: 'one', status: 'connected' },
      busy: false,
    },
  })
  expect(wrapper.text()).not.toContain('正在读取容器')
  expect(wrapper.text()).not.toContain('该编排下没有容器')
  expect(wrapper.findComponent(DockerTable).exists()).toBe(true)
  expect(wrapper.get('section').attributes('aria-busy')).toBe('true')
  expect(wrapper.text()).not.toContain('容器 · 0')
  expect(wrapper.findAllComponents(UiButton).some((button) => button.text() === '刷新')).toBe(false)
  await flushPromises()
  expect(env.sshComposeAction).toHaveBeenCalledWith({
    connectionId: 'one',
    project: { ...project, configFiles: [] },
    action: 'ps',
  })
  expect(env.sshDockerList).not.toHaveBeenCalled()
  expect(wrapper.text()).toContain('permission denied')
  expect(wrapper.text()).not.toContain('该编排下没有容器')
  env.sshComposeAction.mockResolvedValueOnce({ exitCode: 0, stdout: '[]', stderr: '' })
  await wrapper.setProps({ project: { ...project, configFiles: [] } })
  await flushPromises()
  expect(wrapper.text()).toContain('该编排下没有容器')
})

it('编排展开使用 Compose 返回结果，分别传递相近项目名，不读取全量 Docker 列表', async () => {
  env.sshDockerList.mockResolvedValue([{ id: 'manual', name: 'echobank-8081' }])
  env.sshComposeAction.mockImplementation(async ({ project: selected }) => ({
    exitCode: 0,
    stdout: JSON.stringify({ ID: selected.name, Name: selected.name, State: 'running' }),
    stderr: '',
  }))
  const wrapper = mount(ComposeContainers, {
    props: {
      project: { ...project, name: 'echobank' },
      connection: { profileId: 'profile', sessionId: 'one', status: 'connected' },
      busy: false,
    },
  })
  await flushPromises()
  expect(
    wrapper
      .getComponent(DockerTable)
      .props('containers')
      .map((row) => row.name)
  ).toEqual(['echobank'])
  await wrapper.setProps({ project: { ...project, name: 'echobank1' } })
  await flushPromises()
  expect(
    wrapper
      .getComponent(DockerTable)
      .props('containers')
      .map((row) => row.name)
  ).toEqual(['echobank1'])
  expect(
    env.sshComposeAction.mock.calls.map(([payload]) => [payload.project.name, payload.action])
  ).toEqual([
    ['echobank', 'ps'],
    ['echobank1', 'ps'],
  ])
  expect(env.sshDockerList).not.toHaveBeenCalled()
  env.sshComposeAction.mockResolvedValueOnce({ exitCode: 0, stdout: 'invalid json', stderr: '' })
  await wrapper.setProps({ project: { ...project, name: 'echobank1' } })
  await flushPromises()
  expect(wrapper.text()).toContain('不是有效 JSON')
  expect(wrapper.findComponent(DockerTable).exists()).toBe(false)
  expect(wrapper.text()).not.toContain('该编排下没有容器')
})

it('关闭读取中的编辑弹窗后，迟到的 YAML 不再回填', async () => {
  const { api } = setup()
  let finish!: (value: ReturnType<typeof file>) => void
  env.sshEditOpen.mockImplementationOnce(
    () =>
      new Promise((resolve) => {
        finish = resolve
      })
  )
  const pending = api.open(project)
  api.discard()
  finish(file('late'))
  await pending
  expect(api.loaded.value).toBe(false)
  expect(api.loading.value).toBe(false)
  expect(api.content.value).not.toBe('late')
})

it('新增窗口右上角关闭能退出真实模态层，修改后放弃确认也能退出', async () => {
  const wrapper = mount(ComposeTab, {
    attachTo: document.body,
    props: {
      connection: { profileId: 'profile', sessionId: 'one', status: 'connected' },
      profileId: 'profile',
      workspaceId: 'modal-close',
    },
    global: { stubs: { ComposeContainers: true, UiCodeEditor: true } },
  })
  const add = async () => {
    wrapper
      .findAllComponents(UiButton)
      .find((b) => b.text() === '添加容器编排')!
      .vm.$emit('click')
    await flushPromises()
  }
  await flushPromises()
  await add()
  ;(
    document.querySelector('[role="dialog"] button[aria-label="关闭"]') as HTMLButtonElement
  ).click()
  await flushPromises()
  expect(wrapper.findComponent(ComposeCreateDialog).exists()).toBe(false)
  await add()
  wrapper
    .getComponent(ComposeCreateDialog)
    .findAllComponents(UiInput)[0]!
    .vm.$emit('update:modelValue', 'my-app')
  await flushPromises()
  ;(
    document.querySelector('[role="dialog"] button[aria-label="关闭"]') as HTMLButtonElement
  ).click()
  await flushPromises()
  const discard = [...document.querySelectorAll<HTMLButtonElement>('[role="dialog"] button')].find(
    (b) => b.textContent?.trim() === '放弃并关闭'
  )!
  expect(discard).toBeTruthy()
  discard.click()
  await flushPromises()
  expect(wrapper.findComponent(ComposeCreateDialog).exists()).toBe(false)
  expect(wrapper.emitted('state')?.at(-1)).toEqual([{ dirty: false, busy: false }])
})

it('目录选择通过上级图标和路径回车导航，根目录禁用上级', async () => {
  env.sshFileList.mockResolvedValueOnce({ ok: true, path: '/srv', parentPath: '/', files: [] })
  const wrapper = shallowMount(ComposeDirectoryPicker, {
    props: { connectionId: 'one', initialPath: '/srv' },
    global: { renderStubDefaultSlot: true, stubs: { UiIconButton: false } },
  })
  await flushPromises()
  expect(wrapper.getComponent(UiIconButton).getComponent(UiButton).props('disabled')).toBe(false)
  env.sshFileList.mockResolvedValueOnce({ ok: true, path: '/', files: [] })
  wrapper.getComponent(UiIconButton).vm.$emit('click')
  await flushPromises()
  expect(env.sshFileList).toHaveBeenLastCalledWith('one', '/')
  expect(wrapper.getComponent(UiIconButton).getComponent(UiButton).props('disabled')).toBe(true)
  env.sshFileList.mockResolvedValueOnce({ ok: true, path: '/tmp', parentPath: '/', files: [] })
  wrapper.getComponent(UiInput).vm.$emit('update:modelValue', '/tmp')
  await flushPromises()
  wrapper.getComponent(UiInput).vm.$emit('keydown', { key: 'Enter' })
  await flushPromises()
  expect(env.sshFileList).toHaveBeenLastCalledWith('one', '/tmp')
  expect(wrapper.findAllComponents(UiButton).some((b) => b.text() === '前往')).toBe(false)
})

it('列表行操作直接作用于该行项目，展示结果且不打开 YAML', async () => {
  const other = { ...project, name: 'other', configFiles: ['/srv/other.yml'], workingDir: '/srv' }
  env.sshComposeList.mockResolvedValue([project, other])
  env.sshComposeAction.mockResolvedValue({ exitCode: 0, stdout: 'stopped', stderr: '' })
  const wrapper = shallowMount(ComposeTab, {
    props: {
      connection: { profileId: 'profile', sessionId: 'one', status: 'connected' },
      profileId: 'profile',
      workspaceId: 'row-action',
    },
    global: {
      renderStubDefaultSlot: true,
      stubs: { ComposeProjectList: false, UiTableExpandableRow: false },
    },
  })
  await flushPromises()
  wrapper
    .getComponent(ComposeProjectList)
    .findAllComponents(UiButton)
    .filter((b) => b.text() === '停止')[1]!
    .vm.$emit('click')
  await flushPromises()
  expect(env.sshComposeStream.mock.calls[0][0]).toMatchObject({ project: other, action: 'stop' })
  expect(env.sshEditOpen).not.toHaveBeenCalled()
  expect(wrapper.findComponent(UiCodeEditor).exists()).toBe(false)
  expect(wrapper.text()).toContain('stopped')
})
