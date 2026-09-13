/**
 * 编辑器统一契约测试
 *
 * 规则：工具层（plugins / features）只能通过 `@/core/ui` 的 UiCodeEditor / UiCodeDiff 使用编辑器，
 * 不得自行实例化编辑器（CodeMirror / Monaco / Ace）、不得引入其它编辑器基座、不得直接引用编辑器内部实现。
 *
 * 为什么固化成测试：编辑器基座可能整体替换（选型与替换成本见
 * `docs/batches/editor-202609-001-公共编辑器组件/编辑器基座对比-CodeMirror6与Monaco.md`），
 * 只有把「谁可以碰基座 API」钉死在契约里，替换面才会收敛在 core/ui 内部。
 *
 * 合法例外：插件可为共享编辑器**注入扩展**（例如数据库插件的 SQL 补全与语句运行 gutter，
 * 见 `plugins/database/sqlEditorExtensions.ts`），因此这里禁的是「实例化编辑器」，
 * 而不是「import @codemirror 的任何东西」。
 */
import { describe, expect, it } from 'vitest'

/** 仓库内全部前端源码原文（?raw）；以项目根 `src/` 起算，key 形如 `/src/core/ui/...` */
const sources = import.meta.glob('/src/**/*.{ts,vue}', {
  eager: true,
  query: '?raw',
  import: 'default',
}) as Record<string, string>

/** 编辑器实现自身：基座 API 的合法使用者 */
const EDITOR_OWNER = ['/core/ui/UiCodeEditor.vue', '/core/ui/UiCodeDiff.vue', '/core/ui/editor/']

/** 排除测试文件：断言里的规则字面量会造成自我误报 */
const productSources = Object.entries(sources).filter(([path]) => !/\.test\.ts$/.test(path)) as [
  string,
  string,
][]

/** 工具层源码（插件与特性域） */
const toolSources = productSources.filter(([path]) => /\/(plugins|features)\//.test(path))

/** 编辑器实现自身不参与「禁止实例化」判定 */
const isEditorOwner = (path: string) => EDITOR_OWNER.some((owner) => path.includes(owner))

/** 命中规则的源码文件清单（用于报错时看到具体文件） */
function offenders(rx: RegExp, list: [string, string][] = productSources): string[] {
  return list.filter(([, source]) => rx.test(source)).map(([path]) => path)
}

describe('编辑器统一契约', () => {
  it('除编辑器实现自身外，任何地方不得实例化编辑器', () => {
    const instantiate =
      /\bnew\s+EditorView\s*\(|EditorState\.create\s*\(|new\s+EditorState\s*\(|monaco\.editor\.create\s*\(|ace\.edit\s*\(/
    const violations = productSources
      .filter(([path, source]) => !isEditorOwner(path) && instantiate.test(source))
      .map(([path]) => path)
    expect(violations).toEqual([])
  })

  it('不得引入其它编辑器基座（本仓库唯一基座是 CodeMirror 6）', () => {
    const thirdPartyBase =
      /from\s+['"](?:monaco-editor|ace-builds|quill|@tiptap\/[^'"]+|prosemirror-[^'"]+|@milkdown\/[^'"]+|vditor|codemirror)['"]/
    expect(offenders(thirdPartyBase)).toEqual([])
  })

  it('工具层不得直接引用编辑器内部实现（只能从 @/core/ui 取公共组件）', () => {
    const internalImport = /from\s+['"][^'"]*core\/ui\/editor\//
    expect(offenders(internalImport, toolSources)).toEqual([])
  })

  it('不得残留已删除的自研编辑器组件（LineNumberTextarea / CodeViewer）', () => {
    const legacy = /LineNumberTextarea|<CodeViewer|CodeViewer\.vue/
    expect(offenders(legacy)).toEqual([])
  })
})
