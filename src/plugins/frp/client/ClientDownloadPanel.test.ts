/** 历史版本下载不依赖最近发行列表，且必须使用用户选定的完整版本号。 */
import { flushPromises, mount } from '@vue/test-utils'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { i18n } from '@/i18n'
import { UiButton, UiInput, UiSelect } from '@/core/ui'
import ClientDownloadPanel from './ClientDownloadPanel.vue'

const mocked = vi.hoisted(() => ({ binaryVersions: vi.fn(), binaryDownload: vi.fn() }))
vi.mock('../ipc', () => ({ ipc: mocked }))
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn().mockResolvedValue(() => {}) }))

async function setup() {
  const wrapper = mount(ClientDownloadPanel, {
    global: { plugins: [i18n], stubs: { DownloadProgress: true } },
  })
  await flushPromises()
  return wrapper
}

function downloadButton(wrapper: Awaited<ReturnType<typeof setup>>) {
  return wrapper
    .findAllComponents(UiButton)
    .find((button) => button.text() === i18n.global.t('frp.binaryDownloadButton'))!
}

describe('FRP 历史客户端下载', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    mocked.binaryVersions.mockResolvedValue([
      { version: '0.65.0', publishedAt: '2025-09-25', assets: [] },
    ])
    mocked.binaryDownload.mockResolvedValue({ ok: true })
  })

  it('最近列表没有 0.60 时仍能指定下载，并报告实际安装版本', async () => {
    const wrapper = await setup()
    wrapper.findComponent(UiSelect).vm.$emit('update:modelValue', 'custom')
    await flushPromises()
    wrapper.findComponent(UiInput).vm.$emit('update:modelValue', ' v0.60.0 ')
    await flushPromises()
    await downloadButton(wrapper).find('button').trigger('click')
    await flushPromises()
    expect(mocked.binaryDownload).toHaveBeenCalledWith('0.60.0')
    expect(wrapper.emitted('installed')).toEqual([['0.60.0']])
    wrapper.unmount()
  })

  it('版本列表查询失败仍可下载历史版本，并显示旧版缺少校验的提示', async () => {
    mocked.binaryVersions.mockRejectedValue(new Error('API unavailable'))
    mocked.binaryDownload.mockResolvedValue({ ok: true, error: '未强校验' })
    const wrapper = await setup()
    expect(wrapper.text()).toContain('API unavailable')
    await downloadButton(wrapper).find('button').trigger('click')
    await flushPromises()
    expect(mocked.binaryDownload).toHaveBeenCalledWith('0.60.0')
    expect(wrapper.text()).toContain('未强校验')
    wrapper.unmount()
  })

  it('拒绝不完整版本号和路径输入', async () => {
    const wrapper = await setup()
    wrapper.findComponent(UiSelect).vm.$emit('update:modelValue', 'custom')
    await flushPromises()
    for (const value of ['', '0.60', '../0.60.0', '0.60.0/extra']) {
      wrapper.findComponent(UiInput).vm.$emit('update:modelValue', value)
      await flushPromises()
      expect(downloadButton(wrapper).props('disabled')).toBe(true)
      await downloadButton(wrapper).find('button').trigger('click')
    }
    expect(mocked.binaryDownload).not.toHaveBeenCalled()
    wrapper.unmount()
  })
})
