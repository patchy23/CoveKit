/**
 * IPC 类型安全调用封装 + 错误归一化（框架级）
 * - 框架命令封装见下方 `ipc`；
 * - 业务插件在各自 ipc.ts 用 invokeCommand 封装自己的命令（命令名/类型见插件 contracts.ts）。
 */
import { invoke } from '@tauri-apps/api/core'
import type {
  CredentialSavePayload,
  ExportSelection,
  FrameworkPayloads,
  FrameworkResults,
  ImportSelection,
  SpaceDataKey,
  StorageRecoveryAction,
} from './contracts'

/** 归一化 IPC 错误：Tauri 侧错误可能是任意字符串/对象 */
export class IpcError extends Error {
  constructor(command: string, detail: string) {
    super(`[${command}] ${detail}`)
    this.name = 'IpcError'
  }
}

/** 通用命令调用（插件 ipc.ts 使用；命令名与出入参由插件契约约束） */
export async function invokeCommand<P = Record<string, never>, R = void>(
  command: string,
  payload?: P
): Promise<R> {
  try {
    return await invoke<R>(command, (payload ?? {}) as Record<string, unknown>)
  } catch (err) {
    const detail = typeof err === 'string' ? err : err instanceof Error ? err.message : String(err)
    throw new IpcError(command, detail)
  }
}

async function call<K extends keyof FrameworkPayloads & keyof FrameworkResults>(
  command: K,
  payload?: FrameworkPayloads[K]
): Promise<FrameworkResults[K]> {
  return invokeCommand<FrameworkPayloads[K], FrameworkResults[K]>(command, payload)
}

/** 框架命令封装（设置/窗口/外链/命令清单/Vault 凭证） */
export const ipc = {
  settingsGet: (key?: string) => call('settings_get', { key }),
  settingsSet: (key: string, value: unknown) => call('settings_set', { key, value }),
  settingsPatch: (patch: Record<string, unknown>, revision?: number) =>
    call('settings_patch', { revision, patch }),
  settingsSetTool: (tool: string, key: string, value: unknown) =>
    call('settings_set_tool', { tool, key, value }),
  settingsRevision: () => call('settings_revision', {}),
  preferencesGet: (key: SpaceDataKey) => call('preferences_get', { key }),
  preferencesSet: (key: SpaceDataKey, value: unknown) => call('preferences_set', { key, value }),
  updateAvailability: () => call('update_availability', {}),
  windowToggle: () => call('window_toggle', {}),
  windowHide: () => call('window_hide', {}),
  openExternal: (url: string) => call('open_external', { url }),
  frameworkCommandsList: () => call('framework_commands', {}),
  frameworkTasks: () => call('framework_tasks', {}),
  storageInfo: () => call('storage_info', {}),
  storageScheduleMigration: (target: string) => call('storage_schedule_migration', { target }),
  storageCancelMigration: () => call('storage_cancel_migration', {}),
  storageRecoveryStatus: () => call('storage_recovery_status', {}),
  storageRecoveryAction: (action: StorageRecoveryAction, target?: string) =>
    call('storage_recovery_action', { action, target }),
  vaultList: () => call('vault_list', {}),
  vaultSave: (payload: CredentialSavePayload) => call('vault_save', { payload }),
  vaultDelete: (id: string, options?: { force?: boolean; expectedReferences?: number }) =>
    call('vault_delete', { id, ...options }),
  vaultCredentialReferences: (id: string) => call('vault_credential_references', { id }),
  vaultReveal: (id: string) => call('vault_reveal', { id }),
  vaultProtectionStatus: () => call('vault_protection_status', {}),
  vaultExport: (path: string, password: string) => call('vault_export', { path, password }),
  vaultImport: (path: string, password: string, overwrite: boolean) =>
    call('vault_import', { path, password, overwrite }),
  // 数据导出导入（sync L2）
  dataSpacesList: () => call('data_spaces_list', {}),
  dataSpaceSwitch: (spaceId: string, revision?: number) =>
    call('data_space_switch', { spaceId, revision }),
  dataExportCatalog: () => call('data_export_catalog', {}),
  dataExportStart: (selection: ExportSelection, password: string, path: string) =>
    call('data_export_start', { selection, password, path }),
  dataImportInspect: (path: string, password: string) =>
    call('data_import_inspect', { path, password }),
  dataImportPlan: (
    inspectId: string,
    selection: ImportSelection,
    newSpaceName: string,
    allowDuplicate?: boolean
  ) => call('data_import_plan', { inspectId, selection, newSpaceName, allowDuplicate }),
  dataImportCommit: (planId: string, password: string) =>
    call('data_import_commit', { planId, password }),
  dataTransferCancel: (taskId?: string | null) =>
    call('data_transfer_cancel', { taskId: taskId ?? null }),
}
