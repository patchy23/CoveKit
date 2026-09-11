/**
 * 缩进参考线（VS Code 风格细竖线）
 *
 * 用行级 decoration 挂 `cm-indent-guides` 类，背景由 main.css 的 repeating-linear-gradient
 * 绘制（每 2ch 一格，随字体缩放）；本插件只负责"哪些行需要画线"。
 */
import {
  Decoration,
  type DecorationSet,
  EditorView,
  ViewPlugin,
  type ViewUpdate,
} from '@codemirror/view'
import type { Range } from '@codemirror/state'

/** 行首空白字符数 > 0 的行加参考线 */
function buildIndentGuides(view: EditorView): DecorationSet {
  const decorations: Range<Decoration>[] = []
  for (const { from, to } of view.visibleRanges) {
    for (let pos = from; pos <= to;) {
      const line = view.state.doc.lineAt(pos)
      const indent = line.text.match(/^\s*/)?.[0].length ?? 0
      if (indent > 0) {
        decorations.push(Decoration.line({ class: 'cm-indent-guides' }).range(line.from))
      }
      pos = line.to + 1
    }
  }
  return Decoration.set(decorations)
}

/** 缩进参考线扩展：内容或视口变化时重算可见区域的参考线 */
export const indentGuides = ViewPlugin.fromClass(
  class {
    /** 当前可见区域的参考线集合 */
    decorations: DecorationSet

    constructor(view: EditorView) {
      this.decorations = buildIndentGuides(view)
    }

    update(update: ViewUpdate) {
      if (update.docChanged || update.viewportChanged) {
        this.decorations = buildIndentGuides(update.view)
      }
    }
  },
  { decorations: (plugin) => plugin.decorations }
)
