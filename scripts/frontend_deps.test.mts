/**
 * 前端依赖守卫夹具测试（AR02 §5.3）
 *
 * 夹具策略：在系统临时目录里真实写文件（`fs.mkdtempSync(os.tmpdir())`），再用守卫的真实解析
 * 函数跑一遍断言；不做任何 mock，断言对象是 TypeScript / Vue 编译器解析出来的真实结果。
 *
 * 覆盖清单（任务书 §5.3 必需夹具）：alias import、相对 import、跨插件 import、core/ui 重导出、
 * barrel 依赖环、合法 type-only export 不算运行依赖、非字面量动态 import 列为未解析、
 * 例外命中 / 新违规 / 失效例外三种判定。
 */
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { dirname, join } from 'node:path'
import { afterAll, describe, expect, it } from 'vitest'
import {
  RULES,
  analyzeProject,
  collectViolations,
  evaluate,
  loadAliases,
  loadBaseline,
  main,
  resolveSpecifier,
} from './check_frontend_deps.mjs'

const tempDirs = []

/** 建一个临时工程目录，写入给定文件（键为相对工程根路径），返回工程根 */
function createProject(files) {
  const root = mkdtempSync(join(tmpdir(), 'fdeps-'))
  tempDirs.push(root)
  for (const [rel, content] of Object.entries(files)) {
    const abs = join(root, rel)
    mkdirSync(dirname(abs), { recursive: true })
    writeFileSync(abs, content, 'utf8')
  }
  return root
}

/** 跑一遍真实解析 + 规则判定 */
function runGuard(root) {
  const { aliases, source } = loadAliases(root)
  const project = analyzeProject({ rootDir: root, aliases })
  const { violations, truncated } = collectViolations(project)
  return { project, violations, truncated, aliases, aliasSource: source }
}

function violationsOf(violations, rule) {
  return violations.filter((item) => item.rule === rule)
}

afterAll(() => {
  for (const dir of tempDirs) rmSync(dir, { recursive: true, force: true })
})

describe('前端依赖守卫 · 规则表', () => {
  it('规则数为 4 且编号稳定', () => {
    expect(RULES.map((rule) => rule.id)).toEqual(['R1', 'R2', 'R3', 'R4'])
  })
})

describe('alias / 相对路径解析', () => {
  it('alias import 违规：core/ui 依赖 @/stores', () => {
    const root = createProject({
      'src/stores/ui.ts': 'export const useUiStore = () => ({})\n',
      'src/core/ui/Widget.ts':
        "import { useUiStore } from '@/stores/ui'\nexport const x = useUiStore\n",
    })
    const { violations, project, aliasSource } = runGuard(root)
    expect(aliasSource).toContain('硬编码回退映射')
    expect(project.files).toContain('src/core/ui/Widget.ts')
    const r1 = violationsOf(violations, 'R1')
    expect(r1).toHaveLength(1)
    expect(r1[0]).toMatchObject({
      path: 'src/core/ui/Widget.ts',
      target: '@/stores/ui',
      resolved: 'src/stores/ui.ts',
      line: 1,
    })
  })

  it('相对 import 违规：省略扩展名 / 目录 index / 省略 .vue 都能解析', () => {
    const root = createProject({
      'src/core/vault/helper.ts': 'export const helper = 1\n',
      'src/core/vault/index.ts': "export { helper } from './helper'\n",
      'src/core/vault/Comp.vue':
        '<script setup lang="ts">\nconst a = 1\n</script>\n<template><div /></template>\n',
      'src/core/ui/one.ts': "import { helper } from '../vault/helper'\nexport const one = helper\n",
      'src/core/ui/two.ts': "import { helper } from '../vault'\nexport const two = helper\n",
      'src/core/ui/three.ts': "import Comp from '../vault/Comp'\nexport const three = Comp\n",
    })
    const { violations, project } = runGuard(root)
    const r1 = violationsOf(violations, 'R1')
    expect(r1.map((item) => [item.path, item.resolved]).sort()).toEqual([
      ['src/core/ui/one.ts', 'src/core/vault/helper.ts'],
      ['src/core/ui/three.ts', 'src/core/vault/Comp.vue'],
      ['src/core/ui/two.ts', 'src/core/vault/index.ts'],
    ])
    // 解析复用函数：显式断言三种形态都命中真实文件
    const aliases = new Map([['@', 'src']])
    expect(resolveSpecifier('../vault', 'src/core/ui/two.ts', root, aliases)).toEqual({
      kind: 'local',
      relPath: 'src/core/vault/index.ts',
    })
    expect(resolveSpecifier('@/core/vault/Comp', 'src/core/ui/three.ts', root, aliases).kind).toBe(
      'local'
    )
    expect(project.modules.get('src/core/ui/three.ts').imports[0].resolved).toBe(
      'src/core/vault/Comp.vue'
    )
  })

  it('alias 从 vite.config.ts 读取（与硬编码回退不同）', () => {
    const root = createProject({
      'vite.config.ts':
        "import { fileURLToPath, URL } from 'node:url'\nexport default { resolve: { alias: { '@': fileURLToPath(new URL('./src', import.meta.url)) } } }\n",
      'src/core/ui/Widget.ts': "import { useUiStore } from '@/stores/ui'\n",
      'src/stores/ui.ts': 'export const useUiStore = () => ({})\n',
    })
    const { aliases, aliasSource, violations } = runGuard(root)
    expect(aliases.get('@')).toBe('src')
    expect(aliasSource).toContain('vite.config.ts')
    expect(violationsOf(violations, 'R1')).toHaveLength(1)
  })

  it('外部包与资源 import 不进入依赖图', () => {
    const root = createProject({
      'src/core/ui/Widget.ts':
        "import { ref } from 'vue'\nimport icon from '@/assets/icon.svg'\nimport './Widget.css'\nexport const x = [ref, icon]\n",
      'src/assets/icon.svg': '<svg />',
      'src/core/ui/Widget.css': '.a{color:red}\n',
    })
    const { project, violations } = runGuard(root)
    const imports = project.modules.get('src/core/ui/Widget.ts').imports
    expect(imports.map((imp) => imp.resolution)).toEqual(['external', 'asset', 'asset'])
    expect(violations).toEqual([])
    expect(project.unresolved).toEqual([])
  })
})

