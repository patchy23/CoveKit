import { describe, expect, it } from 'vitest'
import { formatXml, isValidXml, minifyXml } from './useXml'

describe('useXml', () => {
  it('格式化缩进', () => {
    const r = formatXml('<a><b x="1"><c>text</c></b><b/></a>')
    expect(r.ok).toBe(true)
    expect(r.output).toBe(
      ['<a>', '  <b x="1">', '    <c>text</c>', '  </b>', '  <b/>', '</a>'].join('\n')
    )
  })

  it('保留声明与注释', () => {
    const r = formatXml('<?xml version="1.0"?><a><!-- 注释 --><b>1</b></a>')
    expect(r.ok).toBe(true)
    expect(r.output).toContain('<?xml version="1.0"?>')
    expect(r.output).toContain('<!-- 注释 -->')
  })

  it('非法 XML 报错', () => {
    const r = formatXml('<a><b></a>')
    expect(r.ok).toBe(false)
    expect(r.error).toBeTruthy()
  })

  it('压缩与校验', () => {
    expect(minifyXml('<a>\n  <b>1</b>\n</a>')).toBe('<a><b>1</b></a>')
    expect(isValidXml('<a><b/></a>')).toBe(true)
    expect(isValidXml('<a><b></a>')).toBe(false)
  })

  it('CDATA 整体保留不破坏缩进', () => {
    const r = formatXml(
      '<taskResult>\n  <hunks>\n    <hunk>\n      <line type="add"><![CDATA[  <span class="badge">+新增</span>]]></line>\n    </hunk>\n  </hunks>\n</taskResult>'
    )
    expect(r.ok).toBe(true)
    expect(r.output).toBe(
      [
        '<taskResult>',
        '  <hunks>',
        '    <hunk>',
        '      <line type="add"><![CDATA[  <span class="badge">+新增</span>]]></line>',
        '    </hunk>',
        '  </hunks>',
        '</taskResult>',
      ].join('\n')
    )
  })

  it('@url: 伪命名空间仍可格式化', () => {
    const r = formatXml(
      '<taskResult xmlns="@url:http://demo.hermes.agent/diff-task/v1">\n  <taskMeta taskId="t1"/>\n</taskResult>'
    )
    expect(r.ok).toBe(true)
    expect(r.output).toContain('@url:http://demo.hermes.agent/diff-task/v1')
    expect(r.output).toContain('  <taskMeta taskId="t1"/>')
    expect(r.output).toContain('</taskResult>')
  })
})
