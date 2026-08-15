/**
 * sqlFormat 纯函数单测（短单行 / 长换行 / 子查询缩进 / 注释保护）
 */
import { describe, expect, it } from 'vitest'
import { formatSql } from './sqlFormat'

describe('formatSql · 短 SQL 单行', () => {
  it('短查询保持一行（归并多余空白）', () => {
    expect(formatSql('select  *   from   users  where id = 1')).toBe(
      'select * from users where id = 1'
    )
  })

  it('已是单行的语句原样（仅 trim）', () => {
    expect(formatSql('  SELECT * FROM t;  ')).toBe('SELECT * FROM t;')
  })

  it('空输入返回空串', () => {
    expect(formatSql('')).toBe('')
    expect(formatSql('   ')).toBe('')
  })
})

describe('formatSql · 长 SQL 换行', () => {
  it('超长查询按主子句换行', () => {
    const sql =
      'SELECT id, username, display_name, email, status, created_at, updated_at, last_login_at FROM users WHERE status = 1 AND deleted_at IS NULL ORDER BY created_at DESC LIMIT 100'
    const out = formatSql(sql)
    expect(out).toContain('\nFROM ')
    expect(out).toContain('\nWHERE ')
    expect(out).toContain('\nORDER BY ')
    expect(out).toContain('\nLIMIT ')
    expect(out.split('\n')[0]).toMatch(/^SELECT /)
  })

  it('WHERE 多条件：AND/OR 换行缩进', () => {
    const sql =
      'SELECT id, username, display_name, email, status, created_at, updated_at, last_login_at FROM users WHERE status = 1 AND deleted_at IS NULL AND age > 18 ORDER BY created_at DESC LIMIT 100'
    const out = formatSql(sql)
    expect(out).toContain('\n  AND ')
  })
})

describe('formatSql · 嵌套子查询', () => {
  it('FROM 子查询：括号内缩进一级', () => {
    const sql =
      'SELECT t.id, t.name, t.total FROM (SELECT id, name, amount AS total FROM orders WHERE status = 1 AND deleted = 0 AND channel = 2) t WHERE t.total > 100 ORDER BY t.total DESC LIMIT 50'
    const out = formatSql(sql)
    // 子查询 SELECT 缩进一级
    expect(out).toContain('(\n  SELECT ')
    // 外层 WHERE 回到零级
    expect(out).toMatch(/\nWHERE t\.total/)
  })
})

describe('formatSql · 注释保护', () => {
  it('行注释独立成行，不吞后续语句', () => {
    const out = formatSql('-- 查用户\nSELECT * FROM users;')
    expect(out).toContain('-- 查用户\n')
    expect(out).toContain('SELECT')
  })

  it('字符串内的分号/关键字不受影响', () => {
    expect(formatSql("SELECT 'a;b' AS v WHERE 1=1")).toBe("SELECT 'a;b' AS v WHERE 1=1")
  })
})

describe('formatSql · 多语句', () => {
  it('分号分隔的语句各自成行', () => {
    const out = formatSql('SELECT 1; SELECT 2;')
    expect(out.split('\n').filter(Boolean)).toEqual(['SELECT 1;', 'SELECT 2;'])
  })
})