describe('插件隔离（R2）', () => {
  it('跨插件 import 违规，且插件引导（组装根）不受影响', () => {
    const root = createProject({
      'src/plugins/alpha/index.ts': "export { useAlpha } from './useAlpha'\n",
      'src/plugins/alpha/useAlpha.ts':
        "import { useBeta } from '../beta/useBeta'\nexport const useAlpha = useBeta\n",
      'src/plugins/beta/useBeta.ts': 'export const useBeta = () => 1\n',
      'src/plugins/index.ts':
        "import '@/plugins/alpha'\nimport '@/plugins/beta'\nexport const bootstrapped = true\n",
    })
    const { violations } = runGuard(root)
    const r2 = violationsOf(violations, 'R2')
    expect(r2).toHaveLength(1)
    expect(r2[0]).toMatchObject({
      path: 'src/plugins/alpha/useAlpha.ts',
      target: '../beta/useBeta',
      resolved: 'src/plugins/beta/useBeta.ts',
      typeOnly: false,
    })
    // 组装根 src/plugins/index.ts 逐条导入插件是它的职责，不得判违规
    expect(violations.filter((item) => item.path === 'src/plugins/index.ts')).toEqual([])
  })

  it('同插件内部 import 不算违规', () => {
    const root = createProject({
      'src/plugins/alpha/index.ts': "export { a } from './impl'\n",
      'src/plugins/alpha/impl.ts': 'export const a = 1\n',
    })
    const { violations } = runGuard(root)
    expect(violationsOf(violations, 'R2')).toEqual([])
  })
})

describe('core/ui 入口重导出（R3）', () => {
  it('直接与传递重导出受限来源都报违规，基础控件重导出不报', () => {
    const root = createProject({
      'src/core/ui/index.ts': [
        "export { default as UiButton } from './UiButton.vue'",
        "export { useVault } from '@/core/vault/useVault'",
        "export { default as CredentialPicker } from './CredentialPicker.vue'",
        "export type { UiSize } from './types'",
        '',
      ].join('\n'),
      'src/core/ui/UiButton.vue':
        '<script setup lang="ts">\nconst a = 1\n</script>\n<template><button /></template>\n',
      'src/core/ui/CredentialPicker.vue':
        '<script setup lang="ts">\nimport { useVault } from \'@/core/vault/useVault\'\nconst v = useVault()\n</script>\n<template><div /></template>\n',
      'src/core/ui/types.ts': "export type UiSize = 'sm' | 'md'\n",
      'src/core/vault/useVault.ts': 'export const useVault = () => ({})\n',
    })
    const { violations } = runGuard(root)
    const r3 = violationsOf(violations, 'R3')
    expect(r3.map((item) => item.target).sort()).toEqual([
      './CredentialPicker.vue',
      '@/core/vault/useVault',
    ])
    const direct = r3.find((item) => item.target === '@/core/vault/useVault')
    const transitive = r3.find((item) => item.target === './CredentialPicker.vue')
    expect(direct.detail).toContain('直接重导出')
    expect(transitive.detail).toContain('传递依赖命中')
    expect(r3.some((item) => item.target === './UiButton.vue')).toBe(false)
    expect(r3.some((item) => item.target === './types')).toBe(false)
  })
})

