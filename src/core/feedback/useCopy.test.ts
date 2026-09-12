/**
 * 复制反馈行为网（AR03 补证据）
 *
 * 用途：AR03 把 `core/ui/useClipboard` 拆成「平台调用 + 反馈」两层后，
 * 需要证明空值、成功、失败三种情形都有可见反馈（任务书 §4.2「取消与预期缺失不弹错误」
 * 「显式操作必须可感知结果」），且平台层不依赖任何应用服务。
 *
 * 手法：Pinia 独立测试实例 + 打桩 navigator.clipboard；断言 toast 文案而不是实现位置。
 */
import { describe, expect, it, beforeEach, afterEach, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { writeClipboardText } from '@/core/platform/clipboard'
import { useUiStore } from '@/stores/ui'
import { useCopy } from './useCopy'

describe('复制文本（平台层）', () => {
  afterEach(() => {
    vi.unstubAllGlobals()
  })

  it('空串按「无内容」返回，不当成错误', async () => {
    vi.stubGlobal('navigator', { clipboard: { writeText: vi.fn() } })
    expect(await writeClipboardText('')).toEqual({ ok: false, reason: 'empty' })
  })

  it('写入成功返回 ok', async () => {
    const writeText = vi.fn().mockResolvedValue(undefined)
    vi.stubGlobal('navigator', { clipboard: { writeText } })
    expect(await writeClipboardText('1521')).toEqual({ ok: true })
    expect(writeText).toHaveBeenCalledWith('1521')
  })

  it('平台拒绝或不可用时返回 failed，不抛给调用方', async () => {
    vi.stubGlobal('navigator', {
      clipboard: { writeText: vi.fn().mockRejectedValue(new Error('denied')) },
    })
    expect(await writeClipboardText('1521')).toEqual({ ok: false, reason: 'failed' })

    // 浏览器预览等场景下 navigator.clipboard 根本不存在，同样必须降级而不是冒泡异常
    vi.stubGlobal('navigator', {})
    expect(await writeClipboardText('1521')).toEqual({ ok: false, reason: 'failed' })
  })
})

describe('复制反馈（带提示）', () => {
  let ui: ReturnType<typeof useUiStore>

  beforeEach(() => {
    setActivePinia(createPinia())
    ui = useUiStore()
  })

  afterEach(() => {
    vi.unstubAllGlobals()
    vi.restoreAllMocks()
  })

  it('成功后提示传入的标签', async () => {
    vi.stubGlobal('navigator', { clipboard: { writeText: vi.fn().mockResolvedValue(undefined) } })
    const spy = vi.spyOn(ui, 'toast')
    await useCopy().copyText('1521', '端口已复制')
    expect(spy).toHaveBeenCalledWith('端口已复制')
  })

  it('空值提示「没有可复制的内容」，不提示成功也不报错', async () => {
    const spy = vi.spyOn(ui, 'toast')
    await useCopy().copyText('')
    expect(spy).toHaveBeenCalledWith('没有可复制的内容')
    expect(spy).not.toHaveBeenCalledWith('已复制')
  })

  it('平台失败给出可操作兜底文案', async () => {
    vi.stubGlobal('navigator', {})
    const spy = vi.spyOn(ui, 'toast')
    await useCopy().copyText('1521')
    expect(spy).toHaveBeenCalledWith('复制失败：请手动选择复制')
  })
})
