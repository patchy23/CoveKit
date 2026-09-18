import { enableAutoUnmount, mount, shallowMount } from '@vue/test-utils'
import { afterEach, expect, it, vi } from 'vitest'
import { ref } from 'vue'
import ServerList from './ServerList.vue'
import ServerForm from './ServerForm.vue'
import ContextMenu from '@/core/ui/ContextMenu.vue'
import { UiButton, UiInput, UiSelect } from '@/core/ui'

vi.mock('./useServerCredentialChoice', () => ({
  useServerCredentialChoice: () => ({
    credentialsFailed: ref(false),
    credFormOpen: ref(false),
    credentialOptions: ref([]),
    selectedCredential: ref(null),
    credentialMissing: ref(false),
    onCredentialSelect: vi.fn(),
    onCredentialSaved: vi.fn(),
  }),
}))
enableAutoUnmount(afterEach)
const groups = [{ id: 'group-1', name: '生产环境', sortOrder: 0 }]
const profile = {
  id: 'profile-1',
  name: '服务器',
  host: 'example.test',
  port: 22,
  username: 'root',
  authMethod: 'password' as const,
  groupId: 'group-1',
}
const modalStub = {
  props: ['open'],
  template: '<div v-if="open"><slot /><slot name="footer" /></div>',
}

it('空白处提供新建入口，分组和服务器菜单不会被冒泡覆盖', async () => {
  const wrapper = mount(ServerList, {
    props: { profiles: [profile], groups, expandedIds: new Set(['group-1']), searchKeyword: '' },
    global: { stubs: { UiModal: modalStub, ConfirmDialog: true, ContextMenu: true } },
  })
  await wrapper.get('[data-scroll-axis="vertical"]').trigger('contextmenu')
  let items = wrapper.getComponent(ContextMenu).props('items')
  expect(items.map((item) => item.label)).toEqual(['添加服务器', '新建分组'])
  items[0].onClick?.()
  expect(wrapper.emitted('add')?.[0]).toEqual([])
  items[1].onClick?.()
  await wrapper.vm.$nextTick()
  expect(wrapper.find('input[placeholder="分组名称，如：生产环境"]').exists()).toBe(true)

  await wrapper.get('[data-group-drop="group-1"]').trigger('contextmenu')
  items = wrapper.getComponent(ContextMenu).props('items')
  items.find((item) => item.label === '添加服务器')?.onClick?.()
  expect(wrapper.emitted('add')?.[1]).toEqual(['group-1'])
  const row = wrapper.findAll('span').find((span) => span.text() === '服务器')!
  await row.trigger('contextmenu')
  expect(
    wrapper
      .getComponent(ContextMenu)
      .props('items')
      .map((item) => item.label)
  ).toContain('编辑')
})

it.each([null, 'group-1'])('添加表单默认分组为 %s，允许修改和清空并提交', async (groupId) => {
  const wrapper = shallowMount(ServerForm, {
    props: { profile: null, groups, defaultGroupId: groupId },
    global: { renderStubDefaultSlot: true, stubs: { UiModal: modalStub } },
  })
  const select = wrapper
    .findAllComponents(UiSelect)
    .find((item) => item.props('options').some((option) => option.label === '未分组'))!
  expect(select.props('modelValue')).toBe(groupId ?? '__ungrouped__')
  const auth = wrapper
    .findAllComponents(UiSelect)
    .find((item) => item.props('options').some((option) => option.value === 'credential'))!
  expect(auth.props('modelValue')).toBe('credential')
  for (const [placeholder, value] of [
    ['如：生产服务器', '新增主机'],
    ['192.168.1.1', 'example.test'],
    ['root', 'root'],
  ]) {
    wrapper
      .findAllComponents(UiInput)
      .find((item) => item.attributes('placeholder') === placeholder)!
      .vm.$emit('update:modelValue', value)
  }
  const save = () =>
    wrapper
      .findAllComponents(UiButton)
      .find((item) => item.text() === '保存')!
      .vm.$emit('click')
  save()
  expect(wrapper.emitted('save')).toBeUndefined()
  expect(wrapper.emitted('error')?.[0]).toEqual(['请选择凭证（或从下拉末尾新建）'])
  auth.vm.$emit('update:modelValue', 'password')
  select.vm.$emit('update:modelValue', 'group-1')
  save()
  expect(wrapper.emitted('save')?.[0]?.[0]).toMatchObject({ groupId: 'group-1' })
  expect(wrapper.emitted('save')?.[0]?.slice(2)).toEqual([false, true])
  select.vm.$emit('update:modelValue', '__ungrouped__')
  save()
  expect((wrapper.emitted('save')?.[1]?.[0] as { groupId?: string }).groupId).toBeUndefined()
})

it('编辑服务器保留已有分组和手工认证方式，忽略添加默认值', () => {
  const wrapper = shallowMount(ServerForm, {
    props: { profile, groups, defaultGroupId: 'another-group' },
    global: { renderStubDefaultSlot: true, stubs: { UiModal: modalStub } },
  })
  const auth = wrapper
    .findAllComponents(UiSelect)
    .find((item) => item.props('options').some((option) => option.value === 'credential'))!
  expect(auth.props('modelValue')).toBe('password')
  wrapper
    .findAllComponents(UiButton)
    .find((item) => item.text() === '保存')!
    .vm.$emit('click')
  expect(wrapper.emitted('save')?.[0]?.[0]).toMatchObject({ id: profile.id, groupId: 'group-1' })
})
