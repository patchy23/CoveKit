/**
 * sqlStatementRanges 纯函数单测（分号分割 / 光标语句定位）
 */
import { describe, expect, it } from 'vitest'
import {
  splitSqlStatements,
  statementExecutableSql,
  statementRangeAtCursor,
  statementStartOffset,
} from './sqlStatementRanges'

describe('splitSqlStatements', () => {
  it('按分号分割多语句', () => {
    const ranges = splitSqlStatements('SELECT 1;\nSELECT 2;')
    expect(ranges.map((r) => r.sql)).toEqual(['SELECT 1;', '\nSELECT 2;'])
    expect(ranges[0].from).toBe(0)
    expect(ranges[0].to).toBe(9)
    expect(ranges[1].from).toBe(9)
  })

  it('忽略字符串字面量内的分号', () => {
    const ranges = splitSqlStatements("SELECT 'a;b' AS v;")
    expect(ranges).toHaveLength(1)
    expect(ranges[0].sql).toBe("SELECT 'a;b' AS v;")
  })

  it('忽略转义引号', () => {
    const ranges = splitSqlStatements("SELECT 'it''s; ok';")
    expect(ranges).toHaveLength(1)
  })

  it('忽略单行注释（-- 与 #）内的分号', () => {
    const ranges = splitSqlStatements('SELECT 1; -- a;b\nSELECT 2;')
    expect(ranges).toHaveLength(2)
    // 语句间的注释归属下一条语句范围（分号仍正确切分）
    expect(ranges[1].sql).toBe(' -- a;b\nSELECT 2;')
  })

  it('忽略块注释内的分号', () => {
    const ranges = splitSqlStatements('SELECT 1 /* a;b */, 2;')
    expect(ranges).toHaveLength(1)
  })

  it('末尾无分号的语句也返回', () => {
    const ranges = splitSqlStatements('SELECT 1; SELECT 2')
    expect(ranges).toHaveLength(2)
    expect(ranges[1].sql).toBe(' SELECT 2')
  })

  it('空文本与纯空白返回空数组', () => {
    expect(splitSqlStatements('')).toEqual([])
    expect(splitSqlStatements('   \n')).toEqual([])
  })
})

describe('statementRangeAtCursor', () => {
  const sql = 'SELECT 1;\nSELECT 22;\nSELECT 333;'

  it('光标在语句中间时归属该语句', () => {
    const range = statementRangeAtCursor(sql, 12)
    expect(range?.sql).toBe('\nSELECT 22;')
  })

  it('光标在语句开头（含前导空白）归属该语句', () => {
    const range = statementRangeAtCursor(sql, 10)
    expect(range?.sql).toBe('\nSELECT 22;')
  })

  it('光标在语句结尾分号处归属该语句', () => {
    const range = statementRangeAtCursor(sql, 19)
    expect(range?.sql).toBe('\nSELECT 22;')
  })

  it('光标超出最后语句结尾时归属最后语句', () => {
    const range = statementRangeAtCursor(sql, 999)
    expect(range?.sql).toBe('\nSELECT 333;')
  })

  it('空文本返回 null', () => {
    expect(statementRangeAtCursor('', 0)).toBeNull()
  })
})

describe('statementExecutableSql', () => {
  it('去除首尾空白与结尾分号', () => {
    expect(statementExecutableSql({ from: 0, to: 10, sql: '  SELECT 1;  ' })).toBe('SELECT 1')
  })
})

describe('statementStartOffset', () => {
  it('分号后换行的语句：起始偏移跳过换行，指向自身首个字符', () => {
    // 回归：gutter 按钮曾用 range.from 定位，第二条语句的按钮错位到第一行
    const text = 'SELECT 1;\nSELECT 2;'
    const ranges = splitSqlStatements(text)
    expect(ranges).toHaveLength(2)
    expect(statementStartOffset(ranges[0])).toBe(0)
    // ranges[1].from=9 指向上一条的换行符；首个非空白字符在 10
    expect(ranges[1].from).toBe(9)
    expect(statementStartOffset(ranges[1])).toBe(10)
  })

  it('带缩进的语句：起始偏移跳过缩进空白', () => {
    const ranges = splitSqlStatements('SELECT 1;\n  SELECT 2;')
    expect(statementStartOffset(ranges[1])).toBe(12)
  })
})

describe('方言语句体', () => {
  it('保留 dollar quote 与嵌套注释内的分号', () => {
    const sql = "SELECT $body$a;'b$body$; /* 外 /* 内; */ 外; */ SELECT 2;"
    expect(splitSqlStatements(sql, 'postgresql').map((range) => range.sql)).toEqual([
      "SELECT $body$a;'b$body$;",
      ' /* 外 /* 内; */ 外; */ SELECT 2;',
    ])
  })
  it('脚本中间的 SQLite trigger 是一个执行单元', () => {
    const sql =
      'CREATE TABLE t(id); CREATE TRIGGER tr AFTER INSERT ON t BEGIN UPDATE t SET id=2; SELECT CASE WHEN 1 THEN 2 END; END; SELECT 1;'
    expect(splitSqlStatements(sql, 'sqlite')).toHaveLength(3)
  })
  it('Oracle 程序块不被行侧执行按钮切开', () => {
    expect(splitSqlStatements('BEGIN null; null; END;\n/', 'oracle')).toHaveLength(1)
  })
})
