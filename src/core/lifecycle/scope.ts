/**
 * 框架 · 作用域化副作用（可靠性 T10-5）
 *
 * 解决三类真实的泄漏与竞态：
 * 1. **卸载早于 `listen` resolve**：组件已卸载但订阅随后才建立，回调再也不会被解绑；
 *    这里在 resolve 后发现已 dispose 就立即 unlisten。
 * 2. **部分成功**：一次建多个订阅，第二个失败时第一个仍在挂着；失败路径也会清理已建立的部分。
 * 3. **dispose 之后又创建定时器**：异步流程晚一步回来还会 setInterval，造成「关不掉的轮询」。
 *
 * 用法：组件在 `setup` 里 `const scope = createScope('frp')`，
 * `onUnmounted(() => void scope.dispose())`；所有订阅与定时器都从 scope 建。
 *
 * 另附活体计数（`scopeStats`）：页签快速开关后订阅与定时器必须回到基线，
 * 开发走查读取全局计数；可选资源监测读取按工具聚合的数字，不持有资源引用。
 */
import { listen } from '@tauri-apps/api/event'
import type { CloseIssue } from './types'
import { countScopeResource } from '@/core/resourceMonitor/metrics'

/** dispose 结果：失败与超时都不抛给调用方，而是回报给调用方决定是否提示 */
export interface ScopeDisposeResult {
  /** 作用域名（日志与失败定位） */
  name: string
  /** 清理失败（含订阅建立失败、dispose 回调抛错） */
  failures: CloseIssue[]
}

/**
 * 活体统计快照（诊断用，不做业务判断）。
 *
 * `created` 只增不减，用来确认循环确实跑过；其余三项在页签开关后必须回到基线。
 */
export interface ScopeStats {
  /** 累计创建的作用域数 */
  created: number
  /** 尚未释放的作用域数 */
  live: number
  /** 尚未解绑的 IPC 事件订阅数（不含 DOM 监听，后者随 dispose 回调一并移除） */
  listeners: number
  /** 尚未清除的定时器数（含一次性定时器） */
  timers: number
}

let createdScopes = 0
let liveScopes = 0
let liveListeners = 0
let liveTimers = 0

/** 读取活体统计快照；开发构建的走查钩子读它，生产代码不依赖返回值做业务判断 */
export function scopeStats(): ScopeStats {
  return { created: createdScopes, live: liveScopes, listeners: liveListeners, timers: liveTimers }
}

/** 作用域句柄 */
export interface Scope {
  /** 作用域名（诊断） */
  readonly name: string
  /** 是否已 dispose（异步流程回来前必须检查） */
  readonly disposed: boolean
  /** 注册一个清理回调（按注册顺序逆序执行；重复注册同一个函数会执行两次，调用方自行去重） */
  addDispose(fn: () => void | Promise<void>): void
  /** 建立一条事件订阅；resolve 时若已 dispose 会立即解绑 */
  listenEvent<T>(event: string, handler: (payload: T) => void): Promise<void>
  /** 建定时器；已 dispose 时不再创建（返回 false 表示被拒绝创建） */
  interval(fn: () => void, ms: number): boolean
  /** 建一次性定时器；已 dispose 时不再创建 */
  timeout(fn: () => void, ms: number): boolean
  /** 页面/窗口重新可见时的回调（隐藏期间被跳过的刷新在此补齐） */
  onResume(fn: () => void): void
  /** 释放：先清定时器与订阅，再逆序执行清理回调；可重复调用 */
  dispose(): Promise<ScopeDisposeResult>
}

/** 清理回调执行形态（允许 async，统一 await） */
type DisposeCallback = () => void | Promise<void>

/**
 * 创建作用域。
 *
 * @param name 作用域名（建议用工具 id 或「工具 id.子模块」，失败信息里直接可见）
 * @param owner 工具标识；默认取作用域名前缀，公共生命周期入口显式传入 toolId。
 */
