/**
 * 编辑器状态栏与降级阈值（纯函数，供状态栏 UI 与单测使用）
 *
 * 状态栏信息：行:列、选中字符数、语言（中文）、缩进、规模、编码。
 * 降级阈值：512KB 关高亮/折叠/补全，5MB 强制只读（与任务书 §3.11 一致）。
 */
import { formatBytes } from '@/core/format/bytes'
import type { EditorCursorInfo } from './types'

/** 大文件阈值：超过即关闭高亮 / 折叠 / 补全 / 选区匹配（512KB） */
export const LARGE_FILE_LIMIT = 524288
/** 超大文件阈值：超过即强制只读（5MB） */
export const HUGE_FILE_LIMIT = 5242880

/** 降级级别：none 正常；large 关高亮等重能力；huge 强制只读 */
export type EditorDegradeLevel = 'none' | 'large' | 'huge'

/** 按文档长度判定降级级别 */
export function degradeLevelFor(docLength: number): EditorDegradeLevel {
  if (docLength > HUGE_FILE_LIMIT) return 'huge'
  if (docLength > LARGE_FILE_LIMIT) return 'large'
  return 'none'
}

/** 状态栏文案输入 */
export interface EditorStatusInput {
  /** 光标位置与选中字符数 */
  cursor: EditorCursorInfo
  /** 文档总行数 */
  lines: number
  /** 文档总字符数 */
  length: number
  /** 语言中文标签 */
  languageLabel: string
  /** 缩进宽度 */
  tabSize: number
  /** 是否只读（只读时隐藏选中项） */
  readonly: boolean
  /** 降级级别（large / huge 时状态栏提示） */
  degrade: EditorDegradeLevel
}

/** 状态栏分项文案 */
export interface EditorStatusText {
  /** 行列：`行 12 : 列 5` */
  position: string
  /** 选中：`选中 24`（只读或未选中时为 null） */
  selection: string | null
  /** 语言：`JSON` */
  language: string
  /** 缩进：`空格: 2` */
  indent: string
  /** 规模：`1,234 行 · 56.7 KB` */
  size: string
  /** 编码（固定 UTF-8） */
  encoding: string
  /** 降级提示（正常时为 null） */
  degrade: string | null
}

/** 字节数格式化转发 core 层实现（`@/core/format/bytes`），避免两份换算逻辑漂移 */
export { formatBytes }

/** 千分位数字（避免依赖 toLocaleString 的平台差异） */
export function formatNumber(value: number): string {
  const rounded = Math.round(value * 10) / 10
  const [int, decimal] = String(rounded).split('.')
  const grouped = int.replace(/\B(?=(\d{3})+(?!\d))/g, ',')
  return decimal ? `${grouped}.${decimal}` : grouped
}

/** 拼装状态栏分项文案 */
export function buildStatusText(input: EditorStatusInput): EditorStatusText {
  const selected = input.cursor.selected
  return {
    position: `行 ${input.cursor.line} : 列 ${input.cursor.column}`,
    selection: !input.readonly && selected > 0 ? `选中 ${formatNumber(selected)}` : null,
    language: input.languageLabel,
    indent: `空格: ${input.tabSize}`,
    size: `${formatNumber(input.lines)} 行 · ${formatBytes(input.length)}`,
    encoding: 'UTF-8',
    degrade:
      input.degrade === 'huge'
        ? '文件超过 5MB，已强制只读'
        : input.degrade === 'large'
          ? '文件超过 512KB，已关闭语法高亮与折叠'
          : null,
  }
}

/** 状态栏单行文本（用于 title 提示与单测断言） */
export function formatStatusLine(status: EditorStatusText): string {
  const parts = [
    status.position,
    status.selection,
    status.language,
    status.indent,
    status.size,
    status.encoding,
  ]
  if (status.degrade) parts.push(status.degrade)
  return parts.filter((part): part is string => Boolean(part)).join(' · ')
}
