/**
 * 前端依赖守卫（任务书 2026-09-12 §5.3）
 *
 * 注意：本文件**刻意不写 `#!` shebang**。夹具测试会 `import` 本模块，而 vitest/vite 的 SSR 转换
 * 会把 import 提升到文件顶部，shebang 因此落到文件中间，变成 `Invalid or unexpected token`
 * 语法错误（未解析的 `#!`）。统一用 `node scripts/check_frontend_deps.mjs` 调用即可。
 *
 * 职责：用仓库已安装的 TypeScript 编译器 API 解析 `.ts/.tsx`，用 Vue 编译器（`vue/compiler-sfc`）
 * 解析 SFC 的 `<script>` / `<script setup>` 块后交给 TS 解析，抽取静态 import/export、
 * 字面量动态 import()、`@/` alias 与相对路径，按四条硬规则报告违规。
 *
 * 明确不承诺的事（避免“全库语义全覆盖”这类声称）：
 * - 只做**静态语法层**的模块关系提取；不执行代码、不解析运行期动态拼接的模块标识。
 * - 非字面量动态路径不做求值，一律列为「未解析项」并要求在基线例外文件登记（未登记即失败）。
 * - 不覆盖构建期插件注入、虚拟模块（virtual module）、`.d.ts` 声明文件、条件导出（package exports）
 *   的运行期语义；`node_modules` 裸包说明符视为外部依赖，不进入依赖图。
 * - 规则判定基于解析结果，不证明被扫描代码的语义正确性。
 *
 * 退出码：0 = 通过（含仅剩已登记例外）；1 = 新违规 / 失效例外 / 未登记未解析项；2 = 用法或环境错误。
 *
 * 可被测试复用的纯函数：loadAliases / extractScriptBlocks / analyzeSource / analyzeProject /
 * collectViolations / resolveSpecifier / evaluate / loadBaseline / formatReport。
 */
import fs from 'node:fs'
import path from 'node:path'
import process from 'node:process'
import { fileURLToPath, pathToFileURL } from 'node:url'
import ts from 'typescript'
// @vue/compiler-sfc：来自现有工具链（vue 3.5 的 `vue/compiler-sfc` 出口），不新增依赖。
import { parse as parseVueSFC } from 'vue/compiler-sfc'

const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url))
const SCRIPT_PATH = fileURLToPath(import.meta.url)

/** 源码根目录名（与 vite.config.ts 的 `@` alias 目标一致） */
export const SRC_DIR = 'src'

/** 参与依赖图扫描与解析的源码扩展名 */
const CODE_EXTENSIONS = ['.ts', '.tsx', '.mts', '.vue']

/** 非代码资源：命中即跳过（不计入依赖图，也不列为未解析项） */
const ASSET_EXTENSIONS = new Set([
  '.css',
  '.scss',
  '.sass',
  '.less',
  '.styl',
  '.svg',
  '.png',
  '.jpg',
  '.jpeg',
  '.gif',
  '.webp',
  '.ico',
  '.woff',
  '.woff2',
  '.ttf',
  '.mp3',
  '.wav',
  '.md',
  '.txt',
])

/** 测试文件不参与生产依赖守卫（测试允许 mock 或被 mock 的依赖组合） */
const TEST_FILE_RE = /\.(?:test|spec)\.[cm]?[jt]sx?$/

/** 环枚举上限：达到上限即视为检查未完成并失败退出，不做静默截断 */
const CYCLE_LIMIT = 500

/**
 * 应用 IPC 的真实入口（已在仓库内确认，不是猜测）：
 * - 框架级封装：`src/core/ipc/ipc.ts`（导出 `ipc` 调用表与 `invokeCommand`）；
 * - 业务插件：`src/plugins/<id>/ipc.ts`（各自封装自己的命令）。
 * `src/core/ipc/contracts.ts` 只声明类型与命令名常量表，不算 IPC 调用入口，不在此列。
 */
const APP_IPC_FILES = [`${SRC_DIR}/core/ipc/ipc.ts`]
const APP_IPC_PATTERN = new RegExp(`^${SRC_DIR}/plugins/[^/]+/ipc\\.ts$`)

/** core/ui 不得依赖的目录（相对扫描根） */
const CORE_UI_FORBIDDEN_DIRS = [
  `${SRC_DIR}/stores/`,
  `${SRC_DIR}/features/`,
  `${SRC_DIR}/plugins/`,
  `${SRC_DIR}/core/vault/`,
  `${SRC_DIR}/core/platform/`,
  `${SRC_DIR}/core/feedback/`,
]

