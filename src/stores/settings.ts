/**
 * 应用设置（Pinia）：主题 / 语言 / 自启 / 默认下载目录 / 工具级设置
 *
 * 保存语义（T07）：
 * - 桌面环境（WebView）只走 IPC：失败时把界面值回滚到磁盘上的旧值并暴露错误、不保留草稿，
 *   不把同一份设置写进 localStorage 伪装成功（那会造成两份配置各自为政）。
 * - 浏览器预览/单测等无容器环境才降级 localStorage。
 * - 保存按顺序串行：并发调用排队执行，后发起的请求不会因为先返回而覆盖新值。
 * - 改动带 revision：其他窗口已改过设置时拒绝本轮保存，重新拉取后由调用方重试。
 * - 工具级设置按 owner/key 粒度提交，不再回传整个 tools 对象。
 */
import { isTauri } from '@tauri-apps/api/core'
import { warn as logWarn } from '@tauri-apps/plugin-log'
import { useDataRefresh } from '@/core/dataTransfer/useDataRefresh'
import { defineStore } from 'pinia'
import { ref } from 'vue'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { ipc } from '@/core/ipc/ipc'
import type { AppSettings } from '@/core/ipc/contracts'
import { storage } from '@/core/storage'
import { setLocale } from '@/i18n'

const STORE_FILE = 'covekit.json'
const SETTINGS_KEY = 'settings'

/** 环境探测：只有 WebView 内才允许走 IPC（浏览器预览没有 Tauri 注入） */
const isDesktop = (): boolean => typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window

/** 默认值每次重新生成，避免调用方修改到共享对象 */
const createDefaults = (): AppSettings => ({
  theme: 'system',
  language: 'zh-CN',
  launchAtStartup: false,
  defaultDownloadDirectory: '',
  tools: {},
})

export const useSettingsStore = defineStore('settings', () => {
  const settings = ref<AppSettings>(createDefaults())
  const loaded = ref(false)
  useDataRefresh('settings.', init)
  /** 最近一次保存失败的原因（界面展示用；成功后清空） */
  const saveError = ref<string | null>(null)
  /** 服务端 revision：保存时回传，防止陈旧覆盖 */
  const revision = ref<number>(0)

  let mediaQuery: MediaQueryList | null = null
  let mediaHandler: ((event: MediaQueryListEvent) => void) | null = null
  /** 保存队列：保证按调用顺序落盘 */
  let saveQueue: Promise<void> = Promise.resolve()

  /** 应用主题到 <html data-theme>（@custom-variant dark 绑定）+ Windows 标题栏同步 */
  function applyTheme() {
    const { theme } = settings.value
    const dark =
      theme === 'dark' ||
      (theme === 'system' && window.matchMedia('(prefers-color-scheme: dark)').matches)
    document.documentElement.dataset.theme = dark ? 'dark' : ''
    if (isDesktop()) {
      getCurrentWindow()
        .setTheme(dark ? 'dark' : 'light')
        .catch((error) => {
          // 失败不静默：标题栏可能不跟随主题（capability 缺失或平台不支持）
          if (isTauri()) {
            void logWarn('原生标题栏主题同步失败 source=settings').catch(() => {
              console.warn('[diagnostics] 日志发送失败')
            })
          }
          console.warn('[theme] 原生标题栏主题同步失败', error)
        })
    }
  }

  /**
   * 订阅系统主题变化：theme=system 时跟随系统切换（不是只在初始化时判定一次）。
   * 重复启动只保留一个订阅，避免监听器堆积。
   */
  function subscribeSystemTheme() {
    if (mediaQuery) return
    mediaQuery = window.matchMedia('(prefers-color-scheme: dark)')
    mediaHandler = () => {
      if (settings.value.theme === 'system') applyTheme()
    }
    mediaQuery.addEventListener('change', mediaHandler)
  }

  /** 释放系统主题订阅（应用卸载/测试清理） */
  function unsubscribeSystemTheme() {
    if (mediaQuery && mediaHandler) mediaQuery.removeEventListener('change', mediaHandler)
    mediaQuery = null
    mediaHandler = null
  }

  async function init() {
    if (isDesktop()) {
      try {
        const remote = await ipc.settingsGet()
        revision.value = await ipc.settingsRevision()
        settings.value = { ...createDefaults(), ...remote }
        saveError.value = null
      } catch (error) {
        // 桌面环境读不到设置：保留默认值并暴露错误，不去读 localStorage 造一份假配置
        saveError.value = String(error)
      }
    } else {
      const local = await storage.get<Partial<AppSettings>>(STORE_FILE, SETTINGS_KEY)
      settings.value = { ...createDefaults(), ...(local ?? {}) }
    }
    applyTheme()
    setLocale(settings.value.language)
    subscribeSystemTheme()
    loaded.value = true
  }

  /** 按当前环境落盘一次（桌面走 IPC，预览走 localStorage） */
  async function persist(patch: Record<string, unknown>): Promise<void> {
    if (!isDesktop()) {
      await storage.set(STORE_FILE, SETTINGS_KEY, settings.value)
      return
    }
    const next = await ipc.settingsPatch(patch, revision.value)
    revision.value = next
  }

  /**
   * 保存设置：乐观更新 + 失败回滚；同一时刻只跑一个保存，按调用顺序执行。
   * 失败时保留错误信息，界面可提示重试（不写另一份配置伪装成功）。
   */
  async function set<K extends keyof AppSettings>(key: K, value: AppSettings[K]) {
    const previous = settings.value[key]
    settings.value[key] = value
    if (key === 'theme') applyTheme()
    if (key === 'language') setLocale(value as AppSettings['language'])
    const task = saveQueue.then(async () => {
      try {
        await persist({ [key as string]: value })
        saveError.value = null
      } catch (error) {
        // 回滚到上一版本：界面看到的与磁盘一致，用户可再次提交
        settings.value[key] = previous
        if (key === 'theme') applyTheme()
        saveError.value = String(error)
        throw error
      }
    })
    // 队列自身不因失败中断，错误交给调用方
    saveQueue = task.catch(() => undefined)
    return task
  }

  /** 保存最近一次失败原因（供界面提示） */
  function clearSaveError() {
    saveError.value = null
  }

  /* ── 工具级设置（settingsSchema 渲染 + 按 owner/key 粒度保存）── */

  function getToolSetting<T>(toolId: string, key: string, fallback: T): T {
    return (settings.value.tools[toolId]?.[key] as T | undefined) ?? fallback
  }

  async function setToolSetting(toolId: string, key: string, value: unknown) {
    const previous = settings.value.tools[toolId]?.[key]
    settings.value.tools = {
      ...settings.value.tools,
      [toolId]: { ...(settings.value.tools[toolId] ?? {}), [key]: value },
    }
    const task = saveQueue.then(async () => {
      try {
        if (isDesktop()) {
          revision.value = await ipc.settingsSetTool(toolId, key, value)
        } else {
          await storage.set(STORE_FILE, SETTINGS_KEY, settings.value)
        }
        saveError.value = null
      } catch (error) {
        const tools = { ...(settings.value.tools[toolId] ?? {}) }
        if (previous === undefined) delete tools[key]
        else tools[key] = previous
        settings.value.tools = { ...settings.value.tools, [toolId]: tools }
        saveError.value = String(error)
        throw error
      }
    })
    saveQueue = task.catch(() => undefined)
    return task
  }

  return {
    settings,
    loaded,
    saveError,
    revision,
    init,
    set,
    applyTheme,
    subscribeSystemTheme,
    unsubscribeSystemTheme,
    clearSaveError,
    getToolSetting,
    setToolSetting,
  }
})
