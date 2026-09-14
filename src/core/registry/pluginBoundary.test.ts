import { describe, expect, it } from 'vitest'

/**
 * 架构守卫 · 前端插件边界（可靠性 T13-5）
 *
 * 口径：`src/plugins/<owner>/` 下的业务插件之间不得互相 import，共享能力只能走 `@/core/*`
 * （引导文件 `src/plugins/index.ts` 不在 owner 目录内，不参与判定）。
 * 判定在纯函数 `findCrossPluginImports` 上做，真实目录与注入夹具共用同一条代码路径。
 */

/** 读取全部插件源码（含 .vue 模板内的 import 说明符） */
const pluginSources = import.meta.glob('/src/plugins/**/*.{ts,vue}', {
  eager: true,
  query: '?raw',
  import: 'default',
}) as Record<string, string>

/** 取 owner 目录名：`/src/plugins/ssh/index.ts` → `ssh`；不在 owner 目录内返回 null */
function ownerOf(path: string): string | null {
  const match = /^\/src\/plugins\/([^/]+)\//.exec(path)
  return match ? match[1] : null
}

/** 取源码里的模块说明符（import / export ... from / 动态 import()） */
function specifiers(source: string): string[] {
  const found: string[] = []
  const pattern = /(?:from\s*|import\s*\(\s*)['"]([^'"]+)['"]/g
  let match: RegExpExecArray | null
  while ((match = pattern.exec(source)) !== null) {
    found.push(match[1])
  }
  return found
}

/** 把说明符解析成 src 下的绝对路径（`@/` 别名与相对路径；裸包名原样返回） */
function resolveSpec(file: string, spec: string): string {
  if (spec.startsWith('@/')) {
    return `/src/${spec.slice(2)}`
  }
  if (!spec.startsWith('.')) {
    return spec
  }
  const parts = file.split('/').slice(0, -1)
  for (const segment of spec.split('/')) {
    if (segment === '' || segment === '.') {
      continue
    }
    if (segment === '..') {
      parts.pop()
    } else {
      parts.push(segment)
    }
  }
  return parts.join('/')
}

/** 找出跨插件 import（返回可读的违规清单，空数组 = 通过） */
function findCrossPluginImports(files: Record<string, string>): string[] {
  const violations: string[] = []
  for (const [file, source] of Object.entries(files)) {
    const mine = ownerOf(file)
    if (!mine) {
      continue
    }
    for (const spec of specifiers(source)) {
      const other = ownerOf(resolveSpec(file, spec))
      if (other && other !== mine) {
        violations.push(`${file} → ${spec}`)
      }
    }
  }
  return violations.sort()
}

describe('架构守卫 · 插件边界', () => {
  it('业务插件之间不得互相 import', () => {
    expect(findCrossPluginImports(pluginSources)).toEqual([])
  })

  it('scanned 范围非空（防止 glob 失效后空扫描被当作通过）', () => {
    expect(Object.keys(pluginSources).length).toBeGreaterThan(20)
  })

  it('注入一处跨插件 import 时守卫必须失败', () => {
    const injected = {
      '/src/plugins/ssh/index.ts': "import { resolveDns } from '@/plugins/dns/api'\n",
    }
    expect(findCrossPluginImports(injected)).toEqual([
      '/src/plugins/ssh/index.ts → @/plugins/dns/api',
    ])
  })

  it('相对路径与动态 import 的跨插件引用同样被抓出', () => {
    const injected = {
      '/src/plugins/frp/runtime/useFrpRuntime.ts':
        "export async function x() {\n  const m = await import('../../database/store')\n  return m\n}\n",
    }
    expect(findCrossPluginImports(injected)).toHaveLength(1)
  })

  it('经 @/core 共享能力的 import 不算违规', () => {
    const shared = {
      '/src/plugins/ssh/index.ts':
        "import { UiButton } from '@/core/ui'\nimport { ipc } from '@/core/ipc'\n",
    }
    expect(findCrossPluginImports(shared)).toEqual([])
  })
})
