/**
 * 更新状态的行为测试（T11-2）
 *
 * 关键口径：离开设置页不再丢状态（store 持有）；进行中不重复起第二次检查/下载；
 * 下载与「待安装」可取消且会释放句柄；安装阶段不可取消；失败带稳定错误码并进错误清单。
 */
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'

/** 可控的更新句柄替身 */
const updateHandle = {
  version: '0.2.0',
  body: '修复连接恢复问题',
  close: vi.fn(async () => {}),
  download: vi.fn(async (onEvent: (event: unknown) => void) => {
    onEvent({ event: 'Started', data: { contentLength: 1000 } })
    onEvent({ event: 'Progress', data: { chunkLength: 400 } })
  }),
  install: vi.fn(async () => {}),
}
const check = vi.fn(async () => updateHandle)
const relaunch = vi.fn(async () => {})

vi.mock('@tauri-apps/plugin-updater', () => ({ check: () => check() }))
vi.mock('@tauri-apps/plugin-process', () => ({ relaunch: () => relaunch() }))

const updateAvailability = vi.fn(async () => ({ available: true, reason: '' }))
vi.mock('@/core/ipc/ipc', () => ({
  ipc: { updateAvailability: () => updateAvailability() },
}))

import { useUpdateStore } from '@/stores/update'
import { listErrors } from '@/core/diagnostics'

describe('更新状态', () => {
  it('尚未加载可用性时直接检查也不能绕过停用通道', async () => {
    updateAvailability.mockResolvedValueOnce({ available: false, reason: 'Alpha 手动下载' })
    const store = useUpdateStore()
    await store.checkNow()
    expect(store.phase).toBe('unavailable')
    expect(check).not.toHaveBeenCalled()
  })

  beforeEach(() => {
    // 更新能力只在桌面环境存在：测试里显式声明，否则 store 会判定为不支持而跳过全部动作
    ;(window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {}
    setActivePinia(createPinia())
    check.mockClear()
    updateHandle.close.mockClear()
    updateHandle.download.mockClear()
    updateHandle.install.mockClear()
    relaunch.mockClear()
    updateAvailability.mockClear()
  })

  it('检查到新版本后进入 available，并记住版本号', async () => {
    const store = useUpdateStore()

    await store.checkNow()

    expect(store.phase).toBe('available')
    expect(store.version).toBe('0.2.0')
    expect(store.releaseNotes).toBe('修复连接恢复问题')
  })

  it('进行中重复点击不会起第二次检查', async () => {
    const store = useUpdateStore()
    const first = store.checkNow()
    const second = store.checkNow()

    await Promise.all([first, second])

    expect(check).toHaveBeenCalledTimes(1)
  })

  it('重新检查失败时不残留上一版本的更新说明', async () => {
    const store = useUpdateStore()
    await store.checkNow()
    check.mockRejectedValueOnce(new Error('网络不可达'))
    await store.checkNow()
    expect(store.phase).toBe('error')
    expect(store.version).toBe('')
    expect(store.releaseNotes).toBe('')
  })

  it('下载累计进度，完成后进入 ready；取消会释放句柄并丢弃进度', async () => {
    const store = useUpdateStore()
    await store.checkNow()

    await store.download()
    expect(store.phase).toBe('ready')
    expect(store.downloaded).toBe(400)
    expect(store.percent).toBe(40)

    await store.cancel()
    expect(updateHandle.close).toHaveBeenCalledTimes(1)
    expect(store.phase).toBe('idle')
    expect(store.downloaded).toBe(0)
    expect(store.releaseNotes).toBe('')
  })

  it('安装阶段不可取消，取消调用被忽略', async () => {
    const store = useUpdateStore()
    await store.checkNow()
    await store.download()

    const installing = store.install()
    expect(store.phase).toBe('installing')
    expect(store.canCancel).toBe(false)

    await store.cancel()
    expect(store.phase).toBe('installing')

    await installing
    expect(updateHandle.install).toHaveBeenCalledTimes(1)
    expect(relaunch).toHaveBeenCalledTimes(1)
  })

  it('检查失败带稳定错误码并进错误清单', async () => {
    check.mockRejectedValueOnce(new Error('网络不可达'))
    const store = useUpdateStore()

    await store.checkNow()

    expect(store.phase).toBe('error')
    expect(store.errorCode).toBe('update.check_failed')
    expect(listErrors().some((entry) => entry.code === 'update.check_failed')).toBe(true)
  })

  it('更新通道不可用时进入 unavailable 并保留原因', async () => {
    updateAvailability.mockResolvedValueOnce({ available: false, reason: '公钥仍是占位值' })
    const store = useUpdateStore()

    await store.ensureAvailability()

    expect(store.phase).toBe('unavailable')
    expect(store.unavailableReason).toBe('公钥仍是占位值')
    expect(store.canCheck).toBe(false)
    await store.checkNow()
    expect(check).not.toHaveBeenCalled()
  })
})
