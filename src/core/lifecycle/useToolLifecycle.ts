/**
 * 框架 · 插件侧工具生命周期组合式函数（可靠性 T10-1/T10-4/T10-7）
 *
 * 插件只写「我的状态是什么」和「怎么清理」，其余交给框架：
 * - 可见性（激活 / 被设置页覆盖 / 窗口隐藏）以响应式 `visibility` 提供；
 * - 未保存与运行中标记进关闭协商，界面关闭时用户看得到原因；
 * - dispose 回调在关闭工具与退出应用时都会被调用；
 * - `scope` 给订阅与定时器，卸载时统一释放（不会留下关不掉的轮询）。
 */
import { onUnmounted, ref, watch, type Ref } from 'vue'
import { createScope, type Scope } from './scope'
import { registerToolOwner } from './toolContext'
import { toolVisibility, watchToolVisibility, type ToolVisibility } from './toolVisibility'
import type { DisposeFn, PrepareFn } from './types'

/** 插件登记的声明：清理由插件提供，框架只协调 */
export interface ToolLifecycleSpec {
  /** owner 名（同一工具内多份资源各用各的；默认取 toolId） */
  owner?: string
  /** 关闭前询问：返回非空字符串表示拒绝关闭 */
  prepare?: PrepareFn
  /** 关闭 / 退出时的清理 */
  dispose?: DisposeFn
}

/** 组合式函数返回值：状态标记 + 作用域 + 当前可见性 */
export interface ToolLifecycle {
  /** 有未保存内容（驱动关闭协商的兜底拒绝理由） */
  dirty: Ref<boolean>
  /** 有进行中的任务（关闭工具时会被拦下询问） */
  running: Ref<boolean>
  /** 作用域：订阅事件、建定时器都从这里走，卸载自动释放 */
  scope: Scope
  /** 当前可见性（激活 / 覆盖 / 窗口隐藏） */
  visibility: Ref<ToolVisibility>
  /** 页面/窗口重新可见时的回调（隐藏期间跳过的刷新在此补齐） */
  onResume: (fn: () => void) => void
}

/**
 * 只取作用域与可见性（不需要关闭协商登记的组件用）。
 *
 * 典型场景：工具内部的某个面板要轮询，但它的资源随面板卸载就结束，
 * 不需要参与「关闭工具」询问——那就别登记 owner，避免每个面板都往协商里塞条目。
 *
 * @param toolId 工具 id（可见性按工具维度发布）
 * @param name 作用域名（诊断用，默认取工具 id）
 */
export function useToolScope(
  toolId: string,
  name: string = toolId
): Pick<ToolLifecycle, 'scope' | 'visibility' | 'onResume'> {
  const scope = createScope(name, toolId)
  const visibility = ref<ToolVisibility>(toolVisibility(toolId))

  const stopWatch = watchToolVisibility(toolId, (state) => {
    visibility.value = state
  })
  visibility.value = toolVisibility(toolId)

  onUnmounted(() => {
    stopWatch()
    void scope.dispose()
  })

  return { scope, visibility, onResume: (fn: () => void) => scope.onResume(fn) }
}

/**
 * 在工具根组件里调用一次。
 *
 * @param toolId 工具 id（与页签 id 一致）
 * @param spec 关闭协商声明（prepare/dispose 可省略，省略时按 dirty/running 标记兜底）
 */
export function useToolLifecycle(toolId: string, spec: ToolLifecycleSpec = {}): ToolLifecycle {
  const handle = registerToolOwner(toolId, spec.owner ?? toolId)
  const { scope, visibility, onResume } = useToolScope(toolId)
  const dirty = ref(false)
  const running = ref(false)

  // 标记同步到关闭协商：关掉 watch 立即同步一次，避免「先置位再被覆盖」
  watch(
    dirty,
    (value) => {
      handle.setDirty(value)
    },
    { immediate: true }
  )
  watch(
    running,
    (value) => {
      handle.setRunning(value)
    },
    { immediate: true }
  )

  if (spec.prepare) handle.onPrepare(spec.prepare)
  if (spec.dispose) handle.onDispose(spec.dispose)

  onUnmounted(() => {
    handle.unregister()
  })

  return { dirty, running, scope, visibility, onResume }
}
