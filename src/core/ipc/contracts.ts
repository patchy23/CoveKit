/**
 * IPC 契约 · 框架级（窗口 / 设置 / 外链）
 * 业务插件的 IPC 契约定义在各插件目录 src/plugins/<id>/contracts.ts
 * （插件隔离：新增/修改插件契约不动本文件与其它插件）。
 */

// ── 出参结构 ──

/** 应用设置（Rust framework/settings.rs 全量读写） */
export interface AppSettings {
  theme: 'light' | 'dark' | 'system'
  language: 'zh-CN' | 'en-US'
  globalHotkey: string
  launchAtStartup: boolean
  /** 工具箱级默认下载目录；各工具的保存对话框优先从这里打开。 */
  defaultDownloadDirectory: string
  recentTools: string[]
  tools: Record<string, Record<string, unknown>>
}

/** 窗口状态 */
export interface WindowState {
  visible: boolean
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
  /** 仍引用该凭证的插件 profile 数量（引用扫描随设计 §6 迁移接入） */
  referencedBy: number
}

/** vault_import 返回 */
export interface VaultImportResult {
  imported: number
  /** 合并模式下同 id 冲突跳过的条数 */
  skipped: number
}

// ── 框架命令清单 ──

export const frameworkCommands = {
  settingsGet: 'settings_get',
  settingsSet: 'settings_set',
  windowToggle: 'window_toggle',
  windowHide: 'window_hide',
  openExternal: 'open_external',
  frameworkCommandsList: 'framework_commands',
  // Vault 凭证管理（框架命令，src-tauri framework/vault）
  vaultList: 'vault_list',
  vaultSave: 'vault_save',
  vaultDelete: 'vault_delete',
  vaultReveal: 'vault_reveal',
  vaultExport: 'vault_export',
  vaultImport: 'vault_import',
} as const

/** 框架命令入参（Record<string, never> = 无参命令） */
export type FrameworkPayloads = {
  settings_get: { key?: string }
  settings_set: { key: string; value: unknown }
  window_toggle: Record<string, never>
  window_hide: Record<string, never>
  open_external: { url: string }
  framework_commands: Record<string, never>
  vault_list: Record<string, never>
  vault_save: { payload: CredentialSavePayload }
  vault_delete: { id: string }
  vault_reveal: { id: string }
  vault_export: { path: string; password: string }
  vault_import: { path: string; password: string; overwrite: boolean }
}

/** 框架命令返回 */
export type FrameworkResults = {
  settings_get: AppSettings
  settings_set: void
  window_toggle: WindowState
  window_hide: void
  open_external: void
  framework_commands: { name: string; doc: string }[]
  vault_list: CredentialSummary[]
  vault_save: CredentialSummary
  vault_delete: VaultDeleteResult
  vault_reveal: Credential
  vault_export: void
  vault_import: VaultImportResult
}
