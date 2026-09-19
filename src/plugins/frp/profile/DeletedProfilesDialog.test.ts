/** 恢复成功才刷新列表，冲突或 IO 失败保留可重试条目。 */
import { flushPromises, mount } from '@vue/test-utils'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { i18n } from '@/i18n'
import DeletedProfilesDialog from './DeletedProfilesDialog.vue'

const mocked = vi.hoisted(() => ({ profilesDeleted: vi.fn(), profileRestore: vi.fn() }))
vi.mock('../ipc', () => ({ ipc: mocked }))

async function setup() {
  const wrapper = mount(DeletedProfilesDialog, {
    props: { open: true },
    global: { plugins: [i18n], stubs: { UiModal: { template: '<div><slot /></div>' } } },
  })
  await flushPromises()
  return wrapper
}

describe('恢复已删除配置', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    mocked.profilesDeleted.mockResolvedValue([
      { trashName: 'test.toml.123', fileName: 'test.toml', deletedAt: 123, managed: true },
    ])
    mocked.profileRestore.mockResolvedValue({ ok: true })
  })

  it('成功后移除回收站条目并通知父级刷新', async () => {
    const wrapper = await setup()
    await wrapper.get('button').trigger('click')
    await flushPromises()
    expect(mocked.profileRestore).toHaveBeenCalledWith('test.toml.123', true)
    expect(wrapper.emitted('restored')).toHaveLength(1)
    expect(wrapper.text()).not.toContain('test.toml')
    wrapper.unmount()
  })

  it('同名冲突保留条目和明确错误，不报告恢复成功', async () => {
    mocked.profileRestore.mockRejectedValue(new Error('档案名称已存在'))
    const wrapper = await setup()
    await wrapper.get('button').trigger('click')
    await flushPromises()
    expect(wrapper.text()).toContain('档案名称已存在')
    expect(wrapper.text()).toContain('test.toml')
    expect(wrapper.emitted('restored')).toBeUndefined()
    expect(wrapper.get('button').attributes('disabled')).toBeUndefined()
    wrapper.unmount()
  })

  it('读取失败显示原因，不误显示回收站为空', async () => {
    mocked.profilesDeleted.mockRejectedValue(new Error('读取失败'))
    const wrapper = await setup()
    expect(wrapper.text()).toContain('读取失败')
    expect(wrapper.text()).not.toContain(i18n.global.t('frp.deletedEmpty'))
    wrapper.unmount()
  })
})
