/**
 * 文件列表多选模型（SSH 双栏共用：远程 FileBrowser / 本地 LocalBrowser）
 * 单选 / Ctrl 切换 / Shift 锚点范围选 / 空白清空；选择集是 path 集合（path 全局唯一）。
 */
import { computed, ref, type Ref } from 'vue'

/** Shift 范围选：返回 [anchor, target] 区间内的 path 列表（纯函数，可单测） */
export function shiftRange(paths: string[], anchor: string, target: string): string[] {
  const ai = paths.indexOf(anchor)
  const ti = paths.indexOf(target)
  if (ai < 0 || ti < 0) return [target]
  const [from, to] = ai <= ti ? [ai, ti] : [ti, ai]
  return paths.slice(from, to + 1)
}

export function useFileSelection(orderedPaths: () => string[]) {
  /** 选中 path 集合 */
  const selectedPaths: Ref<Set<string>> = ref(new Set())
  /** Shift 锚点（最近一次普通单击） */
  let anchor: string | null = null

  /** 行点击入口：根据修饰键分发三种语义 */
  function onRowClick(event: MouseEvent, path: string) {
    if (event.shiftKey && anchor) {
      // Shift 范围选：锚点到当前项（替换选择集）
      selectedPaths.value = new Set(shiftRange(orderedPaths(), anchor, path))
      return
    }
    if (event.ctrlKey || event.metaKey) {
      // Ctrl 切换：不动锚点
      const next = new Set(selectedPaths.value)
      if (next.has(path)) next.delete(path)
      else next.add(path)
      selectedPaths.value = next
      return
    }
    // 普通单击：单选 + 更新锚点
    selectedPaths.value = new Set([path])
    anchor = path
  }

  /** 清空选择（点空白/切目录/切连接） */
  function clear() {
    selectedPaths.value = new Set()
    anchor = null
  }

  /** 单选设置（右键未选中项时对齐选择） */
  function selectOnly(path: string) {
    selectedPaths.value = new Set([path])
    anchor = path
  }

  const isSelected = (path: string) => selectedPaths.value.has(path)
  const count = computed(() => selectedPaths.value.size)

  return { selectedPaths, count, isSelected, onRowClick, clear, selectOnly }
}
