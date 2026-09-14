/**
 * 剪贴板平台层行为网（真机走查回归）
 *
 * 用途：桌面容器里 `navigator.clipboard.writeText` 会触发 WebView2 的浏览器式权限提示，
 * 用户未授权前复制不生效；桌面路径必须走原生剪贴板插件，失败仍要如实返回。
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

const { writeText } = vi.hoisted(() => ({ writeText: vi.fn() }))
vi.mock('@tauri-apps/plugin-clipboard-manager', () => ({ writeText }))

describe('剪贴板（桌面容器）', () => {
  beforeEach(() => {
    writeText.mockReset()
    ;(window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {}
  })

  afterEach(() => {
    delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__
  })

  it('桌面容器走原生剪贴板，不依赖网页权限', async () => {
    writeText.mockResolvedValue(undefined)
    const { writeClipboardText } = await import('@/core/platform/clipboard')
    expect(await writeClipboardText('诊断报告')).toEqual({ ok: true })
    expect(writeText).toHaveBeenCalledWith('诊断报告')
  })

  it('原生写入被拒绝时返回 failed，不冒泡给调用方', async () => {
    writeText.mockRejectedValue(new Error('clipboard denied'))
    const { writeClipboardText } = await import('@/core/platform/clipboard')
    expect(await writeClipboardText('诊断报告')).toEqual({ ok: false, reason: 'failed' })
  })

  it('空串仍按「无内容」返回，不触碰平台 API', async () => {
    const { writeClipboardText } = await import('@/core/platform/clipboard')
    expect(await writeClipboardText('')).toEqual({ ok: false, reason: 'empty' })
    expect(writeText).not.toHaveBeenCalled()
  })
})
