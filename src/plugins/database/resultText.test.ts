import { describe, expect, it } from 'vitest'
import { rowsToTsv, rowToInsertSql } from './resultText'
describe('类型化结果复制', () => {
  it('INSERT 保留大整数、NULL、空串、引号和二进制，按方言引用表名', () => {
    const target = { connId: 'c', database: 'app', schema: 'public', table: 'user"data' }
    expect(
      rowToInsertSql(
        'postgresql',
        ['id', 'nil', 'empty', 'text', 'blob'],
        [
          { kind: 'integer', value: '9007199254740993' },
          { kind: 'null', value: null },
          { kind: 'text', value: '' },
          { kind: 'text', value: "O'Brien\\path" },
          { kind: 'binary', value: '00ff' },
        ],
        target
      )
    ).toBe(
      `INSERT INTO "public"."user""data" ("id", "nil", "empty", "text", "blob") VALUES (9007199254740993, NULL, E'', E'O''Brien\\\\path', decode('00ff', 'hex'));`
    )
    expect(
      rowToInsertSql('mysql', ['a`b'], [{ kind: 'text', value: '\\N' }], {
        ...target,
        table: 'items',
      })
    ).toBe("INSERT INTO `app`.`items` (`a``b`) VALUES (CONVERT(X'5c4e' USING utf8mb4));")
    expect(rowToInsertSql('sqlite', ['blob'], [{ kind: 'binary', value: '00ff' }])).toBe(
      `INSERT INTO "result" ("blob") VALUES (X'00ff');`
    )
  })
  it('拒绝重名列及损坏的二进制，不把数值字段中的任意文本当作 SQL 执行', () => {
    expect(() =>
      rowToInsertSql(
        'sqlite',
        ['id', 'id'],
        [
          { kind: 'null', value: null },
          { kind: 'null', value: null },
        ]
      )
    ).toThrow('重名列')
    expect(() =>
      rowToInsertSql('mysql', ['b'], [{ kind: 'binary', value: "ff'); DROP TABLE t;" }])
    ).toThrow('十六进制')
    expect(
      rowToInsertSql('sqlite', ['n'], [{ kind: 'integer', value: '1); DROP TABLE t;' }])
    ).toContain("VALUES ('1); DROP TABLE t;');")
  })
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
