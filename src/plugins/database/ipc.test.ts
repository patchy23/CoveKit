import { afterEach, expect, it, vi } from 'vitest'
import { invokeCommand } from '@/core/ipc/ipc'
import { draftIpc, queryIpc } from './ipc'
import type { QueryResult } from './contracts'

vi.mock('@/core/ipc/ipc', () => ({ invokeCommand: vi.fn() }))
afterEach(() => vi.clearAllMocks())

it('草稿增量通过原命令携带完整顺序，空顺序用于原子清空', async () => {
  await draftIpc.save([], ['a', 'b'])
  expect(invokeCommand).toHaveBeenLastCalledWith('dbc_drafts_save', {
    drafts: [],
    order: ['a', 'b'],
  })
  await draftIpc.save([], [])
  expect(invokeCommand).toHaveBeenLastCalledWith('dbc_drafts_save', { drafts: [], order: [] })
  await draftIpc.save([])
  expect(invokeCommand).toHaveBeenLastCalledWith('dbc_drafts_save', { drafts: [] })
  const positions = [{ id: 'a', from: 1, to: 3, active: true }]
  await draftIpc.save([], ['a'], positions)
  expect(invokeCommand).toHaveBeenLastCalledWith('dbc_drafts_save', {
    drafts: [],
    order: ['a'],
    positions,
  })
})

it('继续读取携带已接收偏移，关闭精确命中当前页游标', async () => {
  vi.mocked(invokeCommand).mockResolvedValueOnce(result())
  await queryIpc.fetch('connection', 'workspace', 'cursor', 200)
  expect(invokeCommand).toHaveBeenLastCalledWith('dbc_query_fetch', {
    connId: 'connection',
    workspaceId: 'workspace',
    cursorId: 'cursor',
    expectedOffset: 200,
  })
  await queryIpc.closeCursor('connection', 'workspace', 'cursor')
  expect(invokeCommand).toHaveBeenLastCalledWith('dbc_query_close', {
    connId: 'connection',
    workspaceId: 'workspace',
    cursorId: 'cursor',
  })
})

it('Redis 库列表、SCAN 与键详情 IPC 携带明确的逻辑库范围', async () => {
  vi.mocked(invokeCommand).mockResolvedValueOnce({
    databases: ['db0', 'db2'],
    keyCounts: { db0: 12, db2: null },
    warning: null,
  })
  expect(await queryIpc.redisDatabases('redis-conn')).toEqual({
    databases: ['db0', 'db2'],
    keyCounts: { db0: 12, db2: null },
    warning: null,
  })
  expect(invokeCommand).toHaveBeenLastCalledWith('dbc_redis_databases', { connId: 'redis-conn' })

  vi.mocked(invokeCommand).mockResolvedValueOnce([17, ['shared']])
  await queryIpc.redisKeys('redis-conn', 'item:*', 9, 'db2')
  expect(invokeCommand).toHaveBeenLastCalledWith('dbc_redis_keys', {
    connId: 'redis-conn',
    pattern: 'item:*',
    cursor: 9,
    database: 'db2',
  })

  vi.mocked(invokeCommand).mockResolvedValueOnce({
    key: 'shared',
    kind: 'string',
    ttl: -1,
    value: 'value',
  })
  await queryIpc.redisKeyInfo('redis-conn', 'shared', 'db2')
  expect(invokeCommand).toHaveBeenLastCalledWith('dbc_redis_key_info', {
    connId: 'redis-conn',
    key: 'shared',
    database: 'db2',
  })
})

function result(overrides: Partial<QueryResult> = {}): QueryResult {
  return {
    hasMore: false,
    ok: true,
    columns: [],
    rows: [],
    rowsAffected: 0,
    isQuery: true,
    durationMs: 1,
    truncated: false,
    ...overrides,
  }
}

it('多语句根展示引用完整末语句数据，保留脚本汇总状态', async () => {
  const last = result({
    columns: ['value'],
    rows: [['large text']],
    values: [[{ kind: 'text', value: 'large text' }]],
    columnTypes: ['TEXT'],
    pageInfo: { mode: 'cursor', page: 2, pageSize: 100 },
  })
  const payload = result({
    statements: [result({ ok: false }), last],
    displayStatement: 1,
    ok: false,
    truncated: true,
    durationMs: 123,
    transactionActive: true,
  })
  vi.mocked(invokeCommand).mockResolvedValueOnce(payload)
  const received = await queryIpc.execute('c', 'select 1; select 2', 100, 'r')
  expect(received.rows).toBe(last.rows)
  expect(received.values).toBe(last.values)
  expect(received.columns).toBe(last.columns)
  expect(received.columnTypes).toBe(last.columnTypes)
  expect(received.pageInfo).toBe(last.pageInfo)
  expect(received).toMatchObject({
    ok: false,
    truncated: true,
    durationMs: 123,
    transactionActive: true,
  })
  expect(received.statements?.[0]?.ok).toBe(false)
})

it('单语句及原有完整响应直接返回，不重新复制数组', async () => {
  const payload = result({ rows: [['old-compatible']] })
  vi.mocked(invokeCommand).mockResolvedValueOnce(payload)
  expect(await queryIpc.execute('c', 'select 1', 100, 'r')).toBe(payload)
})

it('安全结果计数复用 dbc_execute 请求身份与执行作用域', async () => {
  vi.mocked(invokeCommand).mockResolvedValueOnce(result({ rows: [['42']] }))
  await queryIpc.count('connection', 'SELECT id FROM items', 'count#1', 'query', {
    database: 'app',
    schema: 'public',
  })
  expect(invokeCommand).toHaveBeenLastCalledWith('dbc_execute', {
    connId: 'connection',
    sql: 'SELECT id FROM items',
    maxRows: 1,
    requestId: 'count#1',
    workspaceId: 'query',
    scope: { database: 'app', schema: 'public' },
    countOnly: true,
  })
})

it('缺失的数据引用显式失败，不能伪装成空查询成功', async () => {
  vi.mocked(invokeCommand).mockResolvedValueOnce(result({ displayStatement: 2, statements: [] }))
  await expect(queryIpc.execute('c', 'select 1', 100, 'r')).rejects.toThrow('缺少对应语句')
})

it('类型值紧凑响应在执行、续读和表浏览中保留 NULL、精度与二进制', async () => {
  const values = [
    [
      { kind: 'null', value: null },
      { kind: 'text', value: 'NULL' },
      { kind: 'integer', value: '9007199254740993' },
      { kind: 'binary', value: '00ff' },
      { kind: 'text', value: '' },
    ],
  ] as QueryResult['values']
  for (const read of [
    () => queryIpc.execute('c', 'select * from t', 100, 'r'),
    () => queryIpc.fetch('c', 'w', 'cursor', 100),
    () => queryIpc.tableData('c', 't', 1, 100),
  ]) {
    vi.mocked(invokeCommand).mockResolvedValueOnce(result({ values }))
    const received = await read()
    expect(received.rows).toEqual([['NULL', 'NULL', '9007199254740993', '0x00ff', '']])
    expect(received.values).toBe(values)
  }
})

it('多语句紧凑数据先还原，再共享根展示数组', async () => {
  const last = result({ columns: ['v'], values: [[{ kind: 'text', value: '正文' }]] })
  vi.mocked(invokeCommand).mockResolvedValueOnce(
    result({ statements: [last], displayStatement: 0 })
  )
  const received = await queryIpc.execute('c', 'select 1', 100, 'r')
  expect(received.rows).toBe(last.rows)
  expect(received.rows).toEqual([['正文']])
})
