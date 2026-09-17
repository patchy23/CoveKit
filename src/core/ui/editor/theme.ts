/**
 * 编辑器主题（唯一事实源）
 *
 * 四条约定：
 * 1. 语法高亮色引用 main.css 的 `--cm-*` 变量（浅色/深色同名覆盖）——切换主题不需要重建
 *    编辑器，也不需要重配置 Compartment；
 * 2. 编辑器装饰（gutter 底色、当前行、括号与搜索匹配）同样走 `--cm-*`：这些值在浅色下是
 *    tertiary 的极淡透明度变体，暗色对应值由 main.css 的 `[data-theme='dark']` 覆盖，
 *    因此本文件不含硬编码色值，也不需要额外的暗色规则块；
 * 3. 容器描边、选区、折叠占位等沿用 `--color-*` token（DESIGN.md 为 tokens 单一事实源）；
 * 4. 不在此新造色相：标点用中灰（让关键字/字符串/数字前进一档），变量名用蓝，运算符与
 *    函数名复用紫色，均取自既有调色板。
 */
import { HighlightStyle } from '@codemirror/language'
import { EditorView } from '@codemirror/view'
import { tags as t } from '@lezer/highlight'

/** 语法高亮样式：覆盖 JSON / XML / SQL / YAML / JS / TS / Python / Shell 等常见 token */
export const codeHighlightStyle = HighlightStyle.define([
  { tag: [t.keyword, t.controlKeyword, t.moduleKeyword, t.modifier], color: 'var(--cm-keyword)' },
  { tag: [t.propertyName, t.attributeName], color: 'var(--cm-property)' },
  {
    tag: [t.variableName, t.definition(t.variableName), t.local(t.variableName)],
    color: 'var(--cm-variable)',
  },
  {
    tag: [t.function(t.variableName), t.function(t.propertyName), t.labelName],
    color: 'var(--cm-function)',
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
  {
    tag: [t.operator, t.operatorKeyword, t.arithmeticOperator, t.logicOperator],
    color: 'var(--cm-operator)',
  },
  {
    tag: [t.comment, t.lineComment, t.blockComment, t.docComment],
    color: 'var(--cm-comment)',
    fontStyle: 'italic',
  },
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

/** 编辑器基础外观（字体、gutter、当前行、选区、折叠占位、搜索匹配、lint 波浪线、滚动条） */
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
    lineHeight: '1.6',
    overflow: 'auto',
  },
  '.cm-content': { padding: '10px 0' },
  '.cm-line': { padding: '0 12px', position: 'relative' },
  // 缩进参考线：伪元素 + z-index:-1 绘制，线只到本行缩进深度（每行样式由 indentGuides.ts
  // 写入 --cm-indent-bg），因此不会盖住当前行高亮与选区。
  '.cm-indent-guides::before': {
    content: '""',
    position: 'absolute',
    top: '0',
    bottom: '0',
    left: '12px',
    right: '0',
    background: 'var(--cm-indent-bg)',
    pointerEvents: 'none',
    zIndex: '-1',
  },
  '.cm-gutters': {
    backgroundColor: 'var(--cm-gutter-bg)',
    borderRight: '1px solid var(--color-border)',
    color: 'var(--color-text-muted)',
    fontSize: '12px',
  },
  '.cm-lineNumbers .cm-gutterElement': { padding: '0 8px 0 12px' },
  '.cm-foldGutter': { width: '14px' },
  '.cm-foldGutter .cm-gutterElement': {
    padding: '0 1px',
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    color: 'var(--cm-fold-marker)',
    cursor: 'pointer',
  },
  '.cm-foldGutter .cm-gutterElement:hover': { color: 'var(--cm-fold-marker-hover)' },
  '.cm-foldGutter .cm-fold-marker': { display: 'flex', alignItems: 'center' },
  '.cm-activeLineGutter': {
    backgroundColor: 'var(--cm-active-gutter-bg)',
    color: 'var(--cm-active-gutter-fg)',
  },
  '.cm-activeLine': { backgroundColor: 'var(--cm-active-line)', borderRadius: '2px' },
  '.cm-cursor, .cm-dropCursor': { borderLeftColor: 'var(--color-primary)', borderLeftWidth: '2px' },
  '.cm-selectionBackground': { backgroundColor: 'var(--cm-selection)' },
  // drawSelection 内置了一条更深的聚焦态规则
  // （&light.cm-focused > .cm-scroller > .cm-selectionLayer .cm-selectionBackground，
  //  5 个 class），不写全同一条链条就会被它压过、框选回落到内置的 #d7d4f0 淡紫。
  '&.cm-focused > .cm-scroller > .cm-selectionLayer .cm-selectionBackground': {
    backgroundColor: 'var(--cm-selection)',
  },
  '&.cm-focused .cm-selectionBackground': { backgroundColor: 'var(--cm-selection)' },
  // 原生 ::selection 只作为 drawSelection 不可用时的兜底；编辑器内 CM 会用
  // hideNativeSelection（Prec.highest + !important）把它强制设为透明，不要在此叠加颜色，
  // 否则聚焦框选会变成两层叠色、选中态比失焦更深。
  '.cm-content ::selection': { backgroundColor: 'var(--cm-selection)' },
  '.cm-matchingBracket, &.cm-focused .cm-matchingBracket': {
    backgroundColor: 'var(--cm-match-bg)',
    outline: '1px solid var(--cm-match-border)',
    borderRadius: '3px',
  },
  '.cm-nonmatchingBracket': { color: 'var(--color-danger)' },
  '.cm-selectionMatch': { backgroundColor: 'var(--cm-selection-match)', borderRadius: '2px' },
  '.cm-searchMatch': {
    backgroundColor: 'var(--cm-match-bg)',
    outline: '1px solid var(--cm-match-border)',
    borderRadius: '3px',
  },
  '.cm-searchMatch.cm-searchMatch-selected': {
    backgroundColor: 'var(--color-tertiary-strong)',
    color: '#fff',
  },
  '.cm-foldPlaceholder': {
    backgroundColor: 'var(--color-tertiary-soft)',
    color: 'var(--color-tertiary-strong)',
    border: 'none',
    borderRadius: '4px',
    margin: '0 2px',
    padding: '0 4px',
  },
  '.cm-tooltip': {
    border: '1px solid var(--color-border)',
    backgroundColor: 'var(--color-surface)',
    color: 'var(--color-primary)',
    borderRadius: '4px',
    boxShadow: '0 2px 8px rgba(16, 24, 40, 0.12)',
  },
  '.cm-tooltip.cm-tooltip-autocomplete > ul': { fontFamily: 'var(--font-mono)' },
  '.cm-tooltip-autocomplete > ul > li[aria-selected]': {
    backgroundColor: 'var(--color-tertiary-soft)',
    color: 'var(--color-tertiary-strong)',
  },
  '.cm-placeholder': { color: 'var(--color-text-muted)' },
})