describe('公共域运行依赖环（R4）', () => {
  it('barrel 依赖环被报出（含目录 index 与扩展名省略）', () => {
    const root = createProject({
      'src/core/alpha/index.ts': "export { a } from './impl'\n",
      'src/core/alpha/impl.ts': "import { b } from '../beta'\nexport const a = b\n",
      'src/core/beta/index.ts': "import { a } from '../alpha'\nexport const b = a\n",
    })
    const { violations } = runGuard(root)
    const r4 = violationsOf(violations, 'R4')
    expect(r4).toHaveLength(1)
    expect(r4[0].target).toBe(
      'src/core/alpha/impl.ts -> src/core/beta/index.ts -> src/core/alpha/index.ts -> src/core/alpha/impl.ts'
    )
    expect(r4[0].path).toBe('src/core/alpha/impl.ts')
  })

  it('字面量动态 import 计入运行期依赖，也能成环', () => {
    const root = createProject({
      'src/core/dyn/index.ts': "export const load = () => import('./impl')\n",
      'src/core/dyn/impl.ts': "export const back = async () => import('./index')\n",
    })
    const { project, violations } = runGuard(root)
    expect(project.unresolved).toEqual([])
    expect(project.modules.get('src/core/dyn/index.ts').imports[0]).toMatchObject({
      kind: 'dynamic',
      resolved: 'src/core/dyn/impl.ts',
    })
    expect(violationsOf(violations, 'R4')).toHaveLength(1)
  })

  it('合法 type-only export 不算运行依赖，不构成环', () => {
    const root = createProject({
      'src/core/gamma/index.ts': "export { g } from './impl'\n",
      'src/core/gamma/impl.ts': "export type { Delta } from '../delta'\nexport const g = 1\n",
      'src/core/delta/index.ts':
        "import type { Gamma } from '../gamma'\nexport const build = (input: Gamma): Gamma => input\n",
    })
    const { violations, project } = runGuard(root)
    // 三条语句都被解析到，但环只由 type 边构成 → 不报 R4
    expect(project.modules.get('src/core/delta/index.ts').imports[0].typeOnly).toBe(true)
    expect(project.modules.get('src/core/gamma/impl.ts').imports[0].typeOnly).toBe(true)
    expect(violationsOf(violations, 'R4')).toEqual([])
  })

  it('内联 type 修饰符的混合导入按运行期依赖处理', () => {
    const root = createProject({
      'src/core/mixed/a.ts':
        "import { type T, value } from './b'\nexport const a = [value, 0 as unknown as T]\n",
      'src/core/mixed/b.ts':
        "import { a } from './a'\nexport type T = number\nexport const value = () => a\n",
    })
    const { violations, project } = runGuard(root)
    expect(project.modules.get('src/core/mixed/a.ts').imports[0].typeOnly).toBe(false)
    expect(violationsOf(violations, 'R4')).toHaveLength(1)
  })
})

describe('未解析项', () => {
  it('非字面量动态 import（变量 / 模板字符串）列为未解析并保留位置', () => {
    const root = createProject({
      'src/features/loader.ts': [
        'const name = "dyn"',
        'export const one = () => import(name)',
        'export const two = () => import(`./${name}.ts`)',
        '',
      ].join('\n'),
    })
    const { project, violations } = runGuard(root)
    expect(violations).toEqual([])
    expect(
      project.unresolved.map((item) => [item.kind, item.path, item.line, item.target])
    ).toEqual([
      ['dynamic-nonliteral', 'src/features/loader.ts', 2, 'name'],
      ['dynamic-nonliteral', 'src/features/loader.ts', 3, '`./${name}.ts`'],
    ])
  })

  it('Vue SFC 的 <script setup> 块被解析，行号对应原文件', () => {
    const root = createProject({
      'src/stores/ui.ts': 'export const useUiStore = () => ({})\n',
      'src/core/ui/Bad.vue': [
        '<template>',
        '  <div />',
        '</template>',
        '<script setup lang="ts">',
        "import { useUiStore } from '@/stores/ui'",
        'const s = useUiStore()',
        '</script>',
        '',
      ].join('\n'),
    })
    const { violations } = runGuard(root)
    const r1 = violationsOf(violations, 'R1')
    expect(r1).toHaveLength(1)
    expect(r1[0]).toMatchObject({
      path: 'src/core/ui/Bad.vue',
      target: '@/stores/ui',
      line: 5,
    })
  })
})

