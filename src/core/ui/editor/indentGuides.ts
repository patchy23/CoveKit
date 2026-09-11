/**
 * 缩进参考线（按行绘制，做法对齐 VS Code 的 indent guides 与 Replit 的 indentation-markers）
 *
 * 与旧实现的差别：
 * 1. 线只画到该行「实际缩进深度」为止 —— 旧版用整行宽度的 repeating-gradient，缩进很浅的行
 *    也会铺满整行，视觉上像栅格，这是观感差的主因；
 * 2. 用伪元素 + z-index:-1 绘制：行自身的背景（当前行高亮、选区）不会被线盖住；
 * 3. 光标所在缩进层级的那一条用激活色（VS Code 的 editorIndentGuide.activeBackground1），
 *    其余用普通色；两色都走 main.css 的 --cm-indent-guide / --cm-indent-guide-active 变量。
 */
import {
  Decoration,
  type DecorationSet,
  EditorView,
  ViewPlugin,
  type ViewUpdate,
} from '@codemirror/view'
import type { Range } from '@codemirror/state'

/** 每层缩进折合的字符宽（ch）：与编辑器 tabSize=2 的显示宽度对应 */
export const INDENT_UNIT_CH = 2

/** 把前导空白折算成列数：空格记 1 列，制表符按 tabSize 记列 */
export function indentColumns(text: string, tabSize: number): number {
  const leading = text.match(/^[\t ]*/)?.[0] ?? ''
  let columns = 0
  for (const char of leading) columns += char === '\t' ? tabSize : 1
  return columns
}

/** 行的缩进层级：不足一层的余数归入下一层（与 VS Code 的取整方式一致） */
export function indentLevel(text: string, tabSize: number): number {
  const level = Math.floor(indentColumns(text, tabSize) / tabSize)
  return Math.max(level, 0)
}

/**
 * 生成单行的参考线背景
 *
 * 每层一条 1px 竖线，用多层 linear-gradient 拼成（比单条 repeating-gradient 更易读，
 * 也便于让单独一层换成激活色）；层级为 0 时返回 null（该行不需要画线）。
 *
 * @param level 该行缩进层级
 * @param activeLevel 光标所在行的缩进层级（该层用激活色；为 0 时无激活层）
 * @param unitCh 每层缩进的字符宽
 */
export function indentGuidesBackground(
  level: number,
  activeLevel: number,
  unitCh: number = INDENT_UNIT_CH
): string | null {
  if (level <= 0) return null
  const layers: string[] = []
  for (let index = 0; index < level; index += 1) {
    const isActive = index === activeLevel - 1
    const color = isActive ? 'var(--cm-indent-guide-active)' : 'var(--cm-indent-guide)'
    layers.push(
      `linear-gradient(to right, ${color} 0 1px, transparent 1px) ${index * unitCh}ch 0 / 1px 100% no-repeat`
    )
  }
  return layers.join(', ')
}

/** 按可见区域重建参考线：每行一条 line decoration，样式由 --cm-indent-bg 变量携带 */
function buildIndentGuides(view: EditorView): DecorationSet {
  const tabSize = view.state.tabSize
  const cursorLine = view.state.doc.lineAt(view.state.selection.main.head)
  const activeLevel = indentLevel(cursorLine.text, tabSize)
  const decorations: Range<Decoration>[] = []

  for (const { from, to } of view.visibleRanges) {
    for (let pos = from; pos <= to;) {
      const line = view.state.doc.lineAt(pos)
      const background = indentGuidesBackground(indentLevel(line.text, tabSize), activeLevel)
      if (background) {
        decorations.push(
          Decoration.line({
            class: 'cm-indent-guides',
            attributes: { style: `--cm-indent-bg: ${background}` },
          }).range(line.from)
        )
      }
      pos = line.to + 1
    }
  }
  return Decoration.set(decorations)
}

/** 缩进参考线扩展：文档、视口或光标变化时重算可见区域的参考线 */
export const indentGuides = ViewPlugin.fromClass(
  class {
    /** 当前可见区域的参考线集合 */
    decorations: DecorationSet

    constructor(view: EditorView) {
      this.decorations = buildIndentGuides(view)
    }

    update(update: ViewUpdate) {
      if (update.docChanged || update.viewportChanged || update.selectionSet) {
        this.decorations = buildIndentGuides(update.view)
      }
    }
  },
  { decorations: (plugin) => plugin.decorations }
)
