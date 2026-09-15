/**
 * 空间级用户数据通道（收藏、最近使用）
 *
 * 为什么单独一层：这两项是**用户数据**，与设置一样随空间隔离，
 * 但写入不该递增设置版本号（收藏一下就让设置页缓存的版本号过期，
 * 会导致随后一次设置保存被误判为陈旧而拒绝重试）。
 *
 * 桌面环境：经 Rust 设置服务（`preferences_get` / `preferences_set`）落当前空间偏好文件，
 * 前端不再直接写 `patchybox.json` —— 见 ADR 与 AR06 §9.1「所有桌面持久写入经 Rust 应用入口」。
 * 浏览器预览：降级到 storage 适配层（localStorage），保持无容器环境可独立开发。
 *
 * 隔离语义：读取不做设备层回落。旧 `patchybox.json` 里的收藏与最近使用只属于默认空间，
 * 若回落，非默认空间会读到别处的数据。
 */
import { ipc } from '@/core/ipc/ipc'
import type { SpaceDataKey } from '@/core/ipc/contracts'
import { storage } from '@/core/storage'

/** 浏览器预览下使用的降级文件（桌面环境不写这个文件） */
const PREVIEW_FILE = 'patchybox.json'

/** 是否为 Tauri 容器（WebView）环境 */
const isTauri = (): boolean => typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window

/** 字符串数组 shape 校验：结构不符按损坏数据处理，不把坏数据当收藏用 */
export function isStringArray(value: unknown): value is string[] {
  return Array.isArray(value) && value.every((item) => typeof item === 'string')
}

export const spaceData = {
  /** 读空间级用户数据；无值时返回 null（调用方给默认值） */
  async get<T>(key: SpaceDataKey, validate?: (value: unknown) => boolean): Promise<T | null> {
    if (!isTauri()) {
      return storage.get<T>(PREVIEW_FILE, key, validate)
    }
    const value = await ipc.preferencesGet(key)
    if (value === null || value === undefined) return null
    if (validate && !validate(value)) {
      console.error('[spaceData] 数据结构不符，按空值处理并保留原文件', { key })
      return null
    }
    return value as T
  },

  /** 写空间级用户数据；失败向上抛出，由调用方回滚并给出可见反馈 */
  async set(key: SpaceDataKey, value: unknown): Promise<void> {
    if (!isTauri()) {
      await storage.set(PREVIEW_FILE, key, value)
      return
    }
    await ipc.preferencesSet(key, value)
  },
}
