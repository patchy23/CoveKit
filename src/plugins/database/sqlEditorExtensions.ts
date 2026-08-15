/**
 * SqlEditor 扩展组装（CodeMirror 6）
 * 参考 dbx：基础编辑能力（行号/撤销/折叠/括号匹配/查找/注释切换）、
 * 当前语句框选（光标所在完整语句高亮）、snippet 补全、表名点列名异步补全。
 */
import type { Extension } from '@codemirror/state'
import { Range, RangeSet } from '@codemirror/state'
import { GutterMarker, gutter, keymap } from '@codemirror/view'
import {
  defaultKeymap,
  history,
  historyKeymap,
  indentWithTab,
  toggleComment,
} from '@codemirror/commands'
import {
  HighlightStyle,
  bracketMatching,
  foldGutter,
  foldKeymap,
  syntaxHighlighting,
} from '@codemirror/language'
import { lineNumbers } from '@codemirror/view'
import { tags } from '@lezer/highlight'
import { highlightSelectionMatches, search, searchKeymap } from '@codemirror/search'
import {
  autocompletion,
  closeBrackets,
  closeBracketsKeymap,
  snippetCompletion,
  type CompletionContext,
  type CompletionResult,
} from '@codemirror/autocomplete'
import {
  StandardSQL,
  keywordCompletionSource,
  schemaCompletionSource,
  type SQLDialect,
  type SQLNamespace,
} from '@codemirror/lang-sql'
import {
  splitSqlStatements,
  statementExecutableSql,
  statementStartOffset,
} from './sqlStatementRanges'
/** 常用 SQL 模板（snippet 占位符 Tab 跳转） */
const SQL_SNIPPETS = [
  snippetCompletion('SELECT * FROM ${table}', {
    label: 'SELECT * FROM',
    type: 'keyword',
    detail: '模板',
  }),
  snippetCompletion('SELECT ${columns} FROM ${table} WHERE ${condition}', {
    label: 'SELECT … FROM … WHERE',
    type: 'keyword',
    detail: '模板',
  }),
  snippetCompletion('INSERT INTO ${table} (${columns}) VALUES (${values})', {
    label: 'INSERT INTO',
    type: 'keyword',
    detail: '模板',
  }),
  snippetCompletion('UPDATE ${table} SET ${column} = ${value} WHERE ${condition}', {
    label: 'UPDATE … SET',
    type: 'keyword',
    detail: '模板',
  }),
  snippetCompletion('DELETE FROM ${table} WHERE ${condition}', {
    label: 'DELETE FROM',
    type: 'keyword',
    detail: '模板',
  }),
  snippetCompletion(
    'CREATE TABLE ${table} (\n  ${id} ${type} PRIMARY KEY,\n  ${column} ${type2}\n)',
    {
      label: 'CREATE TABLE',
      type: 'keyword',
      detail: '模板',
    }
  ),
]

/** SQL 语法高亮（CSS 变量 → 深浅色自动跟随；对齐 dbx 配色语义） */
const sqlHighlightStyle = HighlightStyle.define([
  {
    tag: [
      tags.keyword,
      tags.controlKeyword,
      tags.definitionKeyword,
      tags.operatorKeyword,
      tags.modifier,
      tags.bool,
      tags.null,
    ],
    color: 'var(--color-tertiary-strong)',
  },
  { tag: [tags.string, tags.special(tags.string)], color: 'var(--color-success-strong)' },
  { tag: [tags.number, tags.integer, tags.float], color: 'var(--color-info-strong)' },
  {
    tag: [tags.comment, tags.lineComment, tags.blockComment],
    color: 'var(--color-text-muted)',
    fontStyle: 'italic',
  },
  { tag: tags.typeName, color: 'var(--color-tertiary-strong)' },
  { tag: tags.variableName, color: 'var(--color-primary)' },
  { tag: tags.function(tags.variableName), color: 'var(--color-info-strong)' },
  {
    tag: [tags.operator, tags.compareOperator, tags.logicOperator, tags.arithmeticOperator],
    color: 'var(--color-secondary)',
  },
  {
    tag: [tags.punctuation, tags.paren, tags.brace, tags.bracket],
    color: 'var(--color-secondary)',
  },
  { tag: tags.invalid, color: 'var(--color-danger-strong)', textDecoration: 'underline' },
])

/** 折叠 gutter 标记：chevron 图标（展开=向下，收起=向右），替代默认文本符号 */
function foldMarkerDom(open: boolean): HTMLElement {
  const el = document.createElement('span')
  el.className = `cm-fold-marker${open ? ' cm-fold-open' : ''}`
  el.title = open ? '折叠' : '展开'
  el.innerHTML = open
    ? '<svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m6 9 6 6 6-6"/></svg>'
    : '<svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m9 18 6-6-6-6"/></svg>'
  return el
}

