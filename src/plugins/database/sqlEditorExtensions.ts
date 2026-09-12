/**
 * SQL 编辑器扩展：补全源与语句运行按钮（数据库插件的领域能力）
 *
 * 说明：基础编辑能力（行号 / 历史 / 括号匹配 / 折叠 / 查找 / 缩进 / 注释切换）与语法高亮
 * 已由 core 的 `UiCodeEditor` 内置（统一主题走 `--cm-*` 变量），本文件只保留 SQL 专有部分：
 * - `sqlCompletionSources`：snippet 模板、表名点列名的异步补全、关键字、schema 补全；
 * - `statementRunGutterExtension`：每条语句起始行的 ▶ 执行按钮。
 * 两者分别通过 UiCodeEditor 的 `completionSources` 与 `extraExtensions` 装载。
 */
import type { Extension } from '@codemirror/state'
import { Range, RangeSet } from '@codemirror/state'
import { EditorView, GutterMarker, gutter } from '@codemirror/view'
import {
  snippetCompletion,
  type CompletionContext,
  type CompletionResult,
  type CompletionSource,
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

/**
 * SQL 补全源（按优先级：snippet 模板 → 表.列异步 → 关键字 → schema 表列）
 *
 * @param dialect 方言（MySQL / PostgreSQL / SQLite），未指定回退标准 SQL
 * @param schema 表名 → 列名映射（由当前连接的库结构生成）
 * @param resolveColumns 未缓存表的列名异步获取（表名. 之后触发）
 */
export function sqlCompletionSources(
  dialect: SQLDialect | undefined,
  schema: SQLNamespace | undefined,
  resolveColumns?: (table: string) => Promise<string[]>
): CompletionSource[] {
  /** snippet 模板：输入关键字前缀时列出 */
  const snippetSource = (ctx: CompletionContext): CompletionResult | null => {
    const before = ctx.matchBefore(/^\w*$/)
    if (!before) return null
    const word = before.text.toLowerCase()
    const options = SQL_SNIPPETS.filter((s) => s.label.toLowerCase().startsWith(word))
    return options.length ? { from: before.from, options, validFor: /^\w*$/ } : null
  }

  /** 表名. 后异步补列名（未缓存时向后端要） */
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

  return [
    snippetSource,
    columnSource,
    keywordCompletionSource(dialect ?? StandardSQL),
    schemaCompletionSource({ dialect, schema }),
  ]
}

/** 语句执行按钮的 gutter 标记（▶ 图标） */
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

/** 每条语句起始行显示 ▶（样式随扩展自带）；点击执行该条语句（onRun 由上层接入） */
export function statementRunGutterExtension(onRun?: (sql: string) => void): Extension {
  const gutterAndStyle = gutter({
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

  // 按钮外观随扩展自带：配色走 token，深浅主题自动跟随（不需暗色单独覆盖）
  const style = EditorView.theme({
    '.cm-run-statement-gutter': { minWidth: '24px' },
    '.cm-run-statement-gutter .cm-gutterElement': {
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'center',
      minWidth: '24px',
      padding: '0 2px',
    },
    '.cm-run-statement-btn': {
      display: 'inline-flex',
      alignItems: 'center',
      justifyContent: 'center',
      width: '20px',
      height: '20px',
      margin: '0',
      padding: '0',
      border: 'none',
      borderRadius: '4px',
      background: 'transparent',
      cursor: 'pointer',
      opacity: '0',
      color: 'var(--color-success-strong)',
      transition: 'opacity 0.12s, background-color 0.12s',
    },
    '&.cm-editor:hover .cm-run-statement-btn': { opacity: '0.55' },
    '&.cm-editor:hover .cm-run-statement-btn:hover': {
      opacity: '1',
      background: 'var(--color-success-soft)',
    },
  })

  return [gutterAndStyle, style]
}

/**
 * Ctrl(⌘)+点击表名跳转：命中当前连接已知的表名时回调上层打开表结构
 *
 * 悬停时光标变手型提示可点击；命中判定同时兼容 `表名`、`"表名"` 这类引号包裹写法。
 */
export function tableNavigationExtension(
  tables: () => { name: string }[],
  onTableClick: (table: string) => void
): Extension {
  /** 点击位置命中的已知表名（未命中返回空串） */
  function knownTableAt(view: EditorView, event: MouseEvent): string {
    const position = view.posAtCoords({ x: event.clientX, y: event.clientY })
    if (position == null) return ''
    const word = view.state.wordAt(position)
    if (!word) return ''
    let { from, to } = word
    const doc = view.state.doc
    const before = from > 0 ? doc.sliceString(from - 1, from) : ''
    const after = to < doc.length ? doc.sliceString(to, to + 1) : ''
    // 表名被引号包裹时把引号一起纳入，避免 A` 之类的边界截断
    if ((before === '`' || before === '"') && after === before) {
      from -= 1
      to += 1
    }
    const raw = doc.sliceString(from, to).replace(/^["'`]|["'`]$/g, '')
    return tables().find((table) => table.name.toLowerCase() === raw.toLowerCase())?.name ?? ''
  }

  return EditorView.domEventHandlers({
    mousedown: (event, view) => {
      if (!(event.ctrlKey || event.metaKey) || event.button !== 0) return false
      const table = knownTableAt(view, event)
      if (!table) return false
      event.preventDefault()
      onTableClick(table)
      return true
    },
    mousemove: (event, view) => {
      const pointer = (event.ctrlKey || event.metaKey) && knownTableAt(view, event) ? 'pointer' : ''
      if (view.dom.style.cursor !== pointer) view.dom.style.cursor = pointer
    },
  })
}
