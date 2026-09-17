import { enableAutoUnmount, flushPromises, shallowMount } from '@vue/test-utils'
import { afterEach, expect, it } from 'vitest'
import { UiButton, UiInput, UiSelect } from '@/core/ui'
import ConfirmDialog from '@/core/ui/ConfirmDialog.vue'
import ComposeCreateDialog from './ComposeCreateDialog.vue'
import { composeTemplates } from './composeTemplates'
import { parseComposeContainers } from './composeContainers'
enableAutoUnmount(afterEach)

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
    .find((b) => b.text() === '保存并启动')!
    .vm.$emit('click')
  expect(wrapper.emitted('save')?.[0]).toEqual([
    'blog-two',
    '/srv/my apps/blog/docker-compose.yml',
    true,
    '/srv/my apps',
  ])
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

it('容器列表兼容数组和逐行 JSON，解析失败不伪装成空容器列表', () => {
  const row = {
    ID: 'abc',
    Name: 'blog-web-1',
    Service: 'web',
    State: 'running',
    Image: 'nginx',
    Publishers: [{ URL: '0.0.0.0', PublishedPort: 8080, TargetPort: 80, Protocol: 'tcp' }],
  }
  const result = parseComposeContainers(JSON.stringify([row]))
  expect(parseComposeContainers(`${JSON.stringify(row)}\n`)).toEqual(result)
  expect(result[0]).toMatchObject({ service: 'web', ports: '0.0.0.0:8080 → 80/tcp' })
  expect(() => parseComposeContainers('permission denied')).toThrow()
  expect(() => parseComposeContainers('[{}]')).toThrow()
})
