import { expect, it } from 'vitest'
import { parse } from 'vue/compiler-sfc'

const vueSources = import.meta.glob('/src/**/*.vue', {
  eager: true,
  query: '?raw',
  import: 'default',
}) as Record<string, string>
const styleSources = import.meta.glob(['/src/**/*.css', '/src/**/*.ts', '!/src/**/*.test.ts'], {
  eager: true,
  query: '?raw',
  import: 'default',
}) as Record<string, string>

function hasRawScrollClass(source: string): boolean {
  const ast = parse(source).descriptor.template?.ast
  if (!ast) return false
  type Node = typeof ast | (typeof ast.children)[number]
  function visit(node: Node): boolean {
    if (
      node.type === 1 &&
      node.props.some((prop) => {
        const isClass =
          (prop.type === 6 && prop.name === 'class') ||
          (prop.type === 7 &&
            prop.name === 'bind' &&
            prop.arg?.type === 4 &&
            prop.arg.content === 'class')
        return isClass && /\boverflow(?:-[xy])?-(?:auto|scroll)\b/.test(prop.loc.source)
      })
    )
      return true
    return (node.type === 0 || node.type === 1) && node.children.some(visit)
  }
  return visit(ast)
}

const hasScrollbarStyle = (source: string) =>
  /::-webkit-scrollbar|scrollbar-color\s*:|scrollbar-width\s*:/.test(source)

it('滚动入口守卫能识别违规类，说明文字和非滚动 overflow 不误报', () => {
  expect(hasRawScrollClass('<template><div class="overflow-x-auto" /></template>')).toBe(true)
  expect(
    hasRawScrollClass('<template><div :class="{ \'overflow-auto\': open }" /></template>')
  ).toBe(true)
  expect(
    hasRawScrollClass(
      '<template><!-- overflow-auto --><UiScrollArea><div class="overflow-hidden">overflow-auto</div></UiScrollArea></template>'
    )
  ).toBe(false)
  expect(hasScrollbarStyle('.x::-webkit-scrollbar { width: 8px }')).toBe(true)
  expect(hasScrollbarStyle('.x { overflow: auto }')).toBe(false)
})

it('业务滚动容器不再直接拼接原生 overflow 滚动类', () => {
  expect(Object.keys(vueSources).length).toBeGreaterThan(0)
  const violations = Object.entries(vueSources)
    .filter(([, source]) => hasRawScrollClass(source))
    .map(([file]) => file)
  expect(violations).toEqual([])
})

it('滚动条视觉规则只维护在公共样式入口', () => {
  const violations = Object.entries({ ...vueSources, ...styleSources })
    .filter(([file, source]) => file !== '/src/core/ui/scrollbars.css' && hasScrollbarStyle(source))
    .map(([file]) => file)
  expect(violations).toEqual([])
})
