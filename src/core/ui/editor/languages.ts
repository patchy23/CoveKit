/**
 * 编辑器语言识别与懒加载
 *
 * 两条路径：
 * 1. `detectLanguage()` 是纯函数（带单测）：按文件名 / 扩展名给出语言 id 与中文标签，
 *    未命中一律回退 `plaintext`（不高亮、不报错）。
 * 2. `loadLanguage()` 按 id 懒加载语言扩展：高频语言走本仓库直接安装的包（动态 import，
 *    Vite 按语言分 chunk），其余交给 `@codemirror/language-data`（100+ 语言，含 legacy-modes
 *    提供的 Shell / Nginx / TOML / INI 等），加载失败回退空扩展而不是抛异常。
 */
import type { Extension } from '@codemirror/state'
import { LanguageDescription } from '@codemirror/language'

/** 语言识别结果 */
export interface LanguageInfo {
  /** 语言 id（本模块内部标识，亦用于选择加载器） */
  id: string
  /** 中文标签（状态栏展示用） */
  label: string
}

/** 纯文本（未识别时的兜底结果） */
export const PLAIN_TEXT: LanguageInfo = { id: 'plaintext', label: '纯文本' }

/** 语言 id → 中文标签（状态栏展示，批 2 使用） */
export const LANGUAGE_LABELS: Record<string, string> = {
  plaintext: '纯文本',
  json: 'JSON',
  xml: 'XML',
  html: 'HTML',
  css: 'CSS',
  javascript: 'JavaScript',
  typescript: 'TypeScript',
  python: 'Python',
  sql: 'SQL',
  yaml: 'YAML',
  toml: 'TOML',
  ini: 'INI',
  shell: 'Shell',
  markdown: 'Markdown',
  dockerfile: 'Dockerfile',
  nginx: 'Nginx',
  diff: 'Diff',
  makefile: 'Makefile',
}

/** 扩展名（小写，不含点）→ 语言 id */
export const EXTENSION_LANGUAGE: Record<string, string> = {
  json: 'json',
  jsonc: 'json',
  json5: 'json',
  xml: 'xml',
  svg: 'xml',
  plist: 'xml',
  html: 'html',
  htm: 'html',
  xhtml: 'html',
  vue: 'html',
  css: 'css',
  scss: 'css',
  less: 'css',
  js: 'javascript',
  mjs: 'javascript',
  cjs: 'javascript',
  jsx: 'javascript',
  ts: 'typescript',
  tsx: 'typescript',
  py: 'python',
  pyw: 'python',
  sql: 'sql',
  yml: 'yaml',
  yaml: 'yaml',
  toml: 'toml',
  ini: 'ini',
  conf: 'ini',
  cfg: 'ini',
  properties: 'ini',
  env: 'ini',
  sh: 'shell',
  bash: 'shell',
  zsh: 'shell',
  fish: 'shell',
  ksh: 'shell',
  md: 'markdown',
  markdown: 'markdown',
  diff: 'diff',
  patch: 'diff',
}

/** 特例文件名（小写全名）→ 语言 id */
export const FILENAME_LANGUAGE: Record<string, string> = {
  dockerfile: 'dockerfile',
  'dockerfile.dev': 'dockerfile',
  'nginx.conf': 'nginx',
  makefile: 'makefile',
  gnumakefile: 'makefile',
  'cmakelists.txt': 'plaintext',
  hosts: 'plaintext',
  '.gitignore': 'plaintext',
  '.dockerignore': 'plaintext',
  authorized_keys: 'plaintext',
}

/** 从路径中取出文件名（兼容 Windows 与 POSIX 分隔符） */
function baseName(input: string): string {
  const normalized = input.replace(/\\/g, '/')
  const parts = normalized.split('/')
  return parts[parts.length - 1] ?? ''
}

/**
 * 由文件名（或扩展名）识别语言。
 * 纯函数，可在单测中直接断言；不依赖 DOM 与 CodeMirror 运行时。
 */
