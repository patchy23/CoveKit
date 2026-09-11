/**
 * 缩进参考线纯函数单测：列数折算、层级取整、单行背景拼接与激活层着色
 */
import { describe, expect, it } from 'vitest'
import { indentColumns, indentGuidesBackground, indentLevel, INDENT_UNIT_CH } from './indentGuides'

describe('indentColumns', () => {
  it('空格按 1 列计', () => {
    expect(indentColumns('    hello', 2)).toBe(4)
  })

  it('制表符按 tabSize 折算', () => {
    expect(indentColumns('\t\tname', 2)).toBe(4)
    expect(indentColumns('\t name', 3)).toBe(4)
  })

  it('无缩进返回 0', () => {
    expect(indentColumns('SELECT 1', 2)).toBe(0)
  })

  it('只统计前导空白，行内空白不计', () => {
    expect(indentColumns('  a  b  ', 2)).toBe(2)
  })
})

describe('indentLevel', () => {
  it('整除时即为层数', () => {
    expect(indentLevel('    x', 2)).toBe(2)
  })

  it('不足一层归入下一层（向下取整）', () => {
    expect(indentLevel('   x', 2)).toBe(1)
    expect(indentLevel(' x', 2)).toBe(0)
  })

  it('从不返回负数', () => {
    expect(indentLevel('x', 2)).toBe(0)
  })
})

describe('indentGuidesBackground', () => {
  it('层级 0 不画线', () => {
    expect(indentGuidesBackground(0, 0)).toBeNull()
  })

  it('按层级给出等量渐变层，且每层宽 1px、间距 unitCh', () => {
    const background = indentGuidesBackground(3, 0)
    expect(background).not.toBeNull()
    const layers = background!.split('linear-gradient').length - 1
    expect(layers).toBe(3)
    expect(background).toContain('var(--cm-indent-guide)')
    expect(background).toContain(`${INDENT_UNIT_CH}ch 0 / 1px 100% no-repeat`)
  })

  it('光标所在层级改用激活色，其余仍为普通色', () => {
    const background = indentGuidesBackground(3, 2)
    expect(background).toContain('var(--cm-indent-guide-active)')
    expect(background!.match(/var\(--cm-indent-guide\)/g)?.length).toBe(2)
  })

  it('激活层深于本行层级时不越界着色', () => {
    const background = indentGuidesBackground(1, 3)
    expect(background).toContain('var(--cm-indent-guide)')
    expect(background).not.toContain('active')
  })
})
