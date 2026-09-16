/**
 * IPC 契约 · 框架级（窗口 / 设置 / 外链）
 * 业务插件的 IPC 契约定义在各插件目录 src/plugins/<id>/contracts.ts
 * （插件隔离：新增/修改插件契约不动本文件与其它插件）。
 */

// ── 出参结构 ──

/**
 * 应用设置（Rust framework/settings.rs 全量读写）
 *
 * 本文件是 AppSettings 的唯一事实源（`core/registry/types.ts` 只做再导出）。
 */
export interface AppSettings {
  theme: 'light' | 'dark' | 'system'
  language: 'zh-CN' | 'en-US'
  launchAtStartup: boolean
  /** 工具箱级默认下载目录；各工具的保存对话框优先从这里打开。 */
  defaultDownloadDirectory: string
  tools: Record<string, Record<string, unknown>>
}

/**
 * 空间级用户数据键（收藏、最近使用）
 *
 * 这两项是**用户数据**而非设置字段：与设置一样随空间隔离、落当前空间偏好文件，
 * 但经 `preferences_get` / `preferences_set` 读写，不递增设置版本号。
 * 取值必须与 Rust `framework::settings::SPACE_DATA_KEYS` 白名单一致。
 */
export type SpaceDataKey = 'favorites' | 'recentTools'

/**
 * 工具级设置字段（判别联合：类型决定可用参数）
 *
 * 说明：**没有 secret 类型**。普通设置文件是明文，秘密材料必须走凭证管理
 * （`framework::vault`）；历史 schema 若声明 secret，前端不渲染、Rust 侧也会按敏感键名拒绝写入。
 * 数字字段可声明 min/max，越界由渲染层与保存前校验一起拦截。
 */
export type SettingsField =
  | { key: string; type: 'toggle'; label: string; default?: boolean }
  | { key: string; type: 'text'; label: string; default?: string }
  | {
      key: string
      type: 'number'
      label: string
      default?: number
      min?: number
      max?: number
      step?: number
    }
  | {
      key: string
      type: 'select'
      label: string
      default?: string
      options: { label: string; value: string }[]
    }

/** 更新可用性（占位公钥等无效配置按不可用上报） */
export interface UpdateAvailability {
  available: boolean
  /** 不可用原因（可直接展示；可用时为空串） */
  reason: string
  /** 更新通道（下载地址） */
  channel: string
}

/** 框架长任务事件名（与 Rust `framework::tasks::TASK_EVENT` 一致；负载为 TaskSnapshot） */
export const FRAMEWORK_TASK_EVENT = 'framework://task'

/** 框架任务状态（与 Rust `framework::tasks::TaskState` 同步） */
export type TaskState = 'running' | 'succeeded' | 'failed'

/** 任务错误：稳定 code + 面向用户 message（Rust `TaskError`） */
export interface TaskError {
  /** 稳定错误码（如 storage.migrate.failed），界面与诊断按它归类 */
  code: string
  /** 面向用户的说明，可直接展示 */
  message: string
}

/** 任务快照（Rust `TaskSnapshot`） */
export interface TaskSnapshot {
  id: string
  /** 归属模块，如 storage */
  owner: string
  /** 任务类型，如 storage.migrate */
  kind: string
  state: TaskState
  /** 进度百分比 0..=100；不可估算时为 null，不假装 0 */
  progress: number | null
  /** 是否允许取消 */
  cancellable: boolean
  /** 不可取消的原因（cancellable 为 true 时为 null） */
  cancellableReason: string | null
  error: TaskError | null
  /** 登记时间（epoch 毫秒） */
  startedAt: number
  /** 最近变更时间（epoch 毫秒） */
  updatedAt: number
}

/** 任务清单（Rust `TaskList`） */
export interface TaskList {
  active: TaskSnapshot[]
  finished: TaskSnapshot[]
}

/** 窗口状态 */
export interface WindowState {
  visible: boolean
}

/** 关闭决策（与 Rust `framework::lifecycle::CloseDecision` 的 serde camelCase 字段同步） */
export interface CloseDecision {
  /** 是否允许关闭（false = 被业务拒绝，进程仍在运行、页签仍在） */
  proceed: boolean
  /** 是否由用户强制关闭触发（跳过业务拦截） */
  forced: boolean
  /** 拒绝原因（形如 `owner: 原因`，两侧 blockers 合并后直接可展示） */
  blockers: string[]
  /** 清理失败原因（只在提交段返回：允许关闭但某个模块没清干净） */
  failures: string[]
}

