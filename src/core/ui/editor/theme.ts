/**
 * 编辑器主题（唯一事实源）
 *
 * 三条约定：
 * 1. 语法高亮色引用 main.css 的 `--cm-*` 变量（浅色/深色同名覆盖）——切换主题不需要重建
 *    编辑器，也不需要重配置 Compartment；
 * 2. 容器、gutter、当前行、选区等外观引用 `--color-*` token；深色差异统一写在 main.css 的
 *    `[data-theme='dark']` 区块（AGENTS.md 约定：暗色覆盖写 main.css 全局 unlayered 区）；
 * 3. 不在此新造色值（DESIGN.md 为 tokens 单一事实源）。
 */
import { HighlightStyle } from '@codemirror/language'
import { EditorView } from '@codemirror/view'
import { tags as t } from '@lezer/highlight'

/** 语法高亮样式：覆盖 JSON / XML / SQL / YAML / JS / TS / Python / Shell 等常见 token */
export const codeHighlightStyle = HighlightStyle.define([
  { tag: [t.keyword, t.controlKeyword, t.moduleKeyword, t.modifier], color: 'var(--cm-keyword)' },
  { tag: [t.propertyName, t.attributeName], color: 'var(--cm-property)' },
  {
    tag: [t.function(t.variableName), t.function(t.propertyName), t.labelName],
    color: 'var(--cm-property)',
  },
  { tag: [t.string, t.special(t.string), t.regexp, t.character], color: 'var(--cm-string)' },
  { tag: [t.number, t.integer, t.float, t.bool, t.null, t.atom], color: 'var(--cm-number)' },
  { tag: [t.tagName, t.typeName, t.className, t.namespace], color: 'var(--cm-tag)' },
  {
    tag: [
      t.punctuation,
      t.separator,
      t.bracket,
      t.brace,
      t.paren,
      t.angleBracket,
      t.squareBracket,
      t.derefOperator,
    ],
    color: 'var(--cm-punct)',
  },
  { tag: [t.operator, t.operatorKeyword], color: 'var(--cm-keyword)' },
  {
    tag: [t.comment, t.lineComment, t.blockComment, t.docComment],
    color: 'var(--cm-comment)',
    fontStyle: 'italic',
  },
  { tag: [t.variableName, t.definition(t.variableName)], color: 'var(--cm-punct)' },
  {
    tag: [t.self, t.constant(t.variableName), t.standard(t.variableName)],
    color: 'var(--cm-number)',
  },
  { tag: [t.heading, t.strong], color: 'var(--cm-tag)', fontWeight: '600' },
  { tag: [t.emphasis, t.quote], color: 'var(--cm-comment)', fontStyle: 'italic' },
  { tag: t.strikethrough, color: 'var(--cm-comment)', textDecoration: 'line-through' },
  { tag: [t.link, t.url], color: 'var(--cm-property)', textDecoration: 'underline' },
  { tag: [t.meta, t.processingInstruction], color: 'var(--cm-comment)' },
  { tag: t.invalid, color: 'var(--color-danger)' },
])

/** 编辑器基础外观（字体、gutter、当前行、选区、折叠占位、搜索匹配、lint 波浪线） */
export const codeEditorTheme = EditorView.theme({
  '&': {
    height: '100%',
    fontSize: '13px',
    color: 'var(--color-primary)',
    backgroundColor: 'transparent',
  },
  '&.cm-editor.cm-focused': { outline: 'none' },
  '.cm-scroller': {
    fontFamily: 'var(--font-mono)',
    lineHeight: '1.5',
    overflow: 'auto',
  },
  '.cm-content': { padding: '10px 0' },
  '.cm-line': { padding: '0 12px' },
  '.cm-gutters': {
    backgroundColor: 'transparent',
    borderRight: '1px solid var(--color-border)',
    color: 'var(--color-text-muted)',
    fontSize: '12px',
  },
  '.cm-lineNumbers .cm-gutterElement': { padding: '0 8px 0 12px' },
  '.cm-foldGutter .cm-gutterElement': { padding: '0 4px' },
  '.cm-activeLineGutter': { backgroundColor: 'transparent', color: 'var(--color-primary)' },
  '.cm-cursor, .cm-dropCursor': { borderLeftColor: 'var(--color-primary)', borderLeftWidth: '2px' },
  '.cm-selectionBackground': { backgroundColor: 'var(--color-tertiary-soft)' },
  '&.cm-focused .cm-selectionBackground': { backgroundColor: 'var(--color-tertiary-soft)' },
  '.cm-activeLine': { backgroundColor: 'transparent' },
  '.cm-matchingBracket, &.cm-focused .cm-matchingBracket': {
    backgroundColor: 'var(--color-tertiary-soft)',
    outline: '1px solid var(--color-tertiary-strong)',
  },
  '.cm-nonmatchingBracket': { color: 'var(--color-danger)' },
  '.cm-selectionMatch': { backgroundColor: 'var(--color-tertiary-soft)' },
  '.cm-searchMatch': {
    backgroundColor: 'var(--color-tertiary-soft)',
    outline: '1px solid var(--color-border-strong)',
  },
  '.cm-searchMatch.cm-searchMatch-selected': {
    backgroundColor: 'var(--color-tertiary-strong)',
    color: '#fff',
  },
  '.cm-foldPlaceholder': {
    backgroundColor: 'var(--color-tertiary-soft)',
    color: 'var(--color-tertiary-strong)',
    border: 'none',
    borderRadius: '3px',
    margin: '0 2px',
    padding: '0 4px',
  },
  '.cm-tooltip': {
    border: '1px solid var(--color-border)',
    backgroundColor: 'var(--color-surface)',
    color: 'var(--color-primary)',
  },
  '.cm-tooltip.cm-tooltip-autocomplete > ul': { fontFamily: 'var(--font-mono)' },
  '.cm-tooltip-autocomplete > ul > li[aria-selected]': {
    backgroundColor: 'var(--color-tertiary-soft)',
    color: 'var(--color-tertiary-strong)',
  },
  '.cm-placeholder': { color: 'var(--color-text-muted)' },
})
