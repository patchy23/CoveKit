/**
 * 工具注册表行为网（T12-2）
 *
 * 用途：确认注册错误在**启动期**就明确失败，而不是 warn 后覆盖；
 * 并证明注册表实例之间互相隔离（同一进程里多次初始化不会污染）。
 */
import { describe, expect, it } from 'vitest'
import { createToolRegistry } from './toolRegistry'
import type { ToolManifest } from './types'

/** 最小可用清单 */
function manifest(overrides: Partial<ToolManifest> = {}): ToolManifest {
  return {
    id: 'demo',
    name: '演示工具',
    category: 'dev',
    icon: 'json',
    description: '测试用',
    keywords: ['demo'],
    component: () => Promise.resolve({ default: { render: () => null } as never }),
    ...overrides,
  }
}

describe('工具注册表', () => {
  it('重复 id 注册立刻抛错并指出冲突双方', () => {
    const registry = createToolRegistry()
    registry.register(manifest({ id: 'a', name: 'A 工具' }))
    expect(() => registry.register(manifest({ id: 'a', name: 'B 工具' }))).toThrow(/重复注册: a/)
  })

  it('非法分类立刻抛错', () => {
    const registry = createToolRegistry()
    expect(() => registry.register(manifest({ category: 'unknown' as never }))).toThrow(/分类非法/)
  })

  it('工具内设置键重复立刻抛错', () => {
    const registry = createToolRegistry()
    expect(() =>
      registry.register(
        manifest({
          settingsSchema: [
            { key: 'indent', type: 'text', label: '缩进' },
            { key: 'indent', type: 'text', label: '缩进（重复）' },
          ],
        })
      )
    ).toThrow(/设置键重复: indent/)
  })

  it('select 字段缺选项立刻抛错', () => {
    const registry = createToolRegistry()
    expect(() =>
      registry.register(
        manifest({ settingsSchema: [{ key: 'mode', type: 'select', label: '模式', options: [] }] })
      )
    ).toThrow(/没有选项/)
  })

  it('两个实例互不干扰（测试与嵌入式场景隔离）', () => {
    const first = createToolRegistry()
    const second = createToolRegistry()
    first.register(manifest({ id: 'only-first' }))
    expect(first.get('only-first')).toBeDefined()
    expect(second.get('only-first')).toBeUndefined()
    // 第二个实例可以用同样的 id，不受第一个实例影响
    expect(() => second.register(manifest({ id: 'only-first' }))).not.toThrow()
  })

  it('分类计数只统计本实例', () => {
    const registry = createToolRegistry()
    registry.register(manifest({ id: 'a', category: 'dev' }))
    registry.register(manifest({ id: 'b', category: 'text' }))
    expect(registry.categoryCounts()).toEqual({ dev: 1, text: 1 })
  })
})
