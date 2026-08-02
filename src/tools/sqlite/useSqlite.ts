/**
 * SQLite 工具 · 纯函数（SQL 识别/单元格显示/路径名）
 */

/** 判断 SQL 是否为查询语句（返回表格） */
export function isQuerySql(sql: string): boolean {
  const t = sql.trim().toUpperCase();
  return ["SELECT", "PRAGMA", "EXPLAIN", "WITH", "SHOW", "DESCRIBE"].some((k) => t.startsWith(k));
}

/** 单元格显示：NULL 特殊样式、空串显示占位 */
export function displayCell(v: string): string {
  if (v === "NULL") return "NULL";
  return v;
}

/** 文件路径取文件名（db 显示名） */
export function fileName(path: string): string {
  const parts = path.split(/[\\/]/);
  return parts[parts.length - 1] || path;
}

/** 结果行数/影响行数人类描述 */
export function describeResult(isQuery: boolean, count: number): string {
  return isQuery ? `${count} 行` : `影响 ${count} 行`;
}
