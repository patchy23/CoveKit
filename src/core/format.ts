/**
 * 通用格式化纯函数（core 层，各插件共用）
 * formatBytes：四档（B/KB/MB/GB），原 ssh 版为全集，http-ws 三档版已废弃并入（>1GB 才出现行为差异）。
 */

/** 字节数格式化：1024 进制四档（B / KB 1 位小数 / MB / GB 2 位小数） */
export function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`
  if (n < 1024 * 1024 * 1024) return `${(n / 1024 / 1024).toFixed(2)} MB`
  return `${(n / 1024 / 1024 / 1024).toFixed(2)} GB`
}
