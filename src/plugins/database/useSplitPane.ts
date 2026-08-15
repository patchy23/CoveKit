/**
 * 分栏拖拽（纯逻辑，无样式）
 * 每次挂载用默认值，不持久化；拖动按 delta 计算并范围钳制。
 */
import { onBeforeUnmount, ref } from 'vue'

export interface SplitPaneOptions {
  /** 初始大小（px），每次挂载恢复默认 */
  initial: number
  min: number
  max: number
}

/**
 * 分栏拖拽状态
 * @param vertical true=水平分隔条（上下分栏，用 clientY）；false=垂直分隔条（左右分栏，用 clientX）
 * @param getMax 可选动态上限（如容器剩余高度），mousedown 时读取
 */
export function useSplitPane({ initial, min, max }: SplitPaneOptions, vertical = false) {
  const size = ref(initial)
  let dragging = false

  function onPointerDown(event: MouseEvent, getMax?: () => number) {
    event.preventDefault()
    dragging = true
    const startPos = vertical ? event.clientY : event.clientX
    const startSize = size.value
    const onMove = (move: MouseEvent) => {
      if (!dragging) return
      const pos = vertical ? move.clientY : move.clientX
      const upper = getMax?.() ?? max
      size.value = Math.min(upper, Math.max(min, startSize + (pos - startPos)))
    }
    const onUp = () => {
      dragging = false
      window.removeEventListener('mousemove', onMove)
      window.removeEventListener('mouseup', onUp)
    }
    window.addEventListener('mousemove', onMove)
    window.addEventListener('mouseup', onUp)
  }

  onBeforeUnmount(() => {
    dragging = false
  })

  return { size, onPointerDown }
}
