/**
 * createTableSql 纯函数单测（可视化建表 DDL 生成）
 */
import { describe, expect, it } from 'vitest'
import { buildCreateTableSql, emptyColumn, type CreateTableOptions } from './createTableSql'

function baseOptions(overrides: Partial<CreateTableOptions> = {}): CreateTableOptions {
  return {
    database: 'shop',
    table: 'users',
    engine: 'InnoDB',
    charset: 'utf8mb4',
    comment: '',
    columns: [],
    ...overrides,
  }
}

describe('buildCreateTableSql', () => {
  it('表名或有效列为空时返回空串', () => {
    expect(buildCreateTableSql(baseOptions())).toBe('')
    expect(buildCreateTableSql(baseOptions({ columns: [emptyColumn()] }))).toBe('')
    expect(
      buildCreateTableSql(baseOptions({ table: '  ', columns: [{ ...emptyColumn(), name: 'id' }] }))
    ).toBe('')
  })

  it('生成带库限定名/引擎/字符集/主键的完整 DDL', () => {
    const sql = buildCreateTableSql(
      baseOptions({
        comment: '用户表',
        columns: [
          {
            ...emptyColumn(),
            name: 'id',
            type: 'BIGINT',
            length: '',
            nullable: false,
            autoIncrement: true,
            primary: true,
            comment: '主键',
          },
          { ...emptyColumn(), name: 'name', type: 'VARCHAR', length: '64', nullable: false },
          { ...emptyColumn(), name: 'price', type: 'DECIMAL', length: '10,2' },
          {
            ...emptyColumn(),
            name: 'created_at',
            type: 'DATETIME',
            length: '',
            defaultValue: 'CURRENT_TIMESTAMP',
          },
          {
            ...emptyColumn(),
            name: 'status',
            type: 'TINYINT',
            length: '1',
            defaultValue: '1',
          },
        ],
      })
    )
    expect(sql).toBe(
      [
        'CREATE TABLE `shop`.`users` (',
        "  `id` BIGINT NOT NULL AUTO_INCREMENT COMMENT '主键',",
        '  `name` VARCHAR(64) NOT NULL,',
        '  `price` DECIMAL(10,2),',
        '  `created_at` DATETIME DEFAULT CURRENT_TIMESTAMP,',
        '  `status` TINYINT(1) DEFAULT 1,',
        '  PRIMARY KEY (`id`)',
        ") ENGINE=InnoDB DEFAULT CHARACTER SET utf8mb4 COMMENT='用户表'",
      ].join('\n')
    )
  })

  it('库名为空时不带限定名；字符串默认值加引号并转义', () => {
    const sql = buildCreateTableSql(
      baseOptions({
        database: '',
        engine: '',
        charset: '',
        columns: [{ ...emptyColumn(), name: 'nick', defaultValue: "o'k" }],
      })
    )
    expect(sql).toBe("CREATE TABLE `users` (\n  `nick` VARCHAR(64) DEFAULT 'o''k'\n)")
  })

  it('标识符反引号转义；非法长度被丢弃', () => {
    const sql = buildCreateTableSql(
      baseOptions({
        database: '',
        table: 'my`table',
        engine: '',
        charset: '',
        columns: [{ ...emptyColumn(), name: 'a', type: 'VARCHAR', length: '64; DROP TABLE t' }],
      })
    )
    expect(sql).toBe('CREATE TABLE `my``table` (\n  `a` VARCHAR\n)')
  })
})
