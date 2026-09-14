/**
 * BinarySetupCard：装好了但未强校验时，提示必须出现在界面上
 *
 * 回归：后端把「没能与上游 checksums 比对」的原因放在 `error` 字段里返回，
 * 前端曾经整条丢掉——用户既看不到提示，也无从判断这次安装是否被校验过。
 * 用例走完整链路：点卡片上的下载按钮 → 真实 `useFrpBinary` → 假 IPC 返回带 error 的结果。
 */
import { mount } from '@vue/test-utils'
import { createPinia } from 'pinia'
import { nextTick } from 'vue'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { i18n } from '@/i18n'
import BinarySetupCard from './BinarySetupCard.vue'

const downloadResult = vi.hoisted(() => ({ value: {} as Record<string, unknown> }))

vi.mock('../ipc', () => ({
  ipc: {
    binaryDetect: vi.fn().mockResolvedValue({ ok: false }),
    binaryVersions: vi.fn().mockResolvedValue([{ version: '0.70.1', publishedAt: '2025-05-01' }]),
    binaryDownload: vi.fn(() => Promise.resolve(downloadResult.value)),
  },
}))

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}))

vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }))

/** 等异步加载与随后的一轮渲染落定 */
async function settle() {
  await new Promise((resolve) => setTimeout(resolve, 0))
  await nextTick()
}

/** 挂载引导卡（应用的真实 pinia / i18n，其余外部依赖走上面的假实现） */
async function mountAndDownload(): Promise<ReturnType<typeof mount>> {
  const wrapper = mount(BinarySetupCard, {
    props: { detected: null },
    global: { plugins: [createPinia(), i18n] },
  })
  await settle()
  const button = wrapper.findAll('button').find((item) => item.text() === '下载并校验')
  expect(button, '下载按钮应可用').toBeTruthy()
  await button?.trigger('click')
  await settle()
  return wrapper
}

describe('BinarySetupCard 下载结果提示', () => {
  beforeEach(() => {
    downloadResult.value = {}
  })

  it('未强校验时把后端说明显示在卡片上', async () => {
    const reason = '上游未提供 frp_sha256_checksums.txt（版本 0.40.0 可能较旧），未强校验'
    downloadResult.value = { ok: true, path: 'C:/x/frpc-0.40.0.exe', error: reason }

    const wrapper = await mountAndDownload()

    expect(wrapper.text()).toContain('未通过 SHA256 强校验')
    expect(wrapper.text()).toContain(reason)
  })

  it('强校验通过时不出现提示', async () => {
    downloadResult.value = { ok: true, path: 'C:/x/frpc-0.70.1.exe' }

    const wrapper = await mountAndDownload()

    expect(wrapper.text()).not.toContain('未通过 SHA256 强校验')
  })

  it('下载失败走失败提示，不混入警告文案', async () => {
    downloadResult.value = { ok: false, error: 'SHA256 校验不通过，已丢弃下载文件' }

    const wrapper = await mountAndDownload()

    expect(wrapper.text()).toContain('SHA256 校验不通过，已丢弃下载文件')
    expect(wrapper.text()).not.toContain('未通过 SHA256 强校验')
  })
})
