/**
 * 自动转换调度 · 格式转换工具各面板共用的「免按钮」交互（2026-09-18 定稿）
 * 参考 DevToys / IT Tools / CyberChef Auto Bake：输入停顿后自动转换，不再要点按钮。
 * - 输入变化：300ms 防抖执行（连打不闪结果，停顿即有反馈）
 * - 模式/方向切换、挂载初始：立即执行，并取消排队中的防抖任务
 * - 组件卸载（作用域销毁）：自动清定时器，不留悬挂回调
 */
import { onScopeDispose, watch, type WatchSource } from 'vue'

/** 输入防抖时长（ms）：同类工具常见档位，兼顾跟手与结果稳定 */
const DEBOUNCE_MS = 300

export function useAutoConvert(run: () => void) {
  let timer: ReturnType<typeof setTimeout> | null = null

  /** 取消待执行的防抖任务（重复输入 / 立即执行 / 卸载前调用） */
  function cancel() {
    if (timer !== null) {
      clearTimeout(timer)
      timer = null
    }
  }

  /** 监听输入源：每次变化重置 300ms 防抖计时，停顿后才执行转换 */
  function watchInput(source: WatchSource) {
    watch(source, () => {
      cancel()
      timer = setTimeout(() => {
        timer = null
        run()
      }, DEBOUNCE_MS)
    })
  }

  /** 立即执行一次转换（挂载初始渲染、模式/方向切换），并丢弃排队中的防抖任务 */
  function runNow() {
    cancel()
    run()
  }

  onScopeDispose(cancel)

  return { watchInput, runNow }
}
