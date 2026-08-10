/**
 * 收藏（Pinia）：工具 id 集合，持久化走 storage 适配层（Tauri store / localStorage 降级）
 */
import { defineStore } from 'pinia'
import { ref } from 'vue'
import { storage } from '@/core/storage'

const STORE_FILE = 'patchybox.json'
const FAVORITES_KEY = 'favorites'

export const useFavoritesStore = defineStore('favorites', () => {
  const ids = ref<string[]>([])
  const loaded = ref(false)

  async function init() {
    if (loaded.value) return
    ids.value = (await storage.get<string[]>(STORE_FILE, FAVORITES_KEY)) ?? []
    loaded.value = true
  }

  function has(id: string): boolean {
    return ids.value.includes(id)
  }

  async function toggle(id: string): Promise<boolean> {
    const nowFav = !has(id)
    ids.value = nowFav ? [...ids.value, id] : ids.value.filter((x) => x !== id)
    await storage.set(STORE_FILE, FAVORITES_KEY, ids.value)
    return nowFav
  }

  return { ids, loaded, init, has, toggle }
})