// ── Vault 凭证管理（src-tauri framework/vault，serde camelCase 同步）──

/** 凭证类型（kebab-case，与 Rust CredentialKind 同步；数据库凭证复用 password） */
export type CredentialKind = 'password' | 'ssh-key' | 'api-token' | 'access-key-pair' | 'custom'

/** 自定义键值条目（custom 类型 fields 的元素） */
export interface CustomEntry {
  key: string
  value: string
  /** 是否秘密值（true = 列表掩码、复制走 vault_reveal） */
  secret: boolean
}

/** 凭证秘密字段（serde 内部标签 type 按 kind 分派） */
export type CredentialFields =
  | { type: 'password'; username: string; password: string }
  | { type: 'ssh-key'; username: string; privateKey: string; passphrase: string | null }
  | { type: 'api-token'; token: string }
  | { type: 'access-key-pair'; accessKeyId: string; accessKeySecret: string }
  | { type: 'custom'; entries: CustomEntry[] }

/** 凭证条目（含明文：仅 vault_reveal / vault_save 路径出现，列表永远走摘要） */
export interface Credential {
  id: string
  name: string
  kind: CredentialKind
  fields: CredentialFields
  note: string
  createdAt: number
  updatedAt: number
}

/** 凭证脱敏摘要（vault_list 返回项，无明文秘密） */
export interface CredentialSummary {
  id: string
  name: string
  kind: CredentialKind
  /** 掩码摘要（如 AKI****xyz / 用户名 / 「N 个字段」） */
  masked: string
  note: string
  createdAt: number
  updatedAt: number
}

/** vault_save 入参（id 为 null = 新增） */
export interface CredentialSavePayload {
  id: string | null
  name: string
  kind: CredentialKind
  fields: CredentialFields
  note: string
}

/** vault_delete 返回 */
export interface VaultDeleteResult {
  ok: boolean
  error: string | null
  /** 仍引用该凭证的插件对象数量（后端删除前重新核对的结果） */
  referencedBy: number
}

/** 一条凭证引用：哪个插件的哪个对象在用 */
export interface CredentialReferenceItem {
  owner: string
  credentialId: string
  objectId: string
  objectName: string
}

/** 单插件扫描结果（status.state=unknown 表示该插件计数未知，必须提示而不是当成无引用） */
export interface CredentialReferenceOwner {
  owner: string
  references: CredentialReferenceItem[]
  status: { state: 'ok' } | { state: 'unknown'; reason: string }
}

/** 凭证引用概况（按插件自报能力批量扫描） */
export interface CredentialReferenceSummary {
  credentialId: string
  owners: CredentialReferenceOwner[]
  /** 扫描成功的插件里引用总数 */
  total: number
  /** 扫描失败、计数未知的插件 */
  unknownOwners: string[]
}

/** vault_import 返回 */
export interface VaultImportResult {
  imported: number
  /** 合并模式下同 id 冲突跳过的条数 */
  skipped: number
}

/** 主密钥实际保护后端（kebab-case，与 Rust ProtectionBackend 同步） */
export type VaultProtectionBackend = 'system-keyring' | 'file-fallback' | 'unavailable'

/** 单个数据域的可用性（kebab-case，与 Rust ProtectionAvailability 同步） */
export type VaultProtectionAvailability = 'available' | 'locked' | 'uninitialized'

/** 单个数据域（vault / credentials）的保护状态 */
export interface VaultDomainProtection {
  /** 域名：`vault` / `credentials` */
  domain: string
  backend: VaultProtectionBackend
  availability: VaultProtectionAvailability
  /** 走降级路径或锁定时的原因（无秘密，可直接展示；正常情况为 null） */
  fallbackReason: string | null
  /** 系统密钥库里是否有该域的主密钥记录 */
  keyringHasKey: boolean
  /** 本地降级密钥文件是否存在 */
  fallbackFileExists: boolean
  /** 该域是否已有密文（主文件或备份） */
  ciphertextExists: boolean
}

/** vault_protection_status 返回（T04-5：设置页持久展示「到底有没有系统级保护」） */
export interface VaultProtectionStatus {
  /** 本平台是否编译进系统密钥库原生后端（false = 只能降级到本机文件，绝不显示为系统保护） */
  nativeBackend: boolean
  domains: VaultDomainProtection[]
}

// ── 存储位置（src-tauri framework/storage，serde camelCase 同步）──

/** 单个分区（data / vault / logs / cache）的路径与占用 */
export interface StoragePartitionInfo {
  name: string
  path: string
  bytes: number
  fileCount: number
}

