/**
 * 持久化适配层
 * Tauri 环境（WebView 内）走 tauri-plugin-store（架构 §3 持久化策略）；
 * 非 Tauri 环境（纯 vite dev / vitest / 浏览器预览）降级 localStorage，
 * 保证框架在无容器环境可独立开发与测试。
 *
 * 缓存与容错（T07-5）：
 * - 按文件名缓存 Promise 而不是 Store：并发读取同一文件只加载一次，失败后清缓存可重试；
 * - 读取可带 shape 校验：结构不符视为损坏；
 * - 损坏数据先隔离备份（localStorage 下另存副本并删除原键），再返回空值让调用方用默认值，
 *   既不静默使用坏数据，也不会直接覆盖掉还能救回的内容。
 */
import { load, type Store } from '@tauri-apps/plugin-store'

const isTauri = (): boolean => typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window

/** 文件 → 加载中的 Promise（失败时清除，允许重试） */
const storePromises = new Map<string, Promise<Store>>()

function getStore(file: string): Promise<Store> {
  const cached = storePromises.get(file)
  if (cached) return cached
  const pending = load(file).catch((error) => {
    storePromises.delete(file)
    throw error
  })
  storePromises.set(file, pending)
  return pending
}

/** 损坏数据的隔离备份后缀（保留内容，便于人工恢复） */
const QUARANTINE_SUFFIX = ':corrupt'

/** 把损坏内容另存副本并删除原键（只在 localStorage 路径可用） */
function quarantine(file: string, key: string, raw: string): void {
  try {
    localStorage.setItem(`${file}:${key}${QUARANTINE_SUFFIX}`, raw)
    localStorage.removeItem(`${file}:${key}`)
  } catch (error) {
    console.error('[storage] 损坏数据隔离失败', error)
  }
}

export const storage = {
  /**
   * 读取持久化值
   * @param validate 可选的 shape 校验：返回 false 视为损坏数据（隔离后按空值处理）
   */
  async get<T>(
    file: string,
    key: string,
    validate?: (value: unknown) => boolean
  ): Promise<T | null> {
    if (isTauri()) {
      const store = await getStore(file)
      const value = (await store.get<T>(key)) ?? null
      if (value !== null && validate && !validate(value)) {
        console.error('[storage] 数据结构不符，按损坏处理并保留原文件', { file, key })
        return null
      }
      return value
    }
    const raw = localStorage.getItem(`${file}:${key}`)
    if (raw === null) return null
    let parsed: unknown
    try {
      parsed = JSON.parse(raw)
    } catch (error) {
      console.error('[storage] 数据解析失败，已隔离备份后按空值处理', { file, key, error })
      quarantine(file, key, raw)
      return null
    }
    if (validate && !validate(parsed)) {
      console.error('[storage] 数据结构不符，已隔离备份后按空值处理', { file, key })
      quarantine(file, key, raw)
      return null
    }
    return parsed as T
  },

  async set(file: string, key: string, value: unknown): Promise<void> {
    if (isTauri()) {
      const store = await getStore(file)
      await store.set(key, value)
      await store.save()
      return
    }
    localStorage.setItem(`${file}:${key}`, JSON.stringify(value))
  },

  /** 测试/诊断用：清空加载缓存（下次读取重新加载文件） */
  resetCache(): void {
    storePromises.clear()
  },
}
