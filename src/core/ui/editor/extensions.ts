/**
 * 编辑器扩展集（full / minimal 两档）+ 大文件降级判定
 *
 * 分三层组装，便于按下沉级别整块装卸：
 * - 基础层（本文件 `buildBaseExtensions`）：高亮、历史、选区绘制、缩进、主题、缩进参考线、行号；
 * - 重能力层（`buildHeavyExtensions`）：折叠 gutter、括号匹配/自动闭合、选区匹配、当前行高亮、列选择——
 *   由 `useCodeEditor` 放进独立 Compartment，文件超过 512KB 时整块卸载；
 * - 动态层（Compartment 装配）：语言、只读、tabSize、换行、补全、校验。
 *
 * minimal 档不装载重能力层，用于替代原「行号 + textarea」输入输出区。
 */
import { EditorState, type Extension } from '@codemirror/state'
import {
  crosshairCursor,
  drawSelection,
  dropCursor,
  highlightActiveLine,
  highlightActiveLineGutter,
  highlightSpecialChars,
  keymap,
  lineNumbers,
  placeholder as placeholderExtension,
  rectangularSelection,
  type KeyBinding,
} from '@codemirror/view'
import { defaultKeymap, history, historyKeymap, indentWithTab } from '@codemirror/commands'
import {
  bracketMatching,
  foldGutter,
  foldKeymap,
  indentOnInput,
  syntaxHighlighting,
} from '@codemirror/language'
import { closeBrackets, closeBracketsKeymap } from '@codemirror/autocomplete'
import { highlightSelectionMatches } from '@codemirror/search'
import { codeEditorTheme, codeHighlightStyle } from './theme'
import { indentGuides } from './indentGuides'
import type { EditorDegradeLevel } from './status'

/** 编辑器档位：full 完整能力，minimal 轻量（工具输入输出区） */
export type EditorMode = 'full' | 'minimal'

/** 组装静态扩展所需的选项 */
export interface EditorExtensionOptions {
  /** 档位 */
  mode: EditorMode
  /** 是否显示行号 */
  lineNumbers: boolean
  /** 占位提示（空文档时显示） */
  placeholder?: string
}

/** 基础键位（defaultKeymap + 历史 + Tab 缩进），full 档追加括号与折叠键位 */
export function buildKeymap(mode: EditorMode): KeyBinding[] {
  const keys: KeyBinding[] = [...defaultKeymap, ...historyKeymap, indentWithTab]
  if (mode === 'full') keys.push(...closeBracketsKeymap, ...foldKeymap)
  return keys
}

/**
 * 重能力扩展（大文件时整块卸载）
 *
 * minimal 档返回空数组：轻量档本就只要「行号 + 基础编辑」。
 */
export function buildHeavyExtensions(mode: EditorMode, withFoldGutter: boolean): Extension[] {
  if (mode !== 'full') return []
  const extensions: Extension[] = [
    highlightActiveLine(),
    highlightActiveLineGutter(),
    highlightSelectionMatches(),
    bracketMatching(),
    closeBrackets(),
    rectangularSelection(),
    crosshairCursor(),
  ]
  if (withFoldGutter) extensions.push(foldGutter())
  return extensions
}

/** 按降级级别给出重能力扩展：仅 none 级装载，large / huge 一律卸载 */
export function heavyExtensionsFor(
  degrade: EditorDegradeLevel,
  mode: EditorMode,
  withFoldGutter: boolean
): Extension[] {
  return degrade === 'none' ? buildHeavyExtensions(mode, withFoldGutter) : []
}

/** 是否加载语言（语法高亮）：large / huge 级退化为纯文本 */
export function highlightEnabled(degrade: EditorDegradeLevel): boolean {
  return degrade === 'none'
}

/** 是否启用补全：large / huge 级关闭（词法扫描在大文档上收益低、开销高） */
export function completionEnabled(degrade: EditorDegradeLevel): boolean {
  return degrade === 'none'
}

/** 是否启用校验：large / huge 级关闭（解析整篇文档在超大文本上代价高） */
export function linterEnabled(degrade: EditorDegradeLevel): boolean {
  return degrade === 'none'
}

/** 组装静态扩展列表（重能力层与动态层由 useCodeEditor 用 Compartment 装配） */
export function buildBaseExtensions(options: EditorExtensionOptions): Extension[] {
  const extensions: Extension[] = [
    highlightSpecialChars(),
    history(),
    drawSelection(),
    dropCursor(),
    EditorState.allowMultipleSelections.of(true),
    indentOnInput(),
    codeEditorTheme,
    syntaxHighlighting(codeHighlightStyle),
    indentGuides,
  ]

  if (options.lineNumbers) extensions.push(lineNumbers())
  if (options.placeholder) extensions.push(placeholderExtension(options.placeholder))

  extensions.push(keymap.of(buildKeymap(options.mode)))
  return extensions
}
