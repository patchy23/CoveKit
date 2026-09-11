/**
 * 文档统计与降级判定（composable）
 *
 * 每次文档变化统计行数 / 字符数，并按 `status.ts` 的阈值判定降级级别；
 * 级别发生变化时回调宿主（宿主据此重配 Compartment：卸载重能力、退化为纯文本、强制只读）。
 * 行数与字符数走文档统计而非字符串 split，避免大文档上重复生成数组。
 */
import { shallowRef, type Ref } from 'vue'
import type { EditorState } from '@codemirror/state'
import { degradeLevelFor, type EditorDegradeLevel } from './status'
import type { EditorDocStats } from './types'

/** 文档统计与降级跟踪器 */
export interface DocStatsTracker {
  /** 行数 / 字符数 */
  stats: Ref<EditorDocStats>
  /** 当前降级级别 */
  level: Ref<EditorDegradeLevel>
  /** 文档变化后调用（级别变化时触发 onLevelChange） */
  update: (state: EditorState) => void
  /** 用初始内容初始化（挂载时调用，避免首次统计缺失） */
  init: (state: EditorState) => void
}

/**
 * 创建统计跟踪器
 *
 * @param onLevelChange 降级级别变化时回调（只在新级别上触发一次）
 */
export function createDocStatsTracker(
  onLevelChange?: (level: EditorDegradeLevel) => void
): DocStatsTracker {
  const stats = shallowRef<EditorDocStats>({ lines: 1, length: 0 })
  const level = shallowRef<EditorDegradeLevel>('none')

  function update(state: EditorState): void {
    stats.value = { lines: state.doc.lines, length: state.doc.length }
    const next = degradeLevelFor(state.doc.length)
    if (next === level.value) return
    level.value = next
    onLevelChange?.(next)
  }

  return { stats, level, update, init: update }
}