export function detectLanguage(filenameOrExt?: string): LanguageInfo {
  const raw = (filenameOrExt ?? '').trim().toLowerCase()
  if (!raw) return PLAIN_TEXT

  const base = baseName(raw)
  if (!base) return PLAIN_TEXT

  // 特例文件名（含 .env / .env.local 这类点开头的后缀形式）
  const special = FILENAME_LANGUAGE[base]
  if (special) return { id: special, label: LANGUAGE_LABELS[special] ?? special }
  if (base === '.env' || base.startsWith('.env.')) {
    return { id: 'ini', label: LANGUAGE_LABELS.ini }
  }

  // 扩展名：取最后一个点之后的部分；点开头的隐藏文件不视为扩展名
  const dot = base.lastIndexOf('.')
  if (dot <= 0 || dot === base.length - 1) return PLAIN_TEXT
  const ext = base.slice(dot + 1)
  const id = EXTENSION_LANGUAGE[ext]
  if (!id) return PLAIN_TEXT
  return { id, label: LANGUAGE_LABELS[id] ?? id }
}

/** 本仓库直接安装的语言包加载器（动态 import → Vite 按语言分 chunk） */
const PACKAGE_LOADERS: Record<string, () => Promise<Extension>> = {
  json: () => import('@codemirror/lang-json').then((m) => m.json()),
  xml: () => import('@codemirror/lang-xml').then((m) => m.xml()),
  yaml: () => import('@codemirror/lang-yaml').then((m) => m.yaml()),
  javascript: () => import('@codemirror/lang-javascript').then((m) => m.javascript()),
  typescript: () =>
    import('@codemirror/lang-javascript').then((m) => m.javascript({ typescript: true })),
  python: () => import('@codemirror/lang-python').then((m) => m.python()),
  html: () => import('@codemirror/lang-html').then((m) => m.html()),
  css: () => import('@codemirror/lang-css').then((m) => m.css()),
  sql: () => import('@codemirror/lang-sql').then((m) => m.sql()),
}

/** language-data 里的语言名候选（按 id 逐个尝试匹配；用于包加载器之外的冷门语言） */
const LANGUAGE_DATA_NAMES: Record<string, string[]> = {
  toml: ['TOML'],
  ini: ['INI'],
  shell: ['Shell', 'Bash', 'sh'],
  dockerfile: ['Dockerfile'],
  nginx: ['Nginx'],
  markdown: ['Markdown'],
  diff: ['Diff'],
  makefile: ['Makefile'],
}

/**
 * 按语言 id 懒加载 CodeMirror 语言扩展。
 * 失败或未命中时返回空扩展（编辑器退化为纯文本，不影响编辑）。
 *
 * @param info `detectLanguage()` 的结果
 * @param filename 原始文件名（用于 language-data 的文件名匹配兜底）
 */
export async function loadLanguage(info: LanguageInfo, filename?: string): Promise<Extension> {
  // 纯文本不加载任何语法（无扩展名文件、hosts、.gitignore 等走这里）
  if (info.id === 'plaintext') return []

  const packageLoader = PACKAGE_LOADERS[info.id]
  if (packageLoader) {
    try {
      return await packageLoader()
    } catch {
      return []
    }
  }

  // 冷门语言：language-data 本体也按需加载，避免为偶尔打开的配置文件常驻 100+ 语言清单
  const names = LANGUAGE_DATA_NAMES[info.id] ?? []
  if (names.length === 0 && !filename) return []
  let available: readonly LanguageDescription[]
  try {
    const module = await import('@codemirror/language-data')
    available = module.languages
  } catch {
    return []
  }

  let description: LanguageDescription | null = null
  for (const name of names) {
    description = LanguageDescription.matchLanguageName(available, name, true)
    if (description) break
  }
  if (!description && filename) {
    description = LanguageDescription.matchFilename(available, filename)
  }
  if (!description) return []
  try {
    return await description.load()
  } catch {
    return []
  }
}
