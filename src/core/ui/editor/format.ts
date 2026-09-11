/**
 * 编辑器格式化入口
 *
 * 按语言分派到 `@/core/format` 的纯函数（与格式化工具共用同一实现，避免两套逻辑漂移）：
 * - JSON / JSONC：`formatJson`（解析失败给出行列）
 * - XML / SVG / plist：`formatXml`（严格解析失败时其内部有宽松回退）
 * - SQL：`formatSql`（自研词法格式化，无第三方依赖）
 * 不支持的语言返回 `ok: false` 并带中文原因，由上层 toast 提示，禁止静默失败。
 */
import { formatJson } from '@/core/format/json'
import { formatSql } from '@/core/format/sql'
import { formatXml } from '@/core/format/xml'

/** 格式化结果（统一形状，供组件 emit('error') 与 toast 使用） */
export interface EditorFormatResult {
  /** 是否成功 */
  ok: boolean
  /** 格式化后的文本（失败时为原文） */
  output: string
  /** 失败原因（中文） */
  error?: string
}

/** 支持格式化的语言 id */
const JSON_LANGUAGES = new Set(['json', 'jsonc'])
const XML_LANGUAGES = new Set(['xml', 'svg', 'plist'])

/** 该语言是否支持一键格式化 */
export function canFormat(languageId: string): boolean {
  return JSON_LANGUAGES.has(languageId) || XML_LANGUAGES.has(languageId) || languageId === 'sql'
}

/** 格式化文本；失败时原样返回输入并给出中文原因 */
export function formatDocument(text: string, languageId: string, indent = 2): EditorFormatResult {
  if (!text.trim()) return { ok: false, output: text, error: '内容为空，无法格式化' }

  if (JSON_LANGUAGES.has(languageId)) {
    const result = formatJson(text, indent)
    if (result.ok) return { ok: true, output: result.output }
    const location = result.error
      ? `第 ${result.error.line} 行第 ${result.error.col} 列`
      : '位置未知'
    return { ok: false, output: text, error: `JSON 语法错误（${location}）` }
  }

  if (XML_LANGUAGES.has(languageId)) {
    const result = formatXml(text, indent)
    if (result.ok) return { ok: true, output: result.output ?? text }
    return { ok: false, output: text, error: result.error ?? 'XML 格式化失败' }
  }

  if (languageId === 'sql') {
    try {
      return { ok: true, output: formatSql(text) }
    } catch (error) {
      return {
        ok: false,
        output: text,
        error: `SQL 格式化失败：${error instanceof Error ? error.message : String(error)}`,
      }
    }
  }

  return { ok: false, output: text, error: '当前语言不支持格式化' }
}
