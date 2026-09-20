/** 小规模可审阅夹具；只生成 SQL，执行仍经过正常目标与风险预检。 */
export function fixtureSql(dialect: string, table: string, count: number): string {
  if (!['mysql', 'polardb', 'postgresql', 'sqlite'].includes(dialect))
    throw new Error('此驱动暂不提供测试数据模板，请使用自有 SQL')
  if (!/^[A-Za-z_][A-Za-z0-9_]{0,62}$/.test(table))
    throw new Error('表名须以英文字母或下划线开头，只含字母、数字、下划线，最多 63 字符')
  if (!Number.isInteger(count) || count < 1 || count > 1000)
    throw new Error('测试数据行数须为 1 至 1000')
  const name =
    dialect === 'mysql' || dialect === 'polardb' ? '\x60' + table + '\x60' : '"' + table + '"'
  const cte =
    'WITH RECURSIVE seq(n) AS (SELECT 1 UNION ALL SELECT n + 1 FROM seq WHERE n < ' + count + ')'
  const fields = '(id, label, amount, note)'
  const values =
    'SELECT n, ' +
    (dialect === 'mysql' || dialect === 'polardb' ? "CONCAT('sample-', n)" : "'sample-' || n") +
    ", n / 10.0, CASE WHEN n % 3 = 0 THEN NULL ELSE 'test' END FROM seq;"
  const insert = 'INSERT INTO ' + name + ' ' + fields
  return (
    '-- 测试夹具：新建 ' +
    table +
    '，生成 ' +
    count +
    ' 行；不删除已有表。\nCREATE TABLE ' +
    name +
    ' (id INTEGER PRIMARY KEY, label VARCHAR(64), amount DECIMAL(12,2), note TEXT);\n' +
    (dialect === 'mysql' || dialect === 'polardb' ? insert + '\n' + cte : cte + '\n' + insert) +
    '\n' +
    values +
    '\nSELECT * FROM ' +
    name +
    ' ORDER BY id;\n'
  )
}
