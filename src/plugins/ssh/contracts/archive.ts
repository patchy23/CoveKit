/** SSH 归档 DTO，与后端 models/archive.rs 同步。 */
export interface ArchiveRequest {
  operation: 'compress' | 'extract' | 'preview'
  format: 'zip' | 'gz' | 'tar.gz'
  paths: string[]
  output: string
}
export interface ArchiveEntry {
  path: string
  size: number | null
  isDir: boolean
  modifiedAt: number
}
export interface ArchiveEvent {
  kind: 'queued' | 'progress' | 'entries' | 'error' | 'result'
  transferred?: number | null
  total?: number | null
  entries?: ArchiveEntry[] | null
  status?: 'succeeded' | 'failed' | 'cancelled' | null
  error?: string | null
}
