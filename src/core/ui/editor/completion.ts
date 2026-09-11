/**
 * 补全：注入式补全源（数据库表名 / 列名等）优先，文档词法兜底其次
 *
 * 词法兜底从当前文档抽取标识符作为候选，不做语法分析——低成本、无额外体积，
 * 适合 JSON / XML / 配置类文件；SQL 等场景由调用方通过 `sources` 注入更精确的补全源，
 * 注入源排在前面，命中时优先生效。
 */
import { autocompletion, type Completion, type CompletionSource } from '@codemirror/autocomplete'
import type { Extension } from '@codemirror/state'

export type { CompletionSource }

/** 补全装配选项 */
export interface CompletionSetup {
  /** 注入的补全源（优先级高于词法兜底） */
  sources?: CompletionSource[]
  /** 是否启用文档词法兜底（默认启用；超大文档建议关闭） */
  lexical?: boolean
  /** 输入时自动弹出候选（默认启用） */
  activateOnTyping?: boolean
}

/** 词法兜底：抽取文档中的标识符作为候选（去重 + 上限保护） */
export function lexicalCompletionSource(maxWords = 1500): CompletionSource {
  return (context) => {
    const word = context.matchBefore(/[\w$.-]+/)
    if (!word) return null
    if (word.from === word.to && !context.explicit) return null

    const doc = context.state.doc.toString()
    const seen = new Set<string>()
    const pattern = /[A-Za-z_$][\w$]{2,}/g
    let match = pattern.exec(doc)
    while (match) {
      seen.add(match[0])
      if (seen.size >= maxWords) break
      match = pattern.exec(doc)
    }

    const current = doc.slice(word.from, word.to)
    const options: Completion[] = []
    for (const label of seen) {
      if (label === current) continue
      options.push({ label, type: 'text' })
    }
    if (options.length === 0) return null
    return { from: word.from, options, validFor: /^[\w$.-]*$/ }
  }
}

/** 组装补全扩展 */
export function buildCompletion(setup: CompletionSetup = {}): Extension {
  const sources: CompletionSource[] = [...(setup.sources ?? [])]
  if (setup.lexical !== false) sources.push(lexicalCompletionSource())
  if (sources.length === 0) return []
  return autocompletion({
    override: sources,
    activateOnTyping: setup.activateOnTyping !== false,
    closeOnBlur: true,
    icons: false,
    maxRenderedOptions: 40,
  })
}