export function createScope(name: string, owner = name.split('.')[0]): Scope {
  createdScopes += 1
  liveScopes += 1
  countScopeResource(owner, 'scopes', 1)
  let disposed = false
  const unlisteners = new Set<() => void>()
  const timers = new Set<ReturnType<typeof setTimeout>>()
  const intervals = new Set<ReturnType<typeof setInterval>>()
  const callbacks: DisposeCallback[] = []
  const resumeHandlers: Array<() => void> = []
  const failures: CloseIssue[] = []

  const clearTimers = () => {
    for (const handle of timers) clearTimeout(handle)
    for (const handle of intervals) clearInterval(handle)
    liveTimers -= timers.size + intervals.size
    countScopeResource(owner, 'timers', -timers.size - intervals.size)
    timers.clear()
    intervals.clear()
  }

  const scope: Scope = {
    name,
    get disposed() {
      return disposed
    },
    addDispose(fn) {
      if (disposed) {
        // 已释放后注册的清理回调没有意义：立刻执行一次，保持「注册即生效」的直觉
        void Promise.resolve(fn()).catch((error) => {
          failures.push({ owner: name, message: describe(error) })
        })
        return
      }
      callbacks.push(fn)
    },
    async listenEvent<T>(event: string, handler: (payload: T) => void) {
      if (disposed) {
        // 已释放：不建立订阅（否则会得到一条没人解绑的监听）
        failures.push({ owner: name, message: `作用域已释放，跳过订阅 ${event}` })
        return
      }
      try {
        const unlisten = await listen<T>(event, (payload) => handler(payload.payload))
        if (disposed) {
          // 卸载早于 resolve：立刻解绑，不能留下来
          unlisten()
          return
        }
        unlisteners.add(unlisten)
        liveListeners += 1
        countScopeResource(owner, 'listeners', 1)
      } catch (error) {
        failures.push({ owner: name, message: `订阅 ${event} 失败：${describe(error)}` })
      }
    },
    interval(fn, ms) {
      if (disposed) {
        failures.push({ owner: name, message: '作用域已释放，拒绝创建定时器' })
        return false
      }
      const handle = setInterval(fn, ms)
      intervals.add(handle)
      liveTimers += 1
      countScopeResource(owner, 'timers', 1)
      return true
    },
    timeout(fn, ms) {
      if (disposed) {
        failures.push({ owner: name, message: '作用域已释放，拒绝创建定时器' })
        return false
      }
      const handle = setTimeout(() => {
        timers.delete(handle)
        liveTimers -= 1
        countScopeResource(owner, 'timers', -1)
        fn()
      }, ms)
      timers.add(handle)
      liveTimers += 1
      countScopeResource(owner, 'timers', 1)
      return true
    },
    onResume(fn) {
      if (disposed) return
      resumeHandlers.push(fn)
    },
    async dispose() {
      if (disposed) return { name, failures: [...failures] }
      disposed = true
      liveScopes -= 1
      countScopeResource(owner, 'scopes', -1)
      clearTimers()
      let unbound = 0
      for (const unlisten of unlisteners) {
        try {
          unlisten()
          unbound += 1
        } catch (error) {
          // 解绑抛错时不减计数：无法证明该订阅已断，失败本身进 failures 交由调用方提示
          failures.push({ owner: name, message: `解绑订阅失败：${describe(error)}` })
        }
      }
      liveListeners -= unbound
      countScopeResource(owner, 'listeners', -unbound)
      unlisteners.clear()
      // 逆序执行：后建立依赖的资源先释放（订阅→派生的轮询→状态）
      while (callbacks.length > 0) {
        const callback = callbacks.pop()
        if (!callback) continue
        try {
          await callback()
        } catch (error) {
          failures.push({ owner: name, message: describe(error) })
        }
      }
      return { name, failures: [...failures] }
    },
  }

  // 可见性恢复：隐藏期间被节流跳过的刷新在这里补齐（隐藏不暂停协议心跳，见 T10-7）
  if (typeof document !== 'undefined') {
    const onVisibility = () => {
      if (document.visibilityState === 'visible' && !disposed) {
        for (const handler of resumeHandlers) {
          try {
            handler()
          } catch (error) {
            failures.push({ owner: name, message: describe(error) })
          }
        }
      }
    }
    document.addEventListener('visibilitychange', onVisibility)
    callbacks.push(() => document.removeEventListener('visibilitychange', onVisibility))
  }

  return scope
}

/** 错误描述归一化（Error 取 message，其余 String 化） */
function describe(error: unknown): string {
  return error instanceof Error ? error.message : String(error)
}

/**
 * 隐藏时降频的间隔计算（T10-7）。
 *
 * 只降低「可视更新」频率，不改变协议心跳与下载：调用方把该间隔用于状态轮询即可。
 *
 * @param visibleMs 可见时的间隔
 * @param hiddenMs 隐藏时的间隔
 * @param hidden 当前是否处于隐藏（窗口不可见或页签非激活）
 */
export function throttledInterval(visibleMs: number, hiddenMs: number, hidden: boolean): number {
  const value = hidden ? hiddenMs : visibleMs
  return Number.isFinite(value) && value > 0 ? value : visibleMs
}
