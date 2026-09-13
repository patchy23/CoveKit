/**
 * 框架 · 本地诊断（错误收集 + 仅本地生成的诊断报告）
 *
 * 与遥测无关：不落盘、不上传，内容只在本进程内，用户主动复制才离开本机。
 */
export {
  MAX_ERROR_ENTRIES,
  MAX_MESSAGE_LEN,
  MAX_DETAIL_LEN,
  listErrors,
  clearErrors,
  subscribeErrors,
  recordError,
  normalizeUnknownError,
  type AppErrorEntry,
  type ErrorInput,
} from './errors-core'
export { installErrorCollectors } from './errors'
export {
  collectDiagnostics,
  formatDiagnostics,
  redactPaths,
  type DiagnosticsReport,
} from './report'
