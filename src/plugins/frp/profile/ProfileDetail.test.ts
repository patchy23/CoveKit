/** 档案编辑与运行入口：防止旧配置校验、草稿切换丢失和报错进程失去停止入口。 */
import { flushPromises, mount } from '@vue/test-utils'
import { createPinia } from 'pinia'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { i18n } from '@/i18n'
import ProfileDetail from './ProfileDetail.vue'
import ProfileFormEditor from './ProfileFormEditor.vue'
import ProfileSourceEditor from './ProfileSourceEditor.vue'

const mocked = vi.hoisted(() => ({
  verify: vi.fn(),
  profileRead: vi.fn(),
  profileSaveForm: vi.fn(),
}))
vi.mock('../ipc', () => ({ ipc: mocked }))

async function setup(state?: { fileName: string; state: 'error'; pid: number }) {
  const wrapper = mount(ProfileDetail, {
    props: { fileName: 'a.toml', busy: false, logs: [], state },
    global: {
      plugins: [createPinia(), i18n],
      stubs: { ProfileFormEditor: true, ProfileSourceEditor: true, RuntimeLogPanel: true },
    },
  })
  await flushPromises()
  return wrapper
}

describe('FRP 档案详情操作边界', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    mocked.profileRead.mockResolvedValue({
      ok: true,
      content: "serverAddr = 'example.test'",
      parsed: { serverAddr: 'example.test' },
      hasComments: false,
    })
    mocked.profileSaveForm.mockResolvedValue({ ok: true })
    mocked.verify.mockResolvedValue({ ok: true, errors: [] })
  })

  it('未保存的凭证修改不会被模式切换丢弃，也不会校验或启动磁盘旧配置', async () => {
    const wrapper = await setup()
    const form = wrapper.findComponent(ProfileFormEditor)
    form.vm.$emit('update:modelValue', {
      ...form.props('modelValue'),
      authCredentialId: 'credential-id',
    })
    await flushPromises()
    const source = wrapper
      .findAll('button')
      .find((button) => button.text() === i18n.global.t('frp.modeSource'))!
    const verify = wrapper
      .findAll('button')
      .find((button) => button.text() === i18n.global.t('frp.actionVerify'))!
    expect(source.attributes('disabled')).toBeDefined()
    expect(verify.attributes('disabled')).toBeDefined()
    expect(
      wrapper.get(`button[aria-label="${i18n.global.t('frp.actionStart')}"]`).attributes('disabled')
    ).toBeDefined()
    await source.trigger('click')
    await verify.trigger('click')
    expect(wrapper.findComponent(ProfileFormEditor).exists()).toBe(true)
    expect(mocked.verify).not.toHaveBeenCalled()
    const save = wrapper
      .findAll('button')
      .find((button) => button.text() === i18n.global.t('frp.actionSave'))!
    await save.trigger('click')
    await flushPromises()
    expect(mocked.profileSaveForm).toHaveBeenCalledWith(
      'a.toml',
      expect.objectContaining({
        auth: {
          method: 'token',
          token: '{{ .Envs.COVEKIT_FRP_TOKEN_63726564656e7469616c2d6964 }}',
        },
      })
    )
    expect(source.attributes('disabled')).toBeUndefined()
    wrapper.unmount()
  })

  it('进程报错但尚未退出时仍能停止', async () => {
    const wrapper = await setup({ fileName: 'a.toml', state: 'error', pid: 123 })
    const stop = wrapper.get(`button[aria-label="${i18n.global.t('frp.actionStop')}"]`)
    expect(stop.attributes('disabled')).toBeUndefined()
    await stop.trigger('click')
    expect(wrapper.emitted('stop')).toEqual([['a.toml']])
    expect(wrapper.find(`button[aria-label="${i18n.global.t('frp.actionStart')}"]`).exists()).toBe(
      false
    )
    wrapper.unmount()
  })

  it('校验失败即使无错误明细也不能显示通过，编辑后清除旧结论', async () => {
    const wrapper = await setup()
    const source = wrapper
      .findAll('button')
      .find((button) => button.text() === i18n.global.t('frp.modeSource'))!
    await source.trigger('click')
    const verify = wrapper
      .findAll('button')
      .find((button) => button.text() === i18n.global.t('frp.actionVerify'))!
    mocked.verify.mockResolvedValueOnce({ ok: false, errors: [] })
    await verify.trigger('click')
    await flushPromises()
    const editor = wrapper.findComponent(ProfileSourceEditor)
    expect(editor.props('verified')).toBe(false)
    await verify.trigger('click')
    await flushPromises()
    expect(editor.props('verified')).toBe(true)
    editor.vm.$emit('update:modelValue', 'serverAddr = "changed"')
    await flushPromises()
    expect(editor.props('verified')).toBe(false)
    wrapper.unmount()
  })
})
