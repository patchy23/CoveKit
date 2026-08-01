/**
 * 持久化适配层
 * Tauri 环境（WebView 内）走 tauri-plugin-store（架构 §3 持久化策略）；
 * 非 Tauri 环境（纯 vite dev / vitest / 浏览器预览）降级 localStorage，
 * 保证框架在无容器环境可独立开发与测试。
 */
import { load, type Store } from "@tauri-apps/plugin-store";

const isTauri = (): boolean => typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

let storeCache: Store | null = null;

async function getStore(file: string): Promise<Store> {
  if (!storeCache) storeCache = await load(file);
  return storeCache;
}

export const storage = {
  async get<T>(file: string, key: string): Promise<T | null> {
    if (isTauri()) {
      const store = await getStore(file);
      return ((await store.get<T>(key)) ?? null) as T | null;
    }
    const raw = localStorage.getItem(`${file}:${key}`);
    return raw ? (JSON.parse(raw) as T) : null;
  },

  async set(file: string, key: string, value: unknown): Promise<void> {
    if (isTauri()) {
      const store = await getStore(file);
      await store.set(key, value);
      await store.save();
      return;
    }
    localStorage.setItem(`${file}:${key}`, JSON.stringify(value));
  },
};
