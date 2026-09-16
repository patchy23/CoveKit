/**
 * 框架 · 工具/应用生命周期（关闭协商、作用域副作用、可见性、退出）
 *
 * 唯一入口原则：关闭工具走 `negotiateToolClose`（或经 ui store 的 `requestClose`），
 * 退出应用走 `requestAppExit`；插件不直接操作页签数组，也不直接调原生退出能力。
 */
export {
  collectAllToolBlockers,
  collectToolBlockers,
  DEFAULT_DISPOSE_TIMEOUT_MS,
  disposeAllTools,
  disposeToolOwners,
  hasOwners,
  negotiateToolClose,
  registerToolOwner,
  registeredToolIds,
  resetToolOwnersForTest,
  toolCloseState,
  watchToolCloseState,
  type ToolOwnerHandle,
} from './toolContext'
export { closeIssuesFromBackend, toBlockerLines, type CloseBridgeRequest } from './closeBridge'
export {
  watchToolVisibility,
  publishGlobalHidden,
  publishToolVisibility,
  resetToolVisibilityForTest,
  toolVisibility,
  HIDDEN_VISIBILITY,
  type ToolVisibility,
} from './toolVisibility'
export {
  createScope,
  scopeStats,
  throttledInterval,
  type Scope,
  type ScopeDisposeResult,
  type ScopeStats,
} from './scope'
export {
  EXIT_VETO_EVENT,
  forceAppExit,
  requestAppExit,
  watchExitVeto,
  type CloseDecision,
} from './appClose'
export {
  useToolLifecycle,
  useToolScope,
  type ToolLifecycle,
  type ToolLifecycleSpec,
} from './useToolLifecycle'
export type {
  CloseIssue,
  CloseOutcome,
  CloseReason,
  DisposeFn,
  PrepareFn,
  ToolCloseState,
} from './types'
