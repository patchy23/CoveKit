import { describe, expect, it } from 'vitest'
import { rowsToTsv } from './resultText'
describe('类型化结果复制', () => {
  it('区分 NULL、文本 NULL、空串、反斜线及含换行的单元格', () => {
    expect(
      rowsToTsv([
        [
          { kind: 'null', value: null },
          { kind: 'text', value: 'NULL' },
          { kind: 'text', value: '' },
          { kind: 'text', value: String.raw`\N` },
          { kind: 'text', value: 'a\tb\n"c"' },
        ],
      ])
    ).toBe(String.raw`\N` + '\tNULL\t\t' + String.raw`\\N` + '\t"a\tb\n""c"""')
  })
})
