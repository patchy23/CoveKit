/**
 * 诊断内核的行为测试（T11-4/T11-5/T11-6）
 *
 * 关键口径：清单是有界的、同一条错误合并计数、说明与堆栈都被截断；
 * 诊断文本只带白名单字段且路径被脱敏，不做任何上传。
 */
import { beforeEach, describe, expect, it } from 'vitest'
import {
  MAX_DETAIL_LEN,
  MAX_ERROR_ENTRIES,
  MAX_MESSAGE_LEN,
  clearErrors,
  listErrors,
  normalizeUnknownError,
  recordError,
  subscribeErrors,
} from '@/core/diagnostics/errors-core'
import { formatDiagnostics, redactPaths, type DiagnosticsReport } from '@/core/diagnostics/report'

describe('错误清单', () => {
  beforeEach(() => {
    clearErrors()
  })

  it('条数有上限：超出丢弃最旧的', () => {
    for (let index = 0; index < MAX_ERROR_ENTRIES + 10; index += 1) {
      recordError({ code: `code.${index}`, message: `第 ${index} 条`, source: 'test' })
    }

    expect(listErrors()).toHaveLength(MAX_ERROR_ENTRIES)
    expect(listErrors()[0]?.code).toBe(`code.${MAX_ERROR_ENTRIES + 9}`)
  })

  it('同码同说明合并计数并更新时间', () => {
    recordError({ code: 'update.check_failed', message: '网络不可达', source: 'update' })
    recordError({ code: 'update.check_failed', message: '网络不可达', source: 'update' })

    expect(listErrors()).toHaveLength(1)
    expect(listErrors()[0]?.count).toBe(2)
    expect(listErrors()[0]?.lastAt).toBeGreaterThanOrEqual(listErrors()[0]?.firstAt ?? 0)
  })

  it('说明与堆栈按上限截断', () => {
    recordError({
      code: 'ui.render_failed',
      message: 'x'.repeat(MAX_MESSAGE_LEN + 50),
      source: 'vue:render',
      detail: 'y'.repeat(MAX_DETAIL_LEN + 50),
    })

    const entry = listErrors()[0]
    expect(entry?.message.length).toBe(MAX_MESSAGE_LEN + 1)
    expect(entry?.detail.length).toBe(MAX_DETAIL_LEN + 1)
  })

  it('订阅者立刻拿到当前清单，退订后不再收到', () => {
    const seen: number[] = []
    const stop = subscribeErrors((entries) => seen.push(entries.length))
    recordError({ code: 'x', message: 'y', source: 'test' })
    stop()
    recordError({ code: 'x2', message: 'y2', source: 'test' })

    expect(seen).toEqual([0, 1])
  })

  it('非 Error 对象按带 code 的结构识别', () => {
    expect(
      normalizeUnknownError({ code: 'task.not_cancellable', message: '不可取消' }, 'fallback')
    ).toEqual({
      code: 'task.not_cancellable',
      message: '不可取消',
      detail: '',
    })
    expect(normalizeUnknownError('boom', 'fallback').code).toBe('fallback')
  })
})

describe('诊断文本', () => {
  const report: DiagnosticsReport = {
    appVersion: '0.1.0',
    tauriVersion: '2.0.0',
    platform: 'Win32',
    storageIsDefault: true,
    storagePartitions: 4,
    nativeProtection: false,
    protectionDomains: ['credentials：file-fallback（available）'],
    activeTasks: 1,
    recentTasks: ['storage.migrate(t1) running 40%'],
    errorCodes: ['update.check_failed × 2'],
    recentErrors: ['[update.check_failed] 网络不可达'],
  }

  it('路径脱敏覆盖 Windows 与 Unix 形式', () => {
    expect(redactPaths('C:\\Users\\patchy\\AppData\\Roaming\\x')).toBe('<path>')
    expect(redactPaths('/home/patchy/secret.txt')).toBe('<path>')
    expect(redactPaths('备份在 G:\\workspace\\back 下')).toContain('<path>')
  })

  it('渲染文本只包含白名单字段并声明未上传', () => {
    const text = formatDiagnostics({
      ...report,
      recentErrors: ['[x] 失败于 C:\\Users\\patchy\\a.db'],
    })

    expect(text).toContain('CoveKit 诊断信息（本地生成，未上传）')
    expect(text).toContain('应用版本：0.1.0')
    expect(text).toContain('活跃任务数：1')
    expect(text).toContain('错误码：update.check_failed × 2')
    expect(text).not.toContain('C:\\Users\\patchy\\a.db')
    expect(text).toContain('<path>')
  })
})
