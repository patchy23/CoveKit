/** SSH 契约 · 文件（条目/列表/传输进度/远程编辑） */

/* ── 文件管理 ── */

/** 远程文件条目 */
export interface RemoteFile {
  /** 文件名 */
  name: string
  /** 完整路径 */
  path: string
  /** 是否目录 */
  isDir: boolean
  /** 文件大小（字节，目录为 0） */
  size: number
  /** 修改时间（毫秒时间戳） */
  modifiedAt: number
  /** 权限字符串（如 drwxr-xr-x） */
  permissions: string
  /** 所有者 */
  owner: string
  /** 所属组 */
  group: string
}

/** 文件列表结果 */
export interface FileListResult {
  ok: boolean
  /** 当前路径 */
  path: string
  /** 父路径（根目录为 null） */
  parentPath?: string
  /** 文件列表 */
  files: RemoteFile[]
  error?: string
}

/** 文件传输进度 */
export interface FileTransferProgress {
  /** 本次传输唯一标识 */
  transferId: string
  /** 所属 SSH 连接会话 id */
  connectionId: string
  /** 本地路径 */
  localPath: string
  /** 远程路径 */
  remotePath: string
  /** 已传输字节 */
  transferred: number
  /** 总字节 */
  total: number
  /** 是否完成 */
  done: boolean
  /** 是否失败 */
  error?: string
}

/* ── 远程编辑 ── */

/** 远程文件内容 */
export interface RemoteFileContent {
  ok: boolean
  /** 文件路径 */
  path: string
  /** 文件内容（文本） */
  content: string
  /** 文件大小（字节） */
  size: number
  /** 编码（如 UTF-8 / GBK） */
  encoding: string
  /** 修改时间（毫秒；编辑器乐观锁基线） */
  modifiedAt?: number
  error?: string
}

/** 远程编辑保存结果（conflict=true 表示远端已被修改，未写入） */
export interface EditSaveResult {
  ok: boolean
  error?: string
  conflict?: boolean
  /** 冲突时远端当前 mtime（毫秒） */
  currentMtime?: number
}

/* ── 资源监控 ── */