/** 存储位置信息（storage_info 返回） */
export interface StorageInfo {
  /** 当前生效的存储根目录 */
  root: string
  /** 是否使用默认根目录（app_data_dir）
   * */
  isDefault: boolean
  /** 默认根目录（「恢复默认」的目标） */
  defaultRoot: string
  /** 四分区明细 */
  partitions: StoragePartitionInfo[]
  /** 四分区合计字节数 */
  totalBytes: number
  /** 四分区合计文件数 */
  fileCount: number
  /** 已完成的布局版本（等于 paths::LAYOUT_VERSION 表示已是四分区布局） */
  layoutVersion: number
  /** 非秘密空间标识（数据上下文；默认空间在导入导出交付前恒为 default） */
  spaceId: string
  /** 空间代际（导入激活/空间切换后递增） */
  generationId: number
  /** 待执行的迁移计划（重启后执行；运行期不换根） */
  pendingMigration: StoragePendingMigration | null
  /** 最近一次成功迁移的留档 */
  lastMigration: StorageLastMigration | null
  /** 恢复状态（配置盘不可用或迁移失败；非空时前端显示恢复页） */
  recovery: StorageRecovery | null
}

/** 迁移阶段（崩溃后靠阶段值识别半截状态） */
export type StorageMigrationPhase = 'scheduled' | 'copying' | 'verifying'

/** 待执行的迁移计划（重启后由维护阶段执行；运行期不换根） */
export interface StoragePendingMigration {
  /** 计划标识 */
  id: string
  /** 源根目录（登记时的生效根） */
  source: string
  /** 目标根目录 */
  target: string
  /** 登记时的布局版本 */
  layoutVersion: number
  /** 当前阶段 */
  phase: StorageMigrationPhase
  /** 登记时间（Unix 毫秒） */
  createdAt: number
  /** 启动尝试次数 */
  attempts: number
  /** 最近一次失败原因（成功提交时随计划清除） */
  lastError: string | null
}

/** 最近一次成功迁移的留档 */
export interface StorageLastMigration {
  id: string
  source: string
  target: string
  completedAt: number
  copiedFiles: number
  copiedBytes: number
}

/** 存储恢复原因 */
export type StorageRecoveryReason = 'configuredRootUnavailable' | 'migrationFailed'

/** 存储恢复状态（配置盘不可用或迁移失败） */
export interface StorageRecovery {
  reason: StorageRecoveryReason
  /** 配置里指向的数据目录 */
  configuredRoot: string
  /** 本次运行生效的根（恢复态下等于 configuredRoot，不回退默认目录） */
  activeRoot: string
  /** 关联的迁移计划标识 */
  planId: string | null
  /** 面向用户的说明 */
  detail: string
  canRetry: boolean
  canUseDefault: boolean
  createdAt: number
}

/** 迁移进度事件负载（事件名 `storage://progress`） */
export interface StorageMigrateProgress {
  phase: 'copy' | 'verify' | 'done'
  copiedFiles: number
  copiedBytes: number
  totalBytes: number
}

/** 安排迁移的结果（登记成功即返回；复制在重启后执行） */
export interface StorageScheduleResult {
  ok: boolean
  planId: string
  target: string
  source: string
  /** 目标目录登记时是否非空（非空按合并写入） */
  message: string
}

/** 恢复动作结果（动作一律需要重启生效） */
export interface RecoveryActionResult {
  ok: boolean
  restartRequired: boolean
  message: string
}

/** 恢复动作类型 */
export type StorageRecoveryAction = 'retry' | 'use-default' | 'choose'

// ── 数据导出导入（框架命令，src-tauri framework/data_transfer） ──

/** 数据传输策略（与 Rust `TransportPolicy` 逐字对应，serde kebab-case） */
export type TransportPolicy =
  'portable' | 'device-local' | 'secret' | 'sensitive-content' | 'history'

/** 本机空间（`data_spaces_list` 条目；只含非秘密信息） */
export interface SpaceSummary {
  spaceId: string
  name: string
  createdAt: string
  active: boolean
  /** 兼容承载位默认空间（旧扁平布局） */
  legacy: boolean
  imported: boolean
  sourceSpaceName?: string | null
  importedAt?: string | null
  counts: Record<string, number>
}

/** 切换活动空间的结果（本批一律需要重启） */
export interface SpaceSwitchResult {
  spaceId: string
  name: string
  restartRequired: boolean
  revision: number
}

/** 依赖边（如档案 → 凭证） */
export interface DatasetDependencyEdge {
  kind: string
  fromId: string
  toId: string
}

