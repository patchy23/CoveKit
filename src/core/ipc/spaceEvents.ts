/**
 * 空间数据变更事件（`space-data-changed`，后端在合并/覆盖导入提交与快照还原后广播）
 *
 * 为什么集中在这里：合并导入是原地写入（不重启），已打开的页面需要原地刷新；
 * 各 store/组合式函数各自订阅、定点重拉，事件层只做一件事——把 Tauri 事件
 * 包装成可注册的回调并返回解除函数。
 */
import { listen } from '@tauri-apps/api/event'

/** 订阅空间数据变更；`datasets` 含 `*` 表示全量刷新。返回解除函数 */
export function onSpaceDataChanged(handler: (datasets: string[]) => void): () => void {
  let disposed = false
  let unlisten: (() => void) | undefined
  void listen<{ datasets: string[] }>('space-data-changed', (event) => {
    if (!disposed) handler(event.payload.datasets)
  })
    .then((fn) => {
      // 订阅返回前组件已卸载：立即解除，不泄漏
      if (disposed) fn()
      else unlisten = fn
    })
    // 浏览器预览与单测没有 IPC 事件桥：订阅失败 = 永远不刷新（页面本就拉不到数据）
    .catch(() => undefined)
  return () => {
    disposed = true
    unlisten?.()
  }
}
