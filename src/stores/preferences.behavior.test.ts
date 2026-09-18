/**
 * 空间级用户数据（收藏、最近使用）路由行为网
 *
 * 用途：固定 AR06 W4「空间字段经设置服务按 schema 路由」的可观测行为：
 * 1 → 桌面环境读写经 Rust 设置服务（preferences_get / preferences_set），不再直写 covekit.json；
 * 2 → 浏览器预览环境才降级 storage 适配层（localStorage）；
 * 3 → 收藏写盘失败回滚到改动前的集合并暴露错误（界面不得显示成已收藏）；
 * 4 → 最近使用使用与 Rust 白名单一致的键名 recentTools，结构不符按空值处理。
 */
import { createPinia, setActivePinia } from 'pinia'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

/* ── IPC 门面假实现（vi.hoisted：mock 工厂先于被 mock 模块导入执行） ── */
const ipcMock = vi.hoisted(() => ({
  preferencesGet: vi.fn(),
  preferencesSet: vi.fn(),
}))

vi.mock('@/core/ipc/ipc', () => ({ ipc: ipcMock }))
vi.mock('@tauri-apps/plugin-store', () => ({ load: vi.fn() }))

import { isStringArray, spaceData } from '@/core/spaceData'
import { useFavoritesStore } from '@/stores/favorites'
import { useToolsStore } from '@/stores/tools'

/** 标记当前环境为 WebView（走 IPC 分支） */
function enterDesktop() {
  ;(window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {}
}

function leaveDesktop() {
  delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__
}

describe('空间级用户数据通道', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    localStorage.clear()
    enterDesktop()
  })

  afterEach(() => {
    leaveDesktop()
  })

  it('桌面环境读写走设置服务，不落本地文件', async () => {
    ipcMock.preferencesGet.mockResolvedValue(['ssh'])
    await expect(spaceData.get<string[]>('favorites', isStringArray)).resolves.toEqual(['ssh'])
    expect(ipcMock.preferencesGet).toHaveBeenCalledWith('favorites')

    await spaceData.set('recentTools', ['dns'])
    expect(ipcMock.preferencesSet).toHaveBeenCalledWith('recentTools', ['dns'])
    expect(localStorage.getItem('covekit.json:recentTools')).toBeNull()
  })

  it('结构不符按空值处理，不把坏数据当收藏', async () => {
    ipcMock.preferencesGet.mockResolvedValue('ssh')
    await expect(spaceData.get<string[]>('favorites', isStringArray)).resolves.toBeNull()
    ipcMock.preferencesGet.mockResolvedValue(['ssh', 3])
    await expect(spaceData.get<string[]>('favorites', isStringArray)).resolves.toBeNull()
  })

  it('浏览器预览环境降级 storage 适配层', async () => {
    leaveDesktop()
    await spaceData.set('favorites', ['ssh'])
    expect(localStorage.getItem('covekit.json:favorites')).toBe('["ssh"]')
    expect(ipcMock.preferencesSet).not.toHaveBeenCalled()
    await expect(spaceData.get<string[]>('favorites', isStringArray)).resolves.toEqual(['ssh'])
  })
})

describe('收藏与最近使用路由', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    vi.clearAllMocks()
    localStorage.clear()
    enterDesktop()
    ipcMock.preferencesGet.mockResolvedValue(null)
    ipcMock.preferencesSet.mockResolvedValue(undefined)
  })

  afterEach(() => {
    leaveDesktop()
  })

  it('收藏初始化与切换都经设置服务', async () => {
    ipcMock.preferencesGet.mockResolvedValue(['ssh'])
    const favorites = useFavoritesStore()
    await favorites.init()
    expect(favorites.ids).toEqual(['ssh'])

    await expect(favorites.toggle('dns')).resolves.toBe(true)
    expect(ipcMock.preferencesSet).toHaveBeenLastCalledWith('favorites', ['ssh', 'dns'])

    await expect(favorites.toggle('ssh')).resolves.toBe(false)
    expect(ipcMock.preferencesSet).toHaveBeenLastCalledWith('favorites', ['dns'])
  })

  it('收藏写盘失败回滚内存状态并暴露错误', async () => {
    const favorites = useFavoritesStore()
    await favorites.init()
    ipcMock.preferencesSet.mockRejectedValueOnce('磁盘只读')

    await expect(favorites.toggle('ssh')).rejects.toBe('磁盘只读')
    // 回滚：界面不能显示成已收藏，计数也不能被算进去
    expect(favorites.ids).toEqual([])
  })

  it('最近使用读写用 recentTools 键', async () => {
    ipcMock.preferencesGet.mockResolvedValue(['ssh', 'dns'])
    const tools = useToolsStore()
    await tools.initRecent()
    expect(tools.recent).toEqual(['ssh', 'dns'])
    expect(ipcMock.preferencesGet).toHaveBeenCalledWith('recentTools')

    await tools.pushRecent('frp')
    expect(tools.recent).toEqual(['frp', 'ssh', 'dns'])
    expect(ipcMock.preferencesSet).toHaveBeenLastCalledWith('recentTools', ['frp', 'ssh', 'dns'])
  })

  it('最近使用写盘失败不打断调用方，只在控制台记账', async () => {
    const tools = useToolsStore()
    await tools.initRecent()
    const logged = vi.spyOn(console, 'error').mockImplementation(() => undefined)
    ipcMock.preferencesSet.mockRejectedValueOnce('磁盘只读')

    await expect(tools.pushRecent('ssh')).resolves.toBeUndefined()
    expect(tools.recent).toEqual(['ssh'])
    expect(logged).toHaveBeenCalled()
    logged.mockRestore()
  })
})
