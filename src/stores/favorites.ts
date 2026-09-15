/**
 * 收藏（Pinia）：工具 id 集合
 *
 * 持久化走空间级用户数据通道（`@/core/spaceData`）：桌面经 Rust 设置服务落**当前空间**偏好文件，
 * 换空间后收藏各自独立；浏览器预览降级 storage 适配层。
 * 写失败必须回滚内存状态并向上抛出（调用方 toast）——不允许出现「界面已变化、磁盘其实没存」。
 */
import { defineStore } from 'pinia'
import { ref } from 'vue'
import { isStringArray, spaceData } from '@/core/spaceData'

/** 收藏键（与 Rust 白名单一致） */
const FAVORITES_KEY = 'favorites'

export const useFavoritesStore = defineStore('favorites', () => {
  const ids = ref<string[]>([])
  const loaded = ref(false)

  async function init() {
    if (loaded.value) return
    ids.value = (await spaceData.get<string[]>(FAVORITES_KEY, isStringArray)) ?? []
    loaded.value = true
  }

  function has(id: string): boolean {
    return ids.value.includes(id)
  }

  /** 切换收藏；写盘失败时回滚到改动前的集合并把错误抛给调用方 */
  async function toggle(id: string): Promise<boolean> {
    const previous = ids.value
    const nowFav = !has(id)
    ids.value = nowFav ? [...previous, id] : previous.filter((x) => x !== id)
    try {
      await spaceData.set(FAVORITES_KEY, ids.value)
    } catch (error) {
      ids.value = previous
      throw error
    }
    return nowFav
  }

  return { ids, loaded, init, has, toggle }
})
