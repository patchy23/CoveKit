/**
 * 设置与持久化适配层行为网
 *
 * 用途：把 T07 里「用户可感知」的保存与容错行为固化下来：
 * 1 → 桌面环境保存失败时回滚到上一版本并暴露错误，不写另一份配置伪装成功；
 * 2 → 并发保存按调用顺序串行执行，后发起的改动不会被先返回的旧请求覆盖；
 * 3 → 浏览器预览环境才降级 localStorage；
 * 4 → 工具级设置按 owner/key 提交，不整对象覆盖；
 * 5 → 持久化缓存按文件名复用 Promise，失败后清缓存可重试；
 * 6 → 损坏的 JSON 先隔离备份再按空值处理，不覆盖原内容。
 */
import { createPinia, setActivePinia } from 'pinia'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

/* ── IPC 门面假实现（vi.hoisted：mock 工厂先于被 mock 模块导入执行） ── */
const ipcMock = vi.hoisted(() => ({
  settingsGet: vi.fn(),
  settingsRevision: vi.fn(),
  settingsPatch: vi.fn(),
  settingsSetTool: vi.fn(),
  setTheme: vi.fn(),
}))

vi.mock('@/core/ipc/ipc', () => ({ ipc: ipcMock }))
vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => ({ setTheme: ipcMock.setTheme }),
}))
vi.mock('@tauri-apps/plugin-store', () => ({
  load: vi.fn(),
}))

import { load } from '@tauri-apps/plugin-store'
import { storage } from '@/core/storage'
import { useSettingsStore } from '@/stores/settings'

/** 标记当前环境为 WebView（走 IPC 分支） */
function enterDesktop() {
  ;(window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {}
}

function leaveDesktop() {
  delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__
}

describe('设置保存行为', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    vi.clearAllMocks()
    enterDesktop()
    ipcMock.settingsGet.mockResolvedValue({
      theme: 'light',
      language: 'zh-CN',
      launchAtStartup: false,
      defaultDownloadDirectory: '',
      tools: {},
    })
    ipcMock.settingsRevision.mockResolvedValue(3)
    ipcMock.setTheme.mockResolvedValue(undefined)
    ipcMock.settingsPatch.mockResolvedValue(4)
    ipcMock.settingsSetTool.mockResolvedValue(5)
    vi.stubGlobal('matchMedia', () => ({
      matches: false,
      addEventListener: () => undefined,
      removeEventListener: () => undefined,
    }))
  })

  afterEach(() => {
    leaveDesktop()
    vi.unstubAllGlobals()
  })

  it('保存失败时回滚到上一版本并暴露错误', async () => {
    const store = useSettingsStore()
    await store.init()
    ipcMock.settingsPatch.mockRejectedValueOnce('磁盘只读')

    await expect(store.set('theme', 'dark')).rejects.toBeTruthy()
    expect(store.settings.theme).toBe('light')
    expect(store.saveError).toBe('磁盘只读')
    // 不写 localStorage 伪装成功
    expect(localStorage.length).toBe(0)
  })

  it('并发保存按调用顺序执行且丢失的最新值不覆盖', async () => {
    const store = useSettingsStore()
    await store.init()
    const order: string[] = []
    ipcMock.settingsPatch.mockImplementation(async (patch: Record<string, unknown>) => {
      order.push(Object.values(patch)[0] as string)
      return order.length
    })

    await Promise.all([
      store.set('theme', 'dark'),
      store.set('defaultDownloadDirectory', 'D:/downloads'),
    ])
    expect(order).toEqual(['dark', 'D:/downloads'])
    expect(store.settings.theme).toBe('dark')
    expect(store.settings.defaultDownloadDirectory).toBe('D:/downloads')
    // revision 跟随每次保存返回推进
    expect(store.revision).toBe(2)
  })

  it('工具级设置按 owner 与键提交，不发送整个 tools 对象', async () => {
    const store = useSettingsStore()
    await store.init()
    await store.setToolSetting('dns', 'platform', 'dnspod')
    expect(ipcMock.settingsSetTool).toHaveBeenCalledWith('dns', 'platform', 'dnspod')
    expect(ipcMock.settingsPatch).not.toHaveBeenCalled()
  })

  it('浏览器预览环境降级 localStorage', async () => {
    leaveDesktop()
    const store = useSettingsStore()
    await store.init()
    await store.set('theme', 'dark')
    expect(ipcMock.settingsPatch).not.toHaveBeenCalled()
    expect(localStorage.getItem('covekit.json:settings')).toContain('dark')
  })
})

describe('持久化适配层', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    storage.resetCache()
    localStorage.clear()
    leaveDesktop()
  })

  it('缓存按文件名复用 Promise：并发读取只加载一次', async () => {
    enterDesktop()
    const store = { get: vi.fn().mockResolvedValue({ a: 1 }) }
    const loader = vi.mocked(load)
    loader.mockResolvedValue(store as never)

    const [first, second] = await Promise.all([
      storage.get('a.json', 'k'),
      storage.get('a.json', 'k'),
    ])
    expect(first).toEqual({ a: 1 })
    expect(second).toEqual({ a: 1 })
    expect(loader).toHaveBeenCalledTimes(1)

    // 另一个文件各自加载，互不影响
    loader.mockResolvedValue({ get: vi.fn().mockResolvedValue(null) } as never)
    await storage.get('b.json', 'k')
    expect(loader).toHaveBeenCalledTimes(2)
    leaveDesktop()
  })

  it('加载失败后清缓存：下次读取会重试', async () => {
    enterDesktop()
    const loader = vi.mocked(load)
    loader.mockRejectedValueOnce(new Error('文件损坏'))
    await expect(storage.get('c.json', 'k')).rejects.toThrow('文件损坏')

    const store = { get: vi.fn().mockResolvedValue({ ok: true }) }
    loader.mockResolvedValue(store as never)
    await expect(storage.get('c.json', 'k')).resolves.toEqual({ ok: true })
    expect(loader).toHaveBeenCalledTimes(2)
    leaveDesktop()
  })

  it('损坏 JSON 先隔离备份再返回空值，不覆盖原内容', async () => {
    localStorage.setItem('d.json:k', '{ 坏数据')
    const value = await storage.get('d.json', 'k')
    expect(value).toBeNull()
    expect(localStorage.getItem('d.json:k')).toBeNull()
    expect(localStorage.getItem('d.json:k:corrupt')).toBe('{ 坏数据')
  })

  it('结构校验不通过同样隔离备份', async () => {
    localStorage.setItem('e.json:k', JSON.stringify({ wrong: true }))
    const value = await storage.get('e.json', 'k', (raw) => {
      return typeof raw === 'object' && raw !== null && 'expected' in raw
    })
    expect(value).toBeNull()
    expect(localStorage.getItem('e.json:k:corrupt')).toContain('wrong')
  })
})