/** 可勾选条目（导出目录列表项） */
export interface ExportCatalogEntry {
  dataset: string
  id: string
  label: string
  detail: string
  dependencies: DatasetDependencyEdge[]
  note?: string | null
}

/** 数据集摘要（导出目录卡片） */
export interface DatasetSummary {
  name: string
  label: string
  owner: string
  policy: TransportPolicy
  schemaVersion: number
  selectable: boolean
  containsSecret: boolean
  defaultSelected: boolean
  note?: string | null
  recordCount: number
}

/** 按条勾选的数据集 */
export interface ExportSelectionEntry {
  dataset: string
  ids: string[]
}

/** 导出选择（前端提交；含秘密类别需显式确认） */
export interface ExportSelection {
  entries: ExportSelectionEntry[]
  datasets: string[]
  includeCredentials: boolean
}

/** 导出目录（当前空间可导出集合） */
export interface ExportCatalog {
  sourceSpaceId: string
  sourceSpaceName: string
  datasets: DatasetSummary[]
  entries: ExportCatalogEntry[]
  defaults: ExportSelection
  warnings: string[]
}

/** 导出报告 */
export interface ExportReport {
  path: string
  bytes: number
  packageId: string
  sourceSpaceName: string
  counts: Record<string, number>
  excluded: string[]
  secretIncluded: boolean
}

/** 传输启动结果（导出） */
export interface TransferStart {
  taskId: string
  report: ExportReport
}

/** 包内一个数据集的展示视图 */
export interface PackageDatasetView {
  name: string
  label: string
  policy: TransportPolicy
  schemaVersion: number
  recordCount: number
  carried: boolean
  supported: boolean
  reason?: string | null
}

/** 包摘要（不可信字段只用于展示） */
export interface PackageSummaryView {
  packageId: string
  sourceSpaceId: string
  sourceSpaceName: string
  createdAt: string
  appVersion: string
  platform: string
  datasets: PackageDatasetView[]
  excluded: string[]
}

/** 二次导入提示 */
export interface DuplicateHint {
  spaceId: string
  spaceName: string
  importedAt: string
}

/** 导入后的记录处置结论 */
export type ImportOutcome = 'added' | 'pending-reference' | 'excluded'

/** 导入计划条目（预览列表项） */
export interface ImportPlanItem {
  dataset: string
  id: string
  label: string
  outcome: ImportOutcome
  note?: string | null
}

/** 导入选择 */
export interface ImportSelection {
  datasets: string[]
}

/** 导入报告 */
export interface ImportReport {
  spaceId: string
  spaceName: string
  packageId: string
  sourceSpaceId: string
  sourceSpaceName: string
  importedAt: string
  counts: Record<string, number>
  declaredCounts: Record<string, number>
  pending: string[]
  excluded: string[]
}

/** 校验数据包的结果 */
export interface ImportInspectResult {
  inspectId: string
  summary: PackageSummaryView
  defaults: ImportSelection
  preview: ImportPlanItem[]
  pending: string[]
  excluded: string[]
  duplicate?: DuplicateHint | null
}

/** 规划导入的结果 */
export interface ImportPlanResult {
  planId: string
  spaceId: string
  spaceName: string
  preview: ImportPlanItem[]
  pending: string[]
  excluded: string[]
  counts: Record<string, number>
}

/** 提交导入的结果 */
export interface ImportCommitResult {
  taskId: string
  report: ImportReport
}

/** 取消传输的结果 */
export interface CancelResult {
  cancelled: boolean
}

// ── 框架命令清单 ──

