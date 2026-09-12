/**
 * SSH 目录书签（文件页底栏；按服务器 profile 隔离，ssh.db 持久化）
 */
import { ref, watch } from 'vue'
import { useUiStore } from '@/stores/ui'
import { ipc } from '../ipc'
import type { RemoteFile, SshBookmark } from '../contracts'

export function useBookmarks(deps: {
  profileId: () => string | undefined
  /** 点击书签跳转 */
  navigate: (path: string) => void
}) {
  const ui = useUiStore()

  const bookmarks = ref<SshBookmark[]>([])
  /** 下拉面板开关 */
  const open = ref(false)

  async function load() {
    const id = deps.profileId()
    if (!id) {
      bookmarks.value = []
      return
    }
    try {
      bookmarks.value = await ipc.sshBookmarkList(id)
    } catch (e) {
      ui.toast(`读取书签失败：${e}`)
    }
  }

  /** 目录右键「添加书签」：名称默认目录名（同路径后端去重） */
  async function add(dir: RemoteFile) {
    const id = deps.profileId()
    if (!id) return
    try {
      await ipc.sshBookmarkAdd(id, dir.name, dir.path)
      ui.toast(`已添加书签 ${dir.name}`)
      await load()
    } catch (e) {
      ui.toast(`添加书签失败：${e}`)
    }
  }

  async function remove(id: string) {
    try {
      await ipc.sshBookmarkDelete(id)
      await load()
    } catch (e) {
      ui.toast(`删除书签失败：${e}`)
    }
  }

  /** 点击书签：收起面板并跳转 */
  function go(path: string) {
    open.value = false
    deps.navigate(path)
  }

  watch(deps.profileId, () => void load(), { immediate: true })

  return { bookmarks, open, add, remove, go }
}
