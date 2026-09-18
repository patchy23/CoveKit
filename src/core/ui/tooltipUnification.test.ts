import { describe, expect, it } from 'vitest'
import { parse } from 'vue/compiler-sfc'

const sources = import.meta.glob('/src/**/*.vue', {
  eager: true,
  query: '?raw',
  import: 'default',
}) as Record<string, string>

// 只允许已消费为 UiTooltip 的控件入口，或明确不是悬停提示的标题。
const titleComponents = new Set([
  'UiButton',
  'UiIconButton',
  'UiSelect',
  'Select',
  'UiCheckbox',
  'UiSwitch',
  'UiTabsOverflow',
  'UiTabStatusDot',
  'UiModal',
  'UiPanel',
  'UiAlert',
  'UiEmptyState',
  'ConfirmDialog',
  'InputDialog',
  'LiveLogDialog',
  'ToolHost',
])

function titleViolations(source: string): string[] {
  const ast = parse(source).descriptor.template?.ast
  if (!ast) return []
  const violations: string[] = []
  type Node = typeof ast | (typeof ast.children)[number]
  function visit(node: Node) {
    if (node.type === 1) {
      for (const prop of node.props) {
        const isTitle =
          (prop.type === 6 && prop.name === 'title') ||
          (prop.type === 7 &&
            prop.name === 'bind' &&
            prop.arg?.type === 4 &&
            prop.arg.content === 'title')
        if (isTitle && !titleComponents.has(node.tag)) {
          violations.push(`${node.tag}: ${prop.loc.source}`)
        }
      }
    }
    // SFC 原始模板未做控制流转换，只遍历根和元素的模板子节点。
    if (node.type === 0 || node.type === 1) node.children.forEach(visit)
  }
  visit(ast)
  return violations
}

describe('悬停提示统一入口', () => {
  it('原生元素和未接入组件不能携带 title', () => {
    expect(titleViolations('<template><span title="路径" /></template>')).toHaveLength(1)
    expect(titleViolations('<template><UiListRow :title="path" /></template>')).toHaveLength(1)
    expect(titleViolations('<template><UiModal title="编辑" /></template>')).toEqual([])
    expect(titleViolations('<template><UiButton title="保存" /></template>')).toEqual([])
  })

  it('全仓 Vue 模板的 title 均有明确归属', () => {
    const violations = Object.entries(sources).flatMap(([file, source]) =>
      titleViolations(source).map((entry) => `${file}: ${entry}`)
    )
    expect(violations).toEqual([])
  })
})
