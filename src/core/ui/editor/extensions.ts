/**
 * 编辑器基础扩展集（full / minimal 两档）
 *
 * full：完整编辑体验（当前行高亮、选区匹配、括号匹配与自动闭合、折叠、多光标、列选择）。
 * minimal：轻量档，替代原「行号 + textarea」场景——保留历史、缩进、行号与基础键位，
 *          不装折叠/括号闭合/选区匹配/列选择，降低实例开销。
 *
 * 语言、只读、tabSize、换行这四项由组件的 Compartment 动态重配置，不在本文件内。
 */
import { EditorState, type Extension } from '@codemirror/state'
import {
  drawSelection,
  dropCursor,
  highlightActiveLine,
  highlightActiveLineGutter,
  highlightSpecialChars,
  keymap,
  lineNumbers,
  placeholder as placeholderExtension,
  rectangularSelection,
  crosshairCursor,
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

/** 编辑器档位：full 完整能力，minimal 轻量（工具输入输出区） */
export type EditorMode = 'full' | 'minimal'

/** 组装静态扩展所需的选项 */
export interface EditorExtensionOptions {
  /** 档位 */
  mode: EditorMode
  /** 是否显示行号 */
  lineNumbers: boolean
  /** 是否显示折叠 gutter（仅 full 档生效） */
  foldGutter: boolean
  /** 占位提示（空文档时显示） */
  placeholder?: string
}

/** 基础键位（defaultKeymap + 历史 + Tab 缩进），full 档追加括号与折叠键位 */
export function buildKeymap(mode: EditorMode): KeyBinding[] {
  const keys: KeyBinding[] = [...defaultKeymap, ...historyKeymap, indentWithTab]
  if (mode === 'full') keys.push(...closeBracketsKeymap, ...foldKeymap)
  return keys
}

/** 组装静态扩展列表（不含语言 / 只读 / tabSize / 换行） */
export function buildBaseExtensions(options: EditorExtensionOptions): Extension[] {
  const full = options.mode === 'full'
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

  if (full) {
    extensions.push(
      highlightActiveLine(),
      highlightActiveLineGutter(),
      highlightSelectionMatches(),
      bracketMatching(),
      closeBrackets(),
      rectangularSelection(),
      crosshairCursor()
    )
    if (options.foldGutter) extensions.push(foldGutter())
  }

  if (options.placeholder) extensions.push(placeholderExtension(options.placeholder))

  extensions.push(keymap.of(buildKeymap(options.mode)))
  return extensions
}
