/**
 * 凭证应用能力公开入口（core/vault）
 *
 * 对外提供凭证复合组件与常用类型：基础控件仍从 `@/core/ui` 使用，
 * 凭证选择/编辑这类依赖凭证库与 IPC 的复合 UI 一律经本入口引用，
 * 避免基础 UI 反向依赖 Vault 形成环。
 */
export { default as CredentialPicker } from './ui/CredentialPicker.vue'
export { default as CredentialForm } from './ui/CredentialForm.vue'
export type { CredentialFormState } from './useVault'
export type { CredentialKind, CredentialSummary } from '@/core/ipc/contracts'
