/** 文件占用查询契约，与 Rust file_lock/models.rs 同步。 */
export interface FileProcess {
  pid: number
  /** FILETIME 十进制字符串；与 PID 共同标识进程，避免 JS 精度丢失。 */
  startedAt: string
  appName: string
  serviceName: string | null
  /** 无权限、已退出或身份变化时为 null，原因见 detailError。 */
  executablePath: string | null
  processName: string | null
  detailError: string | null
}

export interface FileLockResult {
  path: string
  /** 本次快照中的使用者，不保证阻止删除；空列表不保证文件未被占用。 */
  processes: FileProcess[]
}

export const commands = {
  supported: 'file_lock_supported',
  query: 'file_lock_query',
  terminate: 'file_lock_terminate',
} as const

export type Payloads = {
  file_lock_supported: Record<string, never>
  file_lock_query: { path: string }
  file_lock_terminate: { path: string; pid: number; startedAt: string }
}

export type Results = {
  file_lock_supported: boolean
  file_lock_query: FileLockResult
  file_lock_terminate: void
}
