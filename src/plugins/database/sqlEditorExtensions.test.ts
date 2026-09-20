import { describe, expect, it, vi } from 'vitest'
import { CompletionContext } from '@codemirror/autocomplete'
import { EditorState } from '@codemirror/state'
import { PostgreSQL, sql } from '@codemirror/lang-sql'
import { sqlCompletionSources } from './sqlEditorExtensions'

describe('异步 SQL 列补全', () => {
  it('通过 CodeMirror 识别别名，冷缓存后仍持续补全', async () => {
    const resolve = vi.fn().mockResolvedValue(['id', 'display_name'])
    const source = sqlCompletionSources(PostgreSQL, { users: [] }, resolve)[1]
    const doc = 'SELECT u. FROM users AS u'
    const context = new CompletionContext(
      EditorState.create({ doc, extensions: [sql({ dialect: PostgreSQL })] }),
      doc.indexOf('.') + 1,
      true
    )
    expect((await source(context))?.options.map((option) => option.label)).toEqual([
      'id',
      'display_name',
    ])
    expect(resolve).toHaveBeenCalledWith('users')
    expect((await source(context))?.options.map((option) => option.label)).toEqual([
      'id',
      'display_name',
    ])
    expect(resolve).toHaveBeenCalledTimes(1)
  })
  it('带空格引号表名不会被截断或误作别名', async () => {
    const resolve = vi.fn().mockResolvedValue(['Item Name'])
    const source = sqlCompletionSources(PostgreSQL, { 'Order Items': [] }, resolve)[1]
    const doc = 'SELECT i. FROM "Order Items" i'
    const context = new CompletionContext(
      EditorState.create({ doc, extensions: [sql({ dialect: PostgreSQL })] }),
      doc.indexOf('.') + 1,
      true
    )
    const result = await source(context)
    expect(resolve).toHaveBeenCalledWith('Order Items')
    expect(result?.options[0].label).toBe('Item Name')
    expect(result?.options[0].apply).toBe('"Item Name"')
  })
})