describe('棘轮判定（例外命中 / 新违规 / 失效例外）', () => {
  const fixture = () =>
    createProject({
      'src/stores/ui.ts': 'export const useUiStore = () => ({})\n',
      'src/core/ui/Widget.ts':
        "import { useUiStore } from '@/stores/ui'\nexport const x = useUiStore\n",
      'src/features/loader.ts': 'const name = "dyn"\nexport const one = () => import(name)\n',
    })

  it('已登记的违规不算失败，未登记即失败', () => {
    const root = fixture()
    const { violations, project } = runGuard(root)
    const registration = {
      rule: 'R1',
      path: 'src/core/ui/Widget.ts',
      target: '@/stores/ui',
      reason: '夹具：已登记违规',
    }
    const unresolvedRegistration = {
      path: 'src/features/loader.ts',
      target: 'name',
      reason: '夹具：已登记未解析项',
    }
    const registeredResult = evaluate({
      violations,
      unresolved: project.unresolved,
      baseline: { exceptions: [registration], unresolved: [unresolvedRegistration] },
    })
    expect(registeredResult.registered).toHaveLength(1)
    expect(registeredResult.newViolations).toEqual([])
    expect(registeredResult.staleExceptions).toEqual([])
    expect(registeredResult.unregisteredUnresolved).toEqual([])
    expect(registeredResult.ok).toBe(true)

    const emptyResult = evaluate({
      violations,
      unresolved: [],
      baseline: { exceptions: [], unresolved: [] },
    })
    expect(emptyResult.newViolations).toHaveLength(1)
    expect(emptyResult.ok).toBe(false)
  })

  it('新违规失败；失效例外同样失败', () => {
    const root = fixture()
    const { violations } = runGuard(root)
    const stale = {
      rule: 'R1',
      path: 'src/core/ui/Removed.ts',
      target: '@/stores/old',
      reason: '夹具：失效例外',
    }
    const result = evaluate({
      violations,
      unresolved: [],
      baseline: { exceptions: [stale], unresolved: [] },
    })
    expect(result.newViolations).toHaveLength(1)
    expect(result.staleExceptions).toHaveLength(1)
    expect(result.ok).toBe(false)
  })

  it('未登记的未解析项失败，登记后通过', () => {
    const root = fixture()
    const { violations, project } = runGuard(root)
    const exception = {
      rule: 'R1',
      path: 'src/core/ui/Widget.ts',
      target: '@/stores/ui',
      reason: '夹具',
    }
    const failing = evaluate({
      violations,
      unresolved: project.unresolved,
      baseline: { exceptions: [exception], unresolved: [] },
    })
    expect(failing.unregisteredUnresolved).toHaveLength(1)
    expect(failing.ok).toBe(false)

    const passing = evaluate({
      violations,
      unresolved: project.unresolved,
      baseline: {
        exceptions: [exception],
        unresolved: [{ path: 'src/features/loader.ts', target: 'name', reason: '夹具' }],
      },
    })
    expect(passing.ok).toBe(true)
    expect(passing.registeredUnresolved).toHaveLength(1)
  })

  it('loadBaseline 校验字段，缺 path/target 或 rule 即报错', () => {
    const root = createProject({
      'bad.json': '{"exceptions":[{"path":"a.ts"}],"unresolved":[]}',
      'good.json': '{"exceptions":[],"unresolved":[]}',
    })
    expect(() => loadBaseline(join(root, 'bad.json'))).toThrow(/缺少 path\/target/)
    expect(loadBaseline(join(root, 'good.json'))).toEqual({ exceptions: [], unresolved: [] })
    expect(() => loadBaseline(join(root, 'missing.json'))).toThrow(/不存在/)
  })
})

describe('CLI 退出码', () => {
  it('新违规 → 1；登记后 → 0；用法/环境错误 → 2', () => {
    const root = createProject({
      'src/stores/ui.ts': 'export const useUiStore = () => ({})\n',
      'src/core/ui/Widget.ts':
        "import { useUiStore } from '@/stores/ui'\nexport const x = useUiStore\n",
      'empty-baseline.json': '{"exceptions":[],"unresolved":[]}',
      'registered-baseline.json':
        '{"exceptions":[{"rule":"R1","path":"src/core/ui/Widget.ts","target":"@/stores/ui","reason":"夹具"}],"unresolved":[]}',
    })
    const output = []
    const io = { log: (text) => output.push(text), errorLog: (text) => output.push(text) }

    expect(main(['--root', root, '--baseline', join(root, 'empty-baseline.json')], io)).toBe(1)
    expect(output.join('\n')).toContain('新违规：1 条')
    expect(output.join('\n')).toContain('规则数：4')

    output.length = 0
    expect(main(['--root', root, '--baseline', join(root, 'registered-baseline.json')], io)).toBe(0)
    expect(output.join('\n')).toContain('结果：通过')

    output.length = 0
    expect(main(['--root', root, '--baseline', join(root, 'missing-baseline.json')], io)).toBe(2)
    expect(output.join('\n')).toContain('环境错误')

    output.length = 0
    expect(main(['--bogus'], io)).toBe(2)
    expect(output.join('\n')).toContain('参数错误')
  })
})
