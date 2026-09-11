/**
 * 编辑器契约类型
 *
 * 集中定义对外形状：光标信息、右键菜单负载、实例管理入参、命令式句柄。
 * 实现留在 `useCodeEditor.ts`，避免「契约 + 实现」混在一个文件里越写越长。
 */
import type { Ref } from 'vue'
import type { EditorView } from '@codemirror/view'
import type { Extension } from '@codemirror/state'
import type { CompletionSource } from '@codemirror/autocomplete'
import type { LanguageInfo } from './languages'
import type { EditorMode } from './extensions'
import type { EditorDegradeLevel } from './status'
import type { EditorSearchState } from './searchController'
import type { SearchOptions } from './search'

/** 光标信息（状态栏与上层联动用） */
export interface EditorCursorInfo {
  /** 行号（1 起始） */
  line: number
  /** 列号（1 起始） */
  column: number
  /** 当前选中字符数（无选中为 0） */
  selected: number
}

/** 选区偏移（供插件在自己的封装组件里按选区/光标取文本） */
export interface EditorCursorRange {
  /** 选区起点（文档偏移） */
  from: number
  /** 选区终点（文档偏移） */
  to: number
  /** 主光标位置（文档偏移） */
  head: number
}

/** 文档统计（状态栏展示，随编辑实时更新） */
export interface EditorDocStats {
  /** 总行数 */
  lines: number
  /** 总字符数 */
  length: number
}

/** 右键菜单负载（上层据此定位并渲染菜单） */
export interface EditorContextMenuPayload {
  /** 原始事件（取坐标用） */
  event: MouseEvent
  /** 触发位置所在行号（1 起始） */
  line: number
  /** 该行完整文本 */
  lineText: string
  /** 当前选中文本（无选中为空串） */
  selection: string
}

/** useCodeEditor 需要的响应式取值函数与回调（组件把 props/emit 适配进来） */
export interface UseCodeEditorOptions {
  /** 当前值 */
  modelValue: () => string
  /** 文件名（language='auto' 时用于识别） */
  filename: () => string | undefined
  /** 语言 id 或 'auto' */
  language: () => string | undefined
  /**
   * 直接注入语言扩展（可选，优先于自动识别）
   *
   * 用于需要参数化语言的场景：数据库插件的 SQL 方言（MySQL / PostgreSQL / SQLite）
   * 无法用语言 id 表达，由上层自行构造后注入；`language` 仍用于状态栏标签。
   */
  languageExtension?: () => Extension | Extension[]
  /** 只读 */
  readonly: () => boolean
  /** 档位 */
  mode: () => EditorMode
  /** 行号开关 */
  lineNumbers: () => boolean
  /** 折叠 gutter 开关（full 档） */
  foldGutter: () => boolean
  /** 长行换行开关 */
  lineWrapping: () => boolean
  /** 缩进宽度 */
  tabSize: () => number
  /** 占位提示 */
  placeholder: () => string | undefined
  /** 是否启用补全 */
  completion?: () => boolean
  /** 注入的补全源（数据库表名 / 列名等，优先于词法兜底） */
  completionSources?: () => CompletionSource[]
  /** 是否启用校验（语法错误波浪线） */
  linter?: () => boolean
  /**
   * 插件专用扩展（可选）
   *
   * 供上层在自己的薄封装里注入领域能力（如数据库插件的「执行当前语句」gutter、
   * Ctrl+点击表名跳转）。core 不解释这些扩展的内容，只负责随实例生命周期装载与释放。
   */
  extraExtensions?: () => Extension[]
  /** 用户编辑回调（外部写入不触发） */
  onChange: (value: string) => void
  /** 光标变化回调 */
  onCursor?: (info: EditorCursorInfo) => void
  /** Ctrl+S 回调 */
  onSave?: () => void
  /** 右键菜单回调 */
  onContextMenu?: (payload: EditorContextMenuPayload) => void
  /** 请求打开查找面板（Ctrl+F / Ctrl+H，replace 表示展开替换行） */
  onRequestSearch?: (replace: boolean) => void
  /** 请求打开跳转行面板（Ctrl+G） */
  onRequestGoToLine?: () => void
  /** 请求关闭浮层（Esc）；返回 true 表示确实关闭了，键位才算被消费 */
  onRequestClosePanel?: () => boolean
  /** 需要用户知晓的错误（如文件过大强制只读） */
  onError?: (message: string) => void
}

/** 编辑器实例控制句柄 */
export interface CodeEditorHandle {
  /** 挂载容器（模板 ref 绑定） */
  host: Ref<HTMLElement | null>
  /** 当前 EditorView（未挂载时为 null） */
  view: Ref<EditorView | null>
  /** 当前语言信息（异步识别结果） */
  languageInfo: Ref<LanguageInfo>
  /** 文档统计（行数 / 字符数） */
  docStats: Ref<EditorDocStats>
  /** 当前降级级别 */
  degrade: Ref<EditorDegradeLevel>
  /** 查找状态（面板绑定） */
  searchState: Ref<EditorSearchState>
  /** 创建编辑器（onMounted 调用） */
  mount: () => void
  /** 销毁编辑器 */
  destroy: () => void
  /** 读取内容 */
  getValue: () => string
  /** 写入内容（默认不进撤销历史） */
  setValue: (value: string, options?: { addToHistory?: boolean }) => void
  /** 聚焦 */
  focus: () => void
  /** 失焦 */
  blur: () => void
  /** 跳转到指定行（1 起始） */
  goToLine: (line: number) => void
  /** 读取选中文本 */
  getSelection: () => string
  /** 读取选区与光标偏移（插件封装组件按选区取文本用） */
  getCursor: () => EditorCursorRange
  /** 在光标处插入文本 */
  insert: (text: string) => void
  /** 撤销 */
  undo: () => void
  /** 重做 */
  redo: () => void
  /** 应用查找条件 */
  applySearch: (query: string, replacement: string, options: SearchOptions) => void
  /** 跳到下一个匹配 */
  findNext: () => void
  /** 跳到上一个匹配 */
  findPrevious: () => void
  /** 替换当前匹配 */
  replaceCurrent: () => void
  /** 替换全部匹配 */
  replaceAllMatches: () => void
  /** 清空查找条件与高亮 */
  clearSearch: () => void
  /** 按语言格式化（成功后写入并进撤销历史） */
  format: () => { ok: boolean; error?: string }
  /** 记录当前内容为「已保存」基线 */
  markSaved: () => void
  /** 相对基线是否有未保存修改 */
  isDirty: () => boolean
}

/** 差异统计（UiCodeDiff 顶部展示用） */
export interface DiffStats {
  /** 新增行数 */
  added: number
  /** 删除行数 */
  removed: number
  /** 是否完全相同 */
  same: boolean
}
