import { enableAutoUnmount, flushPromises, mount, shallowMount } from '@vue/test-utils'
import { defineComponent, h, ref } from 'vue'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import type { ComposeProject, ServerConnection } from '../contracts'
import { useCompose } from './useCompose'
import { COMPOSE_TEMPLATE, mergeComposeProjects, validateComposeDraft } from './composeProjects'
import ComposeTab from './ComposeTab.vue'
import { UiButton, UiCodeEditor } from '@/core/ui'
import ConfirmDialog from '@/core/ui/ConfirmDialog.vue'

const env = vi.hoisted(() => ({
  sshComposeList: vi.fn(),
  sshComposeAction: vi.fn(),
  sshComposeCreate: vi.fn(),
  sshEditOpen: vi.fn(),
  sshEditSave: vi.fn(),
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
      { name: 'new-app', status: '已保存 · 尚未部署', configFiles: ['/opt/new/compose.yaml'] },
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

it('编辑后的文件切换须确认，取消保留草稿并向连接页上报未保存状态', async () => {
  const wrapper = shallowMount(ComposeTab, {
    props: {
      connection: { profileId: 'profile', sessionId: 'one', status: 'connected' },
      profileId: 'profile',
      workspaceId: 'ui',
    },
    global: { renderStubDefaultSlot: true },
  })
  await flushPromises()
  const buttons = () => wrapper.findAllComponents(UiButton)
  buttons()
    .find((button) => button.text().includes('running(1)'))!
    .vm.$emit('click')
  await flushPromises()
  wrapper.getComponent(UiCodeEditor).vm.$emit('update:modelValue', 'unsaved draft')
  await flushPromises()
  buttons()
    .find((button) => button.text() === '新建 Compose 配置')!
    .vm.$emit('click')
  await flushPromises()
  const confirmation = wrapper
    .findAllComponents(ConfirmDialog)
    .find((dialog) => dialog.props('title') === '放弃未保存的修改')!
  expect(confirmation.props('open')).toBe(true)
  confirmation.vm.$emit('close')
  await flushPromises()
  expect(wrapper.getComponent(UiCodeEditor).props('modelValue')).toBe('unsaved draft')
  expect(wrapper.emitted('state')?.at(-1)).toEqual([{ dirty: true, busy: false }])
})
