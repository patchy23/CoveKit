/**
 * SqlEditor 扩展组装（CodeMirror 6）
 * 参考 dbx：基础编辑能力（行号/撤销/折叠/括号匹配/查找/注释切换）、
 * 当前语句框选（光标所在完整语句高亮）、snippet 补全、表名点列名异步补全。
 */
import type { Extension } from '@codemirror/state'
import { StateField } from '@codemirror/state'
import { Decoration, EditorView, keymap, type DecorationSet } from '@codemirror/view'
import { defaultKeymap, history, historyKeymap, indentWithTab, toggleComment } from '@codemirror/commands'
import { bracketMatching, foldGutter, foldKeymap } from '@codemirror/language'
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
import { statementRangeAtCursor } from './sqlStatementRanges'

/** 常用 SQL 模板（snippet 占位符 Tab 跳转） */
const SQL_SNIPPETS = [
  snippetCompletion('SELECT * FROM ${table}', { label: 'SELECT * FROM', type: 'keyword', detail: '模板' }),
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
  snippetCompletion('DELETE FROM ${table} WHERE ${condition}', { label: 'DELETE FROM', type: 'keyword', detail: '模板' }),
  snippetCompletion('CREATE TABLE ${table} (\n  ${id} ${type} PRIMARY KEY,\n  ${column} ${type2}\n)', {
    label: 'CREATE TABLE',
    type: 'keyword',
    detail: '模板',
  }),
]

/** 基础编辑扩展（对齐 dbx 编辑器基础能力） */
export function sqlEditorBasics(): Extension[] {
  return [
    // 撤销/重做历史
    history(),
    // 括号匹配 + 折叠
    bracketMatching(),
    foldGutter(),
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

/** 当前语句框选：光标所在完整语句加边框高亮（对齐 dbx 的 currentStatementFrame） */
const currentStatementField = StateField.define<DecorationSet>({
  create() {
    return Decoration.none
  },
  update(deco, tr) {
    // 文档或选区变化时重算光标所在语句范围
    if (!tr.docChanged && !tr.selection) return deco
    const head = tr.state.selection.main.head
    const target = statementRangeAtCursor(tr.state.doc.toString(), head)
    if (!target) return Decoration.none
    const mark = Decoration.mark({ class: 'cm-current-statement' })
    return Decoration.set([mark.range(target.from, target.to)])
  },
  provide: (field) => EditorView.decorations.from(field),
})

/** 当前语句框选扩展 */
export function currentStatementExtension(): Extension {
  return currentStatementField
}
