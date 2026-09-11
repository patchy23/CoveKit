/**
 * 字节数格式化（core 层公共纯函数，SSH / 接口调试 / 编辑器状态栏共用）
 *
 * 由原 `src/core/format.ts` 移入本目录：该单文件与 `src/core/format/` 目录同名共存时，
 * 模块解析会优先命中单文件，导致 `@/core/format` 只能拿到本文件的导出（2026-09-11 实测）。
 * 现统一为目录 + index 的形态，禁止再创建同名的 `format.ts`。
 */

/** 字节数格式化：1024 进制四档（B / KB 1 位小数 / MB / GB 2 位小数） */
export function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`
  if (n < 1024 * 1024 * 1024) return `${(n / 1024 / 1024).toFixed(2)} MB`
  return `${(n / 1024 / 1024 / 1024).toFixed(2)} GB`
}
