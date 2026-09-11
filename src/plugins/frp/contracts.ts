/**
 * frp 插件 · IPC 契约（本插件私有，独立于框架与其它插件）
 * 与 src-tauri/src/plugins/frp/models.rs 逐字段同步（Rust 侧 serde `camelCase`）。
 * 契约冻结基线：docs/plugins/frp/2026-09-12-任务书.md §3。
 */

/** 档案运行状态：未运行 / 进程中待确认 / 已连接 / 失败 */
export type FrpStateName = 'stopped' | 'starting' | 'running' | 'error'

/** frpc 可执行文件来源：设置项 / PATH / 常见位置 / 工具下载 */
export type FrpBinarySource = 'settings' | 'path' | 'common' | 'downloaded'

/** 日志流来源 */
export type FrpLogStream = 'stdout' | 'stderr'

/** 下载阶段：下载 / 解压 / 校验 / 完成 */
export type FrpDownloadPhase = 'download' | 'extract' | 'verify' | 'done'

/** 新建档案的内置模板 id */
export type FrpTemplateId = 'tcp' | 'http' | 'stcp' | 'empty'

/** 内置模板清单（顺序即下拉顺序；模板正文由 Rust 侧 `frp_profile_create` 生成） */
export const FRP_TEMPLATE_IDS: FrpTemplateId[] = ['tcp', 'http', 'stcp', 'empty']

/** 档案摘要（列表项；元数据来自 frp.db，统计来自 TOML 解析） */
export interface FrpProfileSummary {
  /** 档案文件名（单段文件名，.toml 结尾） */
  fileName: string
  /** 展示名（DB displayName；缺省为文件名去扩展名） */
  displayName: string
  /** 备注（DB remark，不污染用户 TOML） */
  remark: string
  /** 服务器地址（解析失败为空串） */
  serverAddr: string
  /** 服务器端口（解析失败为 0） */
  serverPort: number
  /** 代理条目总数 */
  proxyCount: number
  /** 启用中的代理数 */
  enabledProxyCount: number
  /** 文件修改时间（Unix 毫秒） */
  mtime: number
  /** 当前运行状态 */
  state: FrpStateName
  /** 运行中的进程 id（未运行时不出现） */
  pid?: number
  /** 最近一次错误原因（已脱敏） */
  lastError?: string
}

/** 档案列表返回 */
export interface FrpProfileList {
  ok: boolean
  /** 实际使用的配置目录绝对路径 */
  dir: string
  profiles: FrpProfileSummary[]
  error?: string
}

/** 档案内容返回（源码原文 + TOML→JSON 解析结果，未知段落全量透传） */
export interface FrpProfileContent {
  ok: boolean
  fileName: string
  content: string
  /** TOML 解析结果（JSON 对象；表单模式的数据源） */
  parsed: Record<string, unknown>
  /** 原文是否含注释（表单保存会丢注释，据此二次确认） */
  hasComments: boolean
  error?: string
}

/** 写操作统一返回（新建 / 复制 / 重命名 / 删除 / 保存 / 备注） */
export interface FrpOpResult {
  ok: boolean
  fileName?: string
  error?: string
}

/** 单条校验错误 */
export interface FrpVerifyError {
  /** 出错行号（解析不出时不出现） */
  line?: number
  /** 出错列号（解析不出时不出现） */
  column?: number
  /** 错误信息（已剥 ANSI 与时间戳前缀） */
  message: string
}

/** 校验结果（frpc verify -c <path>） */
export interface FrpVerifyResult {
  ok: boolean
  fileName: string
  /** frpc 原始输出（原样返回，供用户判读） */
  raw: string
  errors: FrpVerifyError[]
}

/** 单个档案的运行状态 */
export interface FrpRuntimeState {
  fileName: string
  state: FrpStateName
  pid?: number
  startedAt?: number
  lastLine?: string
  lastError?: string
  exitCode?: number
}

/** frpc 可执行文件信息 */
export interface FrpBinaryInfo {
  ok: boolean
  path?: string
  version?: string
  source?: FrpBinarySource
  error?: string
}

/** 上游 Release 资产 */
export interface FrpReleaseAsset {
  name: string
  size: number
}

/** 上游 Release 条目（供下载选择） */
export interface FrpReleaseInfo {
  /** 版本号（已去掉前缀 v） */
  version: string
  /** 发布时间（ISO8601 原文） */
  publishedAt: string
  assets: FrpReleaseAsset[]
}

/** 事件 `frp://log` 负载 */
export interface FrpLogPayload {
  fileName: string
  /** 日志行（已剥 ANSI、已脱敏） */
  line: string
  /** 时间戳（Unix 毫秒） */
  ts: number
  stream: FrpLogStream
}

/** 事件 `frp://download` 负载 */
export interface FrpDownloadPayload {
  version: string
  phase: FrpDownloadPhase
  /** 已接收字节数（download 阶段递增） */
  received?: number
  /** 总字节数（服务端未给 Content-Length 时不出现） */
  total?: number
  error?: string
}

/** 命令清单（16 条，全部 `frp_` 前缀） */
export const commands = {
  profilesList: 'frp_profiles_list',
  profileRead: 'frp_profile_read',
  profileSaveText: 'frp_profile_save_text',
  profileSaveForm: 'frp_profile_save_form',
  profileCreate: 'frp_profile_create',
  profileDuplicate: 'frp_profile_duplicate',
  profileRename: 'frp_profile_rename',
  profileDelete: 'frp_profile_delete',
  profileRemark: 'frp_profile_remark',
  verify: 'frp_verify',
  start: 'frp_start',
  stop: 'frp_stop',
  restart: 'frp_restart',
  status: 'frp_status',
  binaryDetect: 'frp_binary_detect',
  binaryVersions: 'frp_binary_versions',
  binaryDownload: 'frp_binary_download',
} as const

/** 命令入参 */
export type Payloads = {
  frp_profiles_list: Record<string, never>
  frp_profile_read: { fileName: string }
  frp_profile_save_text: { fileName: string; content: string }
  frp_profile_save_form: { fileName: string; parsed: Record<string, unknown> }
  frp_profile_create: { fileName: string; template: FrpTemplateId }
  frp_profile_duplicate: { fileName: string; newName: string }
  frp_profile_rename: { fileName: string; newName: string }
  frp_profile_delete: { fileName: string }
  frp_profile_remark: { fileName: string; remark: string }
  frp_verify: { fileName: string }
  frp_start: { fileName: string }
  frp_stop: { fileName: string }
  frp_restart: { fileName: string }
  frp_status: Record<string, never>
  frp_binary_detect: { path?: string }
  frp_binary_versions: { limit?: number }
  frp_binary_download: { version: string }
}

/** 命令返回 */
export type Results = {
  frp_profiles_list: FrpProfileList
  frp_profile_read: FrpProfileContent
  frp_profile_save_text: FrpOpResult
  frp_profile_save_form: FrpOpResult
  frp_profile_create: FrpOpResult
  frp_profile_duplicate: FrpOpResult
  frp_profile_rename: FrpOpResult
  frp_profile_delete: FrpOpResult
  frp_profile_remark: FrpOpResult
  frp_verify: FrpVerifyResult
  frp_start: FrpRuntimeState
  frp_stop: FrpRuntimeState
  frp_restart: FrpRuntimeState
  frp_status: FrpRuntimeState[]
  frp_binary_detect: FrpBinaryInfo
  frp_binary_versions: FrpReleaseInfo[]
  frp_binary_download: FrpBinaryInfo
}