export const frameworkCommands = {
  settingsGet: 'settings_get',
  settingsSet: 'settings_set',
  settingsPatch: 'settings_patch',
  settingsSetTool: 'settings_set_tool',
  settingsRevision: 'settings_revision',
  preferencesGet: 'preferences_get',
  preferencesSet: 'preferences_set',
  updateAvailability: 'update_availability',
  windowToggle: 'window_toggle',
  windowHide: 'window_hide',
  // 关闭协商（框架命令，src-tauri framework/exit；页签关闭与退出共用一条裁决链）
  appRequestClose: 'app_request_close',
  appCommitClose: 'app_commit_close',
  appForceExit: 'app_force_exit',
  frameworkTasks: 'framework_tasks',
  openExternal: 'open_external',
  frameworkCommandsList: 'framework_commands',
  // 存储位置（框架命令，src-tauri framework/storage）
  storageInfo: 'storage_info',
  storageScheduleMigration: 'storage_schedule_migration',
  storageCancelMigration: 'storage_cancel_migration',
  storageRecoveryStatus: 'storage_recovery_status',
  storageRecoveryAction: 'storage_recovery_action',
  // Vault 凭证管理（框架命令，src-tauri framework/vault）
  vaultList: 'vault_list',
  vaultSave: 'vault_save',
  vaultDelete: 'vault_delete',
  vaultCredentialReferences: 'vault_credential_references',
  vaultReveal: 'vault_reveal',
  vaultProtectionStatus: 'vault_protection_status',
  vaultExport: 'vault_export',
  vaultImport: 'vault_import',
  // 数据导出导入（框架命令，src-tauri framework/data_transfer，sync L2）
  dataSpacesList: 'data_spaces_list',
  dataSpaceSwitch: 'data_space_switch',
  dataExportCatalog: 'data_export_catalog',
  dataExportStart: 'data_export_start',
  dataImportInspect: 'data_import_inspect',
  dataImportPlan: 'data_import_plan',
  dataImportCommit: 'data_import_commit',
  dataTransferCancel: 'data_transfer_cancel',
} as const

/** 框架命令入参（Record<string, never> = 无参命令） */
export type FrameworkPayloads = {
  settings_get: { key?: string }
  settings_set: { key: string; value: unknown }
  settings_patch: { revision?: number; patch: Record<string, unknown> }
  settings_set_tool: { tool: string; key: string; value: unknown }
  settings_revision: Record<string, never>
  preferences_get: { key: SpaceDataKey }
  preferences_set: { key: SpaceDataKey; value: unknown }
  update_availability: Record<string, never>
  window_toggle: Record<string, never>
  window_hide: Record<string, never>
  app_request_close: { reason: string; toolId?: string; blockers?: string[]; force?: boolean }
  app_commit_close: { reason: string; toolId?: string; force?: boolean }
  app_force_exit: Record<string, never>
  framework_tasks: Record<string, never>
  open_external: { url: string }
  framework_commands: Record<string, never>
  storage_info: Record<string, never>
  storage_schedule_migration: { target: string }
  storage_cancel_migration: Record<string, never>
  storage_recovery_status: Record<string, never>
  storage_recovery_action: { action: StorageRecoveryAction; target?: string }
  vault_list: Record<string, never>
  vault_save: { payload: CredentialSavePayload }
  vault_delete: { id: string; force?: boolean; expectedReferences?: number }
  vault_credential_references: { id: string }
  vault_reveal: { id: string }
  vault_protection_status: Record<string, never>
  vault_export: { path: string; password: string }
  vault_import: { path: string; password: string; overwrite: boolean }
  data_spaces_list: Record<string, never>
  data_space_switch: { spaceId: string; revision?: number }
  data_export_catalog: Record<string, never>
  data_export_start: { selection: ExportSelection; password: string; path: string }
  data_import_inspect: { path: string; password: string }
  data_import_plan: {
    inspectId: string
    selection: ImportSelection
    newSpaceName: string
    allowDuplicate?: boolean
  }
  data_import_commit: { planId: string; password: string }
  data_transfer_cancel: Record<string, never>
}

/** 框架命令返回 */
export type FrameworkResults = {
  settings_get: AppSettings
  settings_set: number
  settings_patch: number
  settings_set_tool: number
  settings_revision: number
  preferences_get: unknown
  preferences_set: void
  update_availability: UpdateAvailability
  window_toggle: WindowState
  window_hide: void
  app_request_close: CloseDecision
  app_commit_close: CloseDecision
  app_force_exit: CloseDecision
  framework_tasks: TaskList
  open_external: void
  framework_commands: { name: string; doc: string }[]
  storage_info: StorageInfo
  storage_schedule_migration: StorageScheduleResult
  storage_cancel_migration: boolean
  storage_recovery_status: StorageRecovery | null
  storage_recovery_action: RecoveryActionResult
  vault_list: CredentialSummary[]
  vault_save: CredentialSummary
  vault_delete: VaultDeleteResult
  vault_credential_references: CredentialReferenceSummary
  vault_reveal: Credential
  vault_protection_status: VaultProtectionStatus
  vault_export: void
  vault_import: VaultImportResult
  data_spaces_list: SpaceSummary[]
  data_space_switch: SpaceSwitchResult
  data_export_catalog: ExportCatalog
  data_export_start: TransferStart
  data_import_inspect: ImportInspectResult
  data_import_plan: ImportPlanResult
  data_import_commit: ImportCommitResult
  data_transfer_cancel: CancelResult
}
