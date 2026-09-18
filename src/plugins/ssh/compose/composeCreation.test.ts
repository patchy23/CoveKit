import { enableAutoUnmount, flushPromises, shallowMount } from '@vue/test-utils'
import { afterEach, expect, it } from 'vitest'
import { UiButton, UiInput, UiSelect, UiModal } from '@/core/ui'
import ConfirmDialog from '@/core/ui/ConfirmDialog.vue'
import ComposeCreateDialog from './ComposeCreateDialog.vue'
import { composeTemplates, composeContainerCount } from './composeTemplates'
enableAutoUnmount(afterEach)

it('列表合计运行与停止容器数，未知状态不伪装成零', () => {
  expect(composeContainerCount('running(2), exited(1)')).toBe(3)
  expect(composeContainerCount('未部署')).toBe(0)
  expect(composeContainerCount('unknown')).toBeNull()
})

function dialog(defaultDirectory = async () => '/home/user/compose') {
  return shallowMount(ComposeCreateDialog, {
    props: {
      connectionId: 'one',
      connected: true,
      busy: false,
      error: '',
      content: composeTemplates[1]!.content,
      defaultDirectory,
    },
    global: {
      stubs: { UiModal: { template: '<div><slot/><slot name="footer"/></div>' } },
      renderStubDefaultSlot: true,
    },
  })
}

it('新建预览 POSIX 路径，显式修改项目目录后改名称不覆盖目录', async () => {
  const wrapper = dialog()
  await flushPromises()
  const inputs = wrapper.findAllComponents(UiInput)
  inputs[0]!.vm.$emit('update:modelValue', 'blog')
  await flushPromises()
  expect(wrapper.text()).toContain('/home/user/compose/blog/docker-compose.yml')
  inputs[1]!.vm.$emit('update:modelValue', '/srv/my apps/blog')
  inputs[0]!.vm.$emit('update:modelValue', 'blog-two')
  await flushPromises()
  wrapper
    .findAllComponents(UiButton)
    .find((b) => b.text() === '保存')!
    .vm.$emit('click')
  expect(wrapper.emitted('save')?.[0]).toEqual([
    'blog-two',
    '/srv/my apps/blog/docker-compose.yml',
    '/srv/my apps',
  ])
})

it('默认模板未修改时关闭直接生效，填写后确认放弃才关闭', async () => {
  const wrapper = dialog()
  await flushPromises()
  wrapper.getComponent(UiModal).vm.$emit('close')
  expect(wrapper.emitted('close')).toHaveLength(1)
  wrapper.findAllComponents(UiInput)[0]!.vm.$emit('update:modelValue', 'new-app')
  await flushPromises()
  wrapper.getComponent(UiModal).vm.$emit('close')
  await flushPromises()
  const confirmation = wrapper
    .findAllComponents(ConfirmDialog)
    .find((c) => c.props('title') === '放弃新增编排')!
  expect(confirmation.props('open')).toBe(true)
  expect(wrapper.emitted('close')).toHaveLength(1)
  confirmation.vm.$emit('confirm')
  expect(wrapper.emitted('close')).toHaveLength(2)
  expect(wrapper.text()).not.toContain('保存并启动')
  expect(wrapper.text()).not.toContain('打开已有文件')
})

it('远程主目录迟到响应不覆盖用户手填的目录，空目录不能保存到根目录', async () => {
  let finish!: (path: string) => void
  const wrapper = dialog(
    () =>
      new Promise((resolve) => {
        finish = resolve
      })
  )
  const inputs = wrapper.findAllComponents(UiInput)
  inputs[0]!.vm.$emit('update:modelValue', 'blog')
  inputs[1]!.vm.$emit('update:modelValue', '/srv/blog')
  finish('/home/user/compose')
  await flushPromises()
  expect(wrapper.text()).toContain('/srv/blog/docker-compose.yml')
  inputs[1]!.vm.$emit('update:modelValue', '')
  await flushPromises()
  wrapper
    .findAllComponents(UiButton)
    .find((b) => b.text() === '保存')!
    .vm.$emit('click')
  await flushPromises()
  expect(wrapper.emitted('save')).toBeUndefined()
  expect(wrapper.text()).toContain('请填写远程项目目录')
})

it('有 YAML 草稿时切换模板需要确认，取消不替换内容', async () => {
  const wrapper = dialog()
  await wrapper.setProps({ content: 'services: { mine: {} }' })
  wrapper.getComponent(UiSelect).vm.$emit('update:modelValue', 'redis')
  await flushPromises()
  expect(wrapper.getComponent(ConfirmDialog).props('open')).toBe(true)
  expect(wrapper.emitted('update:content')).toBeUndefined()
  wrapper.getComponent(ConfirmDialog).vm.$emit('close')
  await flushPromises()
  expect(wrapper.emitted('update:content')).toBeUndefined()
})