/** 基础编辑扩展（对齐 dbx 编辑器基础能力） */
export function sqlEditorBasics(): Extension[] {
  return [
    // SQL 语法高亮（深浅色随主题）
    syntaxHighlighting(sqlHighlightStyle),
    // 行号
    lineNumbers(),
    // 撤销/重做历史
    history(),
    // 括号匹配 + 折叠（chevron 标记，样式走主题）
    bracketMatching(),
    foldGutter({ markerDOM: foldMarkerDom }),
    // 查找替换（Mod-f）+ 选中词高亮
    search({ top: true }),
    highlightSelectionMatches(),
    // 自动闭合括号/引号
    closeBrackets(),
    // 按键绑定：默认 + 历史 + 折叠 + 查找 + 括号 + Tab 缩进 + 行注释（Mod-/）
    keymap.of([
      ...closeBracketsKeymap,
      ...defaultKeymap,
      ...historyKeymap,
      ...foldKeymap,
      ...searchKeymap,
      indentWithTab,
      { key: 'Mod-/', run: toggleComment },
    ]),
  ]
}

/** 补全扩展：关键字 + schema（表/列）+ snippet 模板 + 表.列异步补全 */
export function sqlCompletionExtension(
  dialect: SQLDialect | undefined,
  schema: SQLNamespace | undefined,
  resolveColumns?: (table: string) => Promise<string[]>
): Extension {
  // snippet 模板（输入关键字前缀时列出）
  const snippetSource = (ctx: CompletionContext): CompletionResult | null => {
    const before = ctx.matchBefore(/^\w*$/)
    if (!before) return null
    const word = before.text.toLowerCase()
    const options = SQL_SNIPPETS.filter((s) => s.label.toLowerCase().startsWith(word))
    return options.length ? { from: before.from, options, validFor: /^\w*$/ } : null
  }
  // 表名. 后异步补列名（未缓存时向后端要）
  const columnSource = async (ctx: CompletionContext): Promise<CompletionResult | null> => {
    if (!resolveColumns) return null
    const match = ctx.matchBefore(/([A-Za-z_][\w$]*)\s*\.\s*$/)
    if (!match) return null
    const table = match.text.match(/([A-Za-z_][\w$]*)\s*\.\s*$/)
    if (!table) return null
    const columns = await resolveColumns(table[1])
    if (!columns.length) return null
    return {
      from: match.from + table[1].length + 1,
      options: columns.map((name) => ({ label: name, type: 'variable', detail: table[1] })),
      validFor: /^\w*$/,
    }
  }
  return autocompletion({
    override: [
      snippetSource,
      columnSource,
      keywordCompletionSource(dialect ?? StandardSQL),
      schemaCompletionSource({ dialect, schema }),
    ],
  })
}
/** 语句执行按钮：每条语句起始行前显示 ▶（对齐 dbx runStatementGutter） */
class RunStatementMarker extends GutterMarker {
  eq(other: GutterMarker): boolean {
    return other instanceof RunStatementMarker
  }

  toDOM() {
    const btn = document.createElement('button')
    btn.type = 'button'
    btn.className = 'cm-run-statement-btn'
    btn.setAttribute('aria-label', '执行此语句')
    btn.innerHTML =
      '<svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M5 5a2 2 0 0 1 3.008-1.728l11.997 6.998a2 2 0 0 1 .003 3.458l-12 7A2 2 0 0 1 5 19z"></path></svg>'
    return btn
  }
}

/** 每条语句的执行按钮 gutter；点击执行该条语句（onRun 回调由上层接） */
export function statementRunGutterExtension(onRun?: (sql: string) => void): Extension {
  return gutter({
    class: 'cm-run-statement-gutter',
    markers: (view) => {
      const doc = view.state.doc.toString()
      const ranges = splitSqlStatements(doc)
      const entries: Range<GutterMarker>[] = []
      for (const range of ranges) {
        if (!range.sql.trim()) continue
        // 定位到语句首个非空白字符所在行（否则上一条语句分号后的换行会把按钮错位到上一行）
        const line = view.state.doc.lineAt(statementStartOffset(range))
        entries.push(new RunStatementMarker().range(line.from))
      }
      return RangeSet.of(entries, true)
    },
    domEventHandlers: {
      mousedown: (view, line, event) => {
        if (!(event instanceof MouseEvent) || event.button !== 0) return false
        const doc = view.state.doc.toString()
        // 找到起始行与点击行一致的语句（与 markers 的定位逻辑保持一致）
        const range = splitSqlStatements(doc).find(
          (r) => r.sql.trim() && view.state.doc.lineAt(statementStartOffset(r)).from === line.from
        )
        if (!range) return false
        event.preventDefault()
        onRun?.(statementExecutableSql(range))
        view.focus()
        return true
      },
    },
  })
}
