import { expect, it } from 'vitest'

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

it('业务滚动容器不再直接拼接原生 overflow 滚动类', () => {
  const violations = Object.entries(vueSources)
    .filter(([, source]) => /\boverflow(?:-[xy])?-(?:auto|scroll)\b/.test(source))
    .map(([file]) => file)
  expect(violations).toEqual([])
})

it('滚动条视觉规则只维护在公共样式入口', () => {
  const violations = Object.entries({ ...vueSources, ...styleSources })
    .filter(
      ([file, source]) =>
        file !== '/src/core/ui/scrollbars.css' &&
        /::-webkit-scrollbar|scrollbar-color\s*:|scrollbar-width\s*:/.test(source)
    )
    .map(([file]) => file)
  expect(violations).toEqual([])
})
