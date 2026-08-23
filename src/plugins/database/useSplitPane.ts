/**
 * 分栏拖拽（纯逻辑，无样式）
 * 每次挂载用默认值，不持久化；拖动按 delta 计算并范围钳制。
 *
 * 方向约定：分隔条在面板"右侧"时（左栏），往右拖 → 面板变宽（正向 delta）；
 * 分隔条在面板"左侧"时（右栏），往右拖 → 面板变窄（reverse=true 反向 delta）。
 * 上下分栏同理：reverse=false 时往下拖面板变高；reverse=true 时往下拖面板变矮。
 */
import { onBeforeUnmount, ref } from 'vue'

export interface SplitPaneOptions {
  /** 初始大小（px），每次挂载恢复默认 */
  initial: number
  min: number
  max: number
  /** 分隔条在面板左侧（右栏/下方面板）：拖动方向与大小变化相反 */
  reverse?: boolean
}

/**
 * 分栏拖拽状态
 * @param vertical true=水平分隔条（上下分栏，用 clientY）；false=垂直分隔条（左右分栏，用 clientX）
 * @param getMax 可选动态上限（如容器剩余高度），mousedown 时读取
 */
export function useSplitPane(
  { initial, min, max, reverse = false }: SplitPaneOptions,
  vertical = false
) {
  const size = ref(initial)
  let dragging = false

  function onPointerDown(event: MouseEvent, getMax?: () => number) {
    event.preventDefault()
    dragging = true
    const startPos = vertical ? event.clientY : event.clientX
    const startSize = size.value
    const sign = reverse ? -1 : 1
    const onMove = (move: MouseEvent) => {
      if (!dragging) return
      const pos = vertical ? move.clientY : move.clientX
      const upper = getMax?.() ?? max
      size.value = Math.min(upper, Math.max(min, startSize + sign * (pos - startPos)))
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
