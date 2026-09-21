/**
 * 应用级错误收集（可靠性 T11-4）
 *
 * 解决的问题：错误原来只以 toast 形式闪一下，用户看不到也说不清；控制台又不在用户手里。
 * 这里把「用户可见的错误」留档成一份有界清单，供诊断页面与「复制诊断信息」使用。
 *
 * 边界（刻意克制）：
 * - 只收**结构化**摘要：稳定 code、面向用户的 message、来源、次数、首次/最近时间；
 * - message 截断到 [`MAX_MESSAGE_LEN`]，堆栈只留前 [`MAX_DETAIL_LEN`] 字符；
 * - 条数上限 [`MAX_ERROR_ENTRIES`]，超出丢最旧的；同一 code+message 合并计数；
 * - 错误详情只保存在本进程；全局异常另写固定安全摘要到本地应用日志，不发送任意异常正文。
 */
import type { App } from 'vue'
import { isTauri } from '@tauri-apps/api/core'
import { error as logError } from '@tauri-apps/plugin-log'
import { recordError } from './errors-core'

export {
  MAX_ERROR_ENTRIES,
  MAX_MESSAGE_LEN,
  MAX_DETAIL_LEN,
  listErrors,
  clearErrors,
  subscribeErrors,
  recordError,
  normalizeUnknownError,
  type AppErrorEntry,
} from './errors-core'

/**
 * 装上全局错误钩子：Vue 渲染/生命周期异常、未处理的 Promise 拒绝、全局脚本错误。
 *
 * 装完之后这些错误不再只躺在控制台：诊断页面能看到，用户复制诊断信息时带得走。
 */
export function installErrorCollectors(app: App): void {
  const previous = app.config.errorHandler
  app.config.errorHandler = (error: unknown, instance, info: string) => {
    recordError({
      code: 'ui.render_failed',
      message: error instanceof Error ? error.message : String(error),
      source: `vue:${info}`,
      detail: error instanceof Error ? (error.stack ?? '') : '',
    })
    if (isTauri()) {
      void logError('前端异常 code=ui.render_failed').catch(() => {
        console.warn('[diagnostics] 日志发送失败')
      })
    }
    // 保留既有处理（框架其它地方可能已注册）
    previous?.(error, instance, info)
  }

  window.addEventListener('unhandledrejection', (event) => {
    if (isTauri()) {
      void logError('前端异常 code=ui.unhandled_rejection').catch(() => {
        console.warn('[diagnostics] 日志发送失败')
      })
    }
    const reason = event.reason
    recordError({
      code: 'ui.unhandled_rejection',
      message: reason instanceof Error ? reason.message : String(reason),
      source: 'window',
      detail: reason instanceof Error ? (reason.stack ?? '') : '',
    })
  })

  window.addEventListener('error', (event) => {
    if (isTauri()) {
      void logError('前端异常 code=ui.uncaught_error').catch(() => {
        console.warn('[diagnostics] 日志发送失败')
      })
    }
    // 资源加载失败也会走这里：event.error 为空时用 message 兜底
    const error = event.error as unknown
    recordError({
      code: 'ui.uncaught_error',
      message: error instanceof Error ? error.message : event.message || '未知脚本错误',
      source: 'window',
      detail: error instanceof Error ? (error.stack ?? '') : '',
    })
  })
}