/** core/ui 入口不得重新导出的来源目录（相对扫描根） */
const CORE_UI_ENTRY_FORBIDDEN_DIRS = [
  `${SRC_DIR}/core/vault/`,
  `${SRC_DIR}/core/platform/`,
  `${SRC_DIR}/core/feedback/`,
  `${SRC_DIR}/plugins/`,
  `${SRC_DIR}/stores/`,
  `${SRC_DIR}/features/`,
]

/** 规则总表：报告中的「规则数」= RULES.length */
export const RULES = [
  {
    id: 'R1',
    title: 'core/ui 边界',
    detail:
      'src/core/ui/** 不得依赖 src/stores/**、src/features/**、src/plugins/**、src/core/vault/** ' +
      '或平台/反馈层、pinia 与应用 IPC（src/core/ipc/ipc.ts、src/plugins/*/ipc.ts）',
  },
  {
    id: 'R2',
    title: '插件隔离',
    detail: 'src/plugins/A/** 不得 import src/plugins/B/** 的内部模块（A≠B，含类型导入）',
  },
  {
    id: 'R3',
    title: 'core/ui 入口重导出',
    detail:
      'src/core/ui/index.ts 不得 re-export 来自 src/core/vault、src/plugins、src/stores、src/features ' +
      '的模块（含经由被重导出模块的运行期传递依赖）',
  },
  {
    id: 'R4',
    title: '公共域运行依赖环',
    detail:
      'src/core/** 的运行期依赖图不得有环；`import type`、`export type`、内联 `type` 修饰符不算运行期依赖',
  },
]

// ── 路径 / alias ────────────────────────────────────────────────────────────

function toPosix(value) {
  return value.split(path.sep).join('/')
}

function isUnder(relPath, dirPrefix) {
  return relPath === dirPrefix || relPath.startsWith(dirPrefix)
}

/**
 * 读取 alias：优先解析扫描根下的 `vite.config.ts`（`'@': fileURLToPath(new URL('./src', ...))` 形式），
 * 解析不到时回退到与 vite.config.ts 一致的硬编码映射 `{'@': 'src'}`。
 * 返回值中的 `source` 会写进报告，便于确认 alias 来源。
 */
export function loadAliases(rootDir) {
  const aliases = new Map()
  const viteConfigPath = path.join(rootDir, 'vite.config.ts')
  let source = path.relative(rootDir, viteConfigPath)
  if (!fs.existsSync(viteConfigPath)) {
    source = '硬编码回退映射 {"@": "src"}（与 vite.config.ts resolve.alias 一致）'
    aliases.set('@', SRC_DIR)
    return { aliases, source }
  }
  const text = fs.readFileSync(viteConfigPath, 'utf8')
  const pattern =
    /(['"])([^'"]+)\1\s*:\s*fileURLToPath\(\s*new URL\(\s*(['"])([^'"]+)\3[^)]*\)\s*\)/g
  for (const match of text.matchAll(pattern)) {
    const key = match[2]
    const target = toPosix(match[4]).replace(/^\.?\//, '')
    aliases.set(key, target)
  }
  if (aliases.size === 0) {
    source = '硬编码回退映射 {"@": "src"}（vite.config.ts 未匹配到 fileURLToPath alias）'
    aliases.set('@', SRC_DIR)
  } else {
    source = `${source} 的 resolve.alias（${[...aliases].map(([k, v]) => `${k} -> ${v}`).join(', ')}）`
  }
  return { aliases, source }
}

function matchAlias(specifier, aliases) {
  for (const [key, target] of aliases) {
    if (specifier === key) return target
    if (specifier.startsWith(`${key}/`)) return `${target}${specifier.slice(key.length)}`
  }
  return null
}

function isRelativeSpecifier(specifier) {
  return (
    specifier === '.' ||
    specifier === '..' ||
    specifier.startsWith('./') ||
    specifier.startsWith('../')
  )
}

/**
 * 把源码说明符解析成扫描根下的相对路径（posix 分隔）。
 * 返回 { kind, relPath }，kind ∈ 'external' | 'asset' | 'local' | 'unresolved-path'。
 */
export function resolveSpecifier(specifier, fromRelPath, rootDir, aliases) {
  let base = null
  if (isRelativeSpecifier(specifier)) {
    base = path.posix.normalize(path.posix.join(path.posix.dirname(fromRelPath), specifier))
  } else {
    const aliased = matchAlias(specifier, aliases)
    if (aliased === null) return { kind: 'external', relPath: null }
    base = path.posix.normalize(aliased)
  }
  if (ASSET_EXTENSIONS.has(path.posix.extname(base).toLowerCase())) {
    return { kind: 'asset', relPath: null }
  }
  const candidates = [base]
  if (!path.posix.extname(base)) {
    for (const ext of CODE_EXTENSIONS) candidates.push(`${base}${ext}`)
    for (const ext of CODE_EXTENSIONS) candidates.push(`${base}/index${ext}`)
  }
  for (const candidate of candidates) {
    try {
      if (fs.statSync(path.join(rootDir, candidate)).isFile()) {
        return { kind: 'local', relPath: candidate }
      }
    } catch {
      // 候选路径不存在：继续尝试下一个
    }
  }
  return { kind: 'unresolved-path', relPath: base }
}

// ── 源码抽取与解析 ──────────────────────────────────────────────────────────

/**
 * 抽取脚本块：`.vue` 用 Vue 编译器解析出 `<script>` / `<script setup>` 内容（带行号偏移，
 * 使报告行号对应原文件），其余文件整体作为脚本。
 */
export function extractScriptBlocks(relPath, content) {
  if (!relPath.endsWith('.vue')) {
    return { blocks: [{ code: content, lineOffset: 0 }], errors: [] }
  }
  const { descriptor, errors } = parseVueSFC(content, { filename: relPath })
  const blocks = []
  for (const block of [descriptor.script, descriptor.scriptSetup]) {
    if (!block) continue
    blocks.push({ code: block.content, lineOffset: block.loc.start.line - 1 })
  }
  return { blocks, errors: (errors ?? []).map((err) => err.message ?? String(err)) }
}

function isTypeOnlyImport(clause) {
  if (!clause) return false
  if (clause.isTypeOnly) return true
  const bindings = clause.namedBindings
  if (bindings && ts.isNamedImports(bindings)) {
    return bindings.elements.length > 0 && bindings.elements.every((el) => el.isTypeOnly)
  }
  return false
}

function isTypeOnlyExport(node) {
  if (node.isTypeOnly) return true
  const clause = node.exportClause
  if (clause && ts.isNamedExports(clause)) {
    return clause.elements.length > 0 && clause.elements.every((el) => el.isTypeOnly)
  }
  return false
}

/**
 * 解析单个模块源码：返回 import/export 语句与「未解析项」。
 * 每条依赖记录：{ specifier, kind, typeOnly, line, resolved, resolution }，
 * kind ∈ 'static' | 'dynamic' | 're-export'（dynamic 仅字面量 import()）。
 */
export function analyzeSource({ relPath, content, rootDir, aliases }) {
  const imports = []
  const unresolved = []
  const extraction = extractScriptBlocks(relPath, content)
  for (const message of extraction.errors) {
    unresolved.push({
      kind: 'sfc-parse-error',
      path: relPath,
      target: message,
      line: 0,
      reason: 'Vue SFC 解析失败，该文件的依赖未被提取',
    })
  }
  for (const block of extraction.blocks) {
    const scriptKind = relPath.endsWith('.tsx') ? ts.ScriptKind.TSX : ts.ScriptKind.TS
    const sourceFile = ts.createSourceFile(
      relPath,
      block.code,
      ts.ScriptTarget.Latest,
      true,
      scriptKind
    )
    const lineOf = (node) =>
      sourceFile.getLineAndCharacterOfPosition(node.getStart(sourceFile)).line +
      1 +
      block.lineOffset
    const push = (specifier, kind, typeOnly, node) => {
      const resolution = resolveSpecifier(specifier, relPath, rootDir, aliases)
      imports.push({
        specifier,
        kind,
        typeOnly,
        line: lineOf(node),
        resolution: resolution.kind,
        resolved: resolution.relPath,
      })
    }
    const visit = (node) => {
      if (ts.isImportDeclaration(node)) {
        if (ts.isStringLiteral(node.moduleSpecifier)) {
          push(node.moduleSpecifier.text, 'static', isTypeOnlyImport(node.importClause), node)
        } else {
          unresolved.push({
            kind: 'nonliteral-module-specifier',
            path: relPath,
            target: node.moduleSpecifier.getText(sourceFile),
            line: lineOf(node),
            reason: 'import 说明符不是字符串字面量，无法静态解析',
          })
        }
      } else if (ts.isExportDeclaration(node) && node.moduleSpecifier) {
        if (ts.isStringLiteral(node.moduleSpecifier)) {
          push(node.moduleSpecifier.text, 're-export', isTypeOnlyExport(node), node)
        } else {
          unresolved.push({
            kind: 'nonliteral-module-specifier',
            path: relPath,
            target: node.moduleSpecifier.getText(sourceFile),
            line: lineOf(node),
            reason: 'export ... from 说明符不是字符串字面量，无法静态解析',
          })
        }
      } else if (
        ts.isCallExpression(node) &&
        node.expression.kind === ts.SyntaxKind.ImportKeyword
      ) {
        const [arg] = node.arguments
        if (arg && ts.isStringLiteral(arg)) {
          push(arg.text, 'dynamic', false, node)
        } else {
          unresolved.push({
            kind: 'dynamic-nonliteral',
            path: relPath,
            target: arg ? arg.getText(sourceFile) : 'import()',
            line: lineOf(node),
            reason: '动态 import 路径不是字符串字面量，需在基线例外文件中显式登记',
          })
        }
      }
      ts.forEachChild(node, visit)
    }
    visit(sourceFile)
  }
  return { relPath, imports, unresolved }
}

/** 递归收集扫描范围内的源码文件（相对扫描根的 posix 路径） */
export function listSourceFiles({ rootDir, srcDir = SRC_DIR }) {
  const files = []
  const walk = (relDir) => {
    const absDir = path.join(rootDir, relDir)
    for (const entry of fs.readdirSync(absDir, { withFileTypes: true })) {
      if (entry.name === 'node_modules' || entry.name.startsWith('.') || entry.name === 'dist') {
        continue
      }
      const rel = `${relDir}/${entry.name}`
      if (entry.isDirectory()) {
        walk(rel)
        continue
      }
      const ext = path.extname(entry.name).toLowerCase()
      if (!CODE_EXTENSIONS.includes(ext)) continue
      if (TEST_FILE_RE.test(entry.name)) continue
      files.push(rel)
    }
  }
  walk(srcDir)
  return files.sort()
}

/** 解析整个扫描范围：返回 { rootDir, files, modules, unresolved }（modules 以相对路径为键） */
export function analyzeProject({ rootDir, srcDir = SRC_DIR, aliases } = {}) {
  if (!fs.existsSync(path.join(rootDir, srcDir))) {
    throw new Error(`扫描根下找不到源码目录：${path.join(rootDir, srcDir)}`)
  }
  const aliasInfo = aliases ?? loadAliases(rootDir).aliases
  const files = listSourceFiles({ rootDir, srcDir })
  const modules = new Map()
  const unresolved = []
  for (const relPath of files) {
    const content = fs.readFileSync(path.join(rootDir, relPath), 'utf8')
    const analysis = analyzeSource({ relPath, content, rootDir, aliases: aliasInfo })
    modules.set(relPath, analysis)
    unresolved.push(...analysis.unresolved)
  }
  return { rootDir, srcDir, files, modules, unresolved }
}

/** 运行期依赖（type-only 不算） */
function runtimeImports(module) {
  return module.imports.filter((imp) => !imp.typeOnly)
}

// ── 规则判定 ────────────────────────────────────────────────────────────────

function isForbiddenCoreUiTarget(resolved) {
  if (!resolved) return false
  if (CORE_UI_FORBIDDEN_DIRS.some((dir) => isUnder(resolved, dir))) return true
  if (APP_IPC_FILES.includes(resolved)) return true
  return APP_IPC_PATTERN.test(resolved)
}

function isForbiddenCoreUiEntrySource(resolved) {
  if (!resolved) return false
  return CORE_UI_ENTRY_FORBIDDEN_DIRS.some((dir) => isUnder(resolved, dir))
}

/**
 * 从 startRel 出发的运行期可达集合（用于 R3 传递依赖判定）。
 * `ignore` 里的模块不参与遍历：R3 判定时忽略 core/ui 入口自身，否则任何「被入口重导出、又 import 入口」
 * 的组件都会把入口的全部重导出继承过来，造成连锁误判（评审 §5.2.4 反对的路径式武断判定）。
 */
function collectReachableRuntime(project, startRel, ignore = new Set()) {
  const seen = new Set(ignore)
  const stack = [startRel]
  while (stack.length > 0) {
    const current = stack.pop()
    if (seen.has(current)) continue
    seen.add(current)
    const module = project.modules.get(current)
    if (!module) continue
    for (const imp of runtimeImports(module)) {
      if (imp.resolved && project.modules.has(imp.resolved)) stack.push(imp.resolved)
    }
  }
  seen.delete(startRel)
  return seen
}

/**
 * 枚举运行期依赖图的基本环（每个环只报一次：以环内字典序最小的节点为起点，
 * 遍历时只走字典序不小于起点的节点）。返回 { cycles, truncated }。
 * 说明：只枚举基本环（elementary cycle），不合并强连通分量，便于例外绑定到具体环。
 */
export function findElementaryCycles(nodes, edgesOf, { limit = CYCLE_LIMIT } = {}) {
  const sorted = [...nodes].sort()
  const order = new Map(sorted.map((node, position) => [node, position]))
  const cycles = []
  let truncated = false
  for (const start of sorted) {
    const path = [start]
    const onPath = new Set([start])
    const walk = (node) => {
      if (truncated) return
      for (const next of edgesOf(node)) {
        if (order.get(next) < order.get(start)) continue
        if (next === start) {
          cycles.push([...path])
          if (cycles.length >= limit) truncated = true
          if (truncated) return
          continue
        }
        if (onPath.has(next)) continue
        path.push(next)
        onPath.add(next)
        walk(next)
        path.pop()
        onPath.delete(next)
        if (truncated) return
      }
    }
    walk(start)
    if (truncated) break
  }
  return { cycles, truncated }
}

/**
 * 按四条硬规则收集违规：返回 { violations }（路径为扫描根下的 posix 相对路径）。
 * violation 结构：{ rule, path, target, line?, typeOnly?, resolved?, detail? }
 */
export function collectViolations(project) {
  const violations = []
  const coreUiPrefix = `${project.srcDir}/core/ui/`
  const pluginsPrefix = `${project.srcDir}/plugins/`

  // R1 / R2：逐条依赖判定
  for (const [relPath, module] of project.modules) {
    // 插件 owner 只取 `src/plugins/<owner>/` 目录内的文件；`src/plugins/index.ts` 是插件引导（组装根），
    // 它的职责就是逐条 import 各插件完成注册，不属于插件间互相 import。
    const pluginTail = relPath.startsWith(pluginsPrefix)
      ? relPath.slice(pluginsPrefix.length)
      : null
    const pluginOwner = pluginTail && pluginTail.includes('/') ? pluginTail.split('/')[0] : null
    for (const imp of module.imports) {
      if (
        relPath.startsWith(coreUiPrefix) &&
        (isForbiddenCoreUiTarget(imp.resolved) || imp.specifier === 'pinia')
      ) {
        violations.push({
          rule: 'R1',
          path: relPath,
          target: imp.specifier,
          line: imp.line,
          typeOnly: imp.typeOnly,
          resolved: imp.resolved,
        })
      }
      if (
        pluginOwner &&
        imp.resolved &&
        imp.resolved.startsWith(pluginsPrefix) &&
        imp.resolved.slice(pluginsPrefix.length).split('/')[0] !== pluginOwner
      ) {
        violations.push({
          rule: 'R2',
          path: relPath,
          target: imp.specifier,
          line: imp.line,
          typeOnly: imp.typeOnly,
          resolved: imp.resolved,
        })
      }
    }
  }

  // R3：core/ui 入口的重导出（含运行期传递依赖）
  const entryPath = `${project.srcDir}/core/ui/index.ts`
  const entry = project.modules.get(entryPath)
  if (entry) {
    for (const imp of entry.imports) {
      if (imp.kind !== 're-export') continue
      if (isForbiddenCoreUiEntrySource(imp.resolved)) {
        violations.push({
          rule: 'R3',
          path: entryPath,
          target: imp.specifier,
          line: imp.line,
          typeOnly: imp.typeOnly,
          resolved: imp.resolved,
          detail: `直接重导出受限来源 ${imp.resolved}`,
        })
        continue
      }
      if (!imp.resolved) continue
      const reachable = collectReachableRuntime(project, imp.resolved, new Set([entryPath]))
      const forbidden = [...reachable].find((rel) => isForbiddenCoreUiEntrySource(rel))
      if (forbidden) {
        violations.push({
          rule: 'R3',
          path: entryPath,
          target: imp.specifier,
          line: imp.line,
          typeOnly: imp.typeOnly,
          resolved: imp.resolved,
          detail: `经由 ${imp.resolved} 的运行期传递依赖命中受限来源 ${forbidden}`,
        })
      }
    }
  }

  // R4：src/core/** 运行期依赖图的环（只枚举基本环）
  const corePrefix = `${project.srcDir}/core/`
  const coreNodes = [...project.modules.keys()].filter((rel) => rel.startsWith(corePrefix))
  const coreNodeSet = new Set(coreNodes)
  const edgesOf = (node) => {
    const module = project.modules.get(node)
    if (!module) return []
    const next = []
    for (const imp of runtimeImports(module)) {
      if (imp.resolved && coreNodeSet.has(imp.resolved)) next.push(imp.resolved)
    }
    return next
  }
  const { cycles, truncated } = findElementaryCycles(coreNodes, edgesOf)
  for (const cycle of cycles) {
    violations.push({
      rule: 'R4',
      path: cycle[0],
      target: `${cycle.join(' -> ')} -> ${cycle[0]}`,
      line: 0,
      typeOnly: false,
      resolved: null,
      detail: `运行期依赖环（${cycle.length} 个模块，只统计运行期依赖）`,
    })
  }

  return { violations: dedupeViolations(violations), truncated }
}

function dedupeViolations(violations) {
  const seen = new Set()
  const unique = []
  for (const item of violations) {
    const key = `${item.rule}\u0000${item.path}\u0000${item.target}`
    if (seen.has(key)) continue
    seen.add(key)
    unique.push(item)
  }
  return unique.sort(
    (a, b) =>
      a.rule.localeCompare(b.rule) ||
      a.path.localeCompare(b.path) ||
      a.target.localeCompare(b.target)
  )
}

// ── 例外 / 基线 ─────────────────────────────────────────────────────────────

/** 基线文件结构：{ "exceptions": [{rule,path,target,reason}], "unresolved": [{path,target,reason}] } */
export function loadBaseline(baselinePath) {
  if (!fs.existsSync(baselinePath)) {
    throw new Error(`基线例外文件不存在：${baselinePath}`)
  }
  let parsed
  try {
    parsed = JSON.parse(fs.readFileSync(baselinePath, 'utf8'))
  } catch (err) {
    throw new Error(`基线例外文件不是合法 JSON：${baselinePath}（${err.message}）`)
  }
  const exceptions = Array.isArray(parsed.exceptions) ? parsed.exceptions : []
  const unresolved = Array.isArray(parsed.unresolved) ? parsed.unresolved : []
  for (const item of [...exceptions, ...unresolved]) {
    if (!item || typeof item.path !== 'string' || typeof item.target !== 'string') {
      throw new Error(`基线条目缺少 path/target 字段：${JSON.stringify(item)}`)
    }
  }
  for (const item of exceptions) {
    if (typeof item.rule !== 'string' || item.rule.length === 0) {
      throw new Error(`基线例外缺少 rule 字段：${JSON.stringify(item)}`)
    }
  }
  return { exceptions, unresolved }
}

const violationKey = (rule, relPath, target) => `${rule}\u0000${relPath}\u0000${target}`
const unresolvedKey = (relPath, target) => `${relPath}\u0000${target}`

/**
 * 棘轮判定：
 * - 已登记违规不算失败（registered）；
 * - 未登记违规失败（newViolations）；
 * - 登记了但当前已不命中的例外失败（staleExceptions / staleUnresolved）；
 * - 未登记的未解析项失败（unregisteredUnresolved）。
 */
export function evaluate({ violations, unresolved, baseline }) {
  const exceptionIndex = new Map(
    baseline.exceptions.map((item) => [violationKey(item.rule, item.path, item.target), item])
  )
  const unresolvedIndex = new Map(
    baseline.unresolved.map((item) => [unresolvedKey(item.path, item.target), item])
  )
  const registered = []
  const newViolations = []
  const hitExceptions = new Set()
  for (const violation of violations) {
    const key = violationKey(violation.rule, violation.path, violation.target)
    if (exceptionIndex.has(key)) {
      hitExceptions.add(key)
      registered.push({ ...violation, reason: exceptionIndex.get(key).reason })
    } else {
      newViolations.push(violation)
    }
  }
  const staleExceptions = baseline.exceptions.filter(
    (item) => !hitExceptions.has(violationKey(item.rule, item.path, item.target))
  )
  const registeredUnresolved = []
  const unregisteredUnresolved = []
  const hitUnresolved = new Set()
  for (const item of unresolved) {
    const key = unresolvedKey(item.path, item.target)
    if (unresolvedIndex.has(key)) {
      hitUnresolved.add(key)
      registeredUnresolved.push({ ...item, reason: unresolvedIndex.get(key).reason })
    } else {
      unregisteredUnresolved.push(item)
    }
  }
  const staleUnresolved = baseline.unresolved.filter(
    (item) => !hitUnresolved.has(unresolvedKey(item.path, item.target))
  )
  const ok =
    newViolations.length === 0 &&
    staleExceptions.length === 0 &&
    unregisteredUnresolved.length === 0 &&
    staleUnresolved.length === 0
  return {
    ok,
    registered,
    newViolations,
    staleExceptions,
    registeredUnresolved,
    unregisteredUnresolved,
    staleUnresolved,
    totalViolations: violations.length,
    totalUnresolved: unresolved.length,
  }
}

// ── 报告 ────────────────────────────────────────────────────────────────────

function topLevelScope(files) {
  const buckets = new Map()
  for (const rel of files) {
    const parts = rel.split('/')
    const key = parts.length > 2 ? `${parts[0]}/${parts[1]}` : rel
    buckets.set(key, (buckets.get(key) ?? 0) + 1)
  }
  return [...buckets]
    .sort((a, b) => a[0].localeCompare(b[0]))
    .map(([dir, count]) => `${dir}(${count})`)
    .join('、')
}

export function formatReport({ project, aliasSource, baselinePath, result, truncated }) {
  const lines = []
  lines.push('前端依赖守卫报告（AR02 §5.3）')
  lines.push('='.repeat(72))
  lines.push(`规则数：${RULES.length}`)
  for (const rule of RULES) lines.push(`  - ${rule.id} ${rule.title}：${rule.detail}`)
  const runtimeEdges = [...project.modules.values()].reduce(
    (sum, mod) => sum + runtimeImports(mod).length,
    0
  )
  const typeEdges = [...project.modules.values()].reduce(
    (sum, mod) => sum + mod.imports.filter((imp) => imp.typeOnly).length,
    0
  )
  const externalOrAsset = [...project.modules.values()].reduce(
    (sum, mod) =>
      sum +
      mod.imports.filter((imp) => imp.resolution === 'external' || imp.resolution === 'asset')
        .length,
    0
  )
  lines.push('')
  lines.push(
    `源范围：扫描 ${SRC_DIR}/ 下 ${project.files.length} 个模块（.ts/.tsx/.mts/.vue，排除 *.test.*/*.spec.*、node_modules）`
  )
  lines.push(`  目录分布：${topLevelScope(project.files)}`)
  lines.push(`  alias 来源：${aliasSource}`)
  lines.push(`  基线例外文件：${baselinePath}`)
  lines.push(
    `  解析到的依赖：运行期 ${runtimeEdges} 条、仅类型 ${typeEdges} 条、外部/资源跳过 ${externalOrAsset} 条`
  )
  lines.push('')
  lines.push(`命中已登记例外：${result.registered.length} 条（棘轮：不再失败）`)
  for (const item of result.registered) {
    lines.push(
      `  - [${item.rule}] ${item.path} -> ${item.target}${item.line ? `:${item.line}` : ''}`
    )
    lines.push(`      登记原因：${item.reason ?? '(未写原因)'}`)
  }
  lines.push('')
  lines.push(`新违规：${result.newViolations.length} 条`)
  for (const item of result.newViolations) {
    lines.push(
      `  - [${item.rule}] ${item.path} -> ${item.target}${item.line ? `:${item.line}` : ''}`
    )
    if (item.resolved)
      lines.push(`      解析到：${item.resolved}${item.typeOnly ? '（仅类型导入）' : ''}`)
    if (item.detail) lines.push(`      ${item.detail}`)
  }
  lines.push('')
  lines.push(`失效例外（登记了但已不再命中）：${result.staleExceptions.length} 条`)
  for (const item of result.staleExceptions) {
    lines.push(`  - [${item.rule}] ${item.path} -> ${item.target}`)
  }
  lines.push('')
  lines.push(
    `未解析项：${result.totalUnresolved} 条（已登记 ${result.registeredUnresolved.length}，未登记 ${result.unregisteredUnresolved.length}，失效登记 ${result.staleUnresolved.length}）`
  )
  for (const item of result.unregisteredUnresolved) {
    lines.push(
      `  - [未登记] (${item.kind}) ${item.path}${item.line ? `:${item.line}` : ''} -> ${item.target}`
    )
    lines.push(`      ${item.reason}`)
  }
  for (const item of result.registeredUnresolved) {
    lines.push(
      `  - [已登记] (${item.kind}) ${item.path}${item.line ? `:${item.line}` : ''} -> ${item.target}`
    )
    lines.push(`      登记原因：${item.reason ?? '(未写原因)'}`)
  }
  for (const item of result.staleUnresolved) {
    lines.push(`  - [失效登记] ${item.path} -> ${item.target}`)
  }
  lines.push('')
  if (truncated) {
    lines.push(
      `⚠ 环枚举达到上限 ${CYCLE_LIMIT} 条，仍有未枚举的运行期依赖环：本次检查不完整，按失败处理。`
    )
    lines.push('')
  }
  lines.push(
    `依赖语句合计：${result.totalViolations} 条违规命中（其中新违规 ${result.newViolations.length}）`
  )
  lines.push(
    '范围声明：本检查只基于 TypeScript / Vue 编译器的静态 import-export 语法解析，不执行代码、'
  )
  lines.push(
    '不解析运行期动态拼接路径、构建期插件注入或虚拟模块；不证明被扫描代码语义正确，也不代表全库语义全覆盖。'
  )
  lines.push(
    result.ok && !truncated
      ? '结果：通过（无新违规、无失效例外、无未登记未解析项）'
      : '结果：失败（存在新违规 / 失效例外 / 未登记未解析项 / 环枚举不完整）'
  )
  return lines.join('\n')
}

// ── CLI ─────────────────────────────────────────────────────────────────────

export function parseArgs(argv) {
  const options = { root: process.cwd(), baseline: null, json: false, help: false }
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i]
    if (arg === '--help' || arg === '-h') options.help = true
    else if (arg === '--json') options.json = true
    else if (arg === '--root') options.root = argv[(i += 1)]
    else if (arg === '--baseline') options.baseline = argv[(i += 1)]
    else throw new Error(`未知参数：${arg}`)
  }
  if (!options.root) throw new Error('--root 需要目录参数')
  if (!options.baseline && argv.includes('--baseline')) throw new Error('--baseline 需要文件参数')
  return options
}

const USAGE = `用法：node scripts/check_frontend_deps.mjs [--root <扫描根>] [--baseline <例外文件>] [--json]

  --root      扫描根目录（默认当前工作目录），需包含 src/
  --baseline  例外/基线文件（默认脚本同目录的 frontend_deps_baseline.json）
  --json      以 JSON 输出结果摘要

退出码：0 通过（含仅剩已登记例外）；1 新违规/失效例外/未登记未解析项；2 用法或环境错误。`

export function main(argv, io = {}) {
  const log = io.log ?? ((text) => process.stdout.write(`${text}\n`))
  const errorLog = io.errorLog ?? ((text) => process.stderr.write(`${text}\n`))
  let options
  try {
    options = parseArgs(argv)
  } catch (err) {
    errorLog(`[前端依赖守卫] 参数错误：${err.message}`)
    errorLog(USAGE)
    return 2
  }
  if (options.help) {
    log(USAGE)
    return 0
  }
  const rootDir = path.resolve(options.root)
  const baselinePath = path.resolve(
    options.baseline ?? path.join(SCRIPT_DIR, 'frontend_deps_baseline.json')
  )
  try {
    const { aliases, source: aliasSource } = loadAliases(rootDir)
    const project = analyzeProject({ rootDir, srcDir: SRC_DIR, aliases })
    if (project.files.length === 0) {
      throw new Error('扫描结果为空，请检查 src 路径与文件发现规则')
    }
    const { violations, truncated } = collectViolations(project)
    const baseline = loadBaseline(baselinePath)
    const result = evaluate({ violations, unresolved: project.unresolved, baseline })
    const ok = result.ok && !truncated
    if (options.json) {
      log(
        JSON.stringify(
          {
            rules: RULES.map((rule) => rule.id),
            scannedFiles: project.files.length,
            violations: violations.length,
            unresolved: project.unresolved.length,
            registered: result.registered.length,
            newViolations: result.newViolations.map((v) => ({ ...v })),
            staleExceptions: result.staleExceptions,
            unregisteredUnresolved: result.unregisteredUnresolved,
            staleUnresolved: result.staleUnresolved,
            cycleEnumerationTruncated: truncated,
            ok,
          },
          null,
          2
        )
      )
    } else {
      log(formatReport({ project, aliasSource, baselinePath, result, truncated }))
    }
    return ok ? 0 : 1
  } catch (err) {
    errorLog(`[前端依赖守卫] 环境错误：${err.message}`)
    return 2
  }
}

const isMainEntry =
  process.argv[1] !== undefined &&
  pathToFileURL(path.resolve(process.argv[1])).href === pathToFileURL(SCRIPT_PATH).href

if (isMainEntry) {
  process.exitCode = main(process.argv.slice(2))
}
