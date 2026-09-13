/**
 * 工具注册表 · 数据模型
 * 与 docs/standards/02-架构.md §3 同步。
 */
import type { Component } from 'vue'

/** 工具分类（分类是数据不是枚举：新增分类 = 扩展联合 + 图标表） */
export type CategoryId = 'dev' | 'text' | 'image' | 'net' | 'sys'

/** 工具级设置声明式 schema：框架自动渲染设置表单并存 settings.tools[id] */
export interface SettingsField {
  key: string
  type: 'toggle' | 'text' | 'number' | 'select' | 'secret'
  label: string
  default?: unknown
  options?: { label: string; value: string }[]
}

/** 工具清单：工具目录自注册，新增工具 = 建目录 + registerTool 一行 */
export interface ToolManifest {
  id: string
  name: string
  category: CategoryId
  /** 图标 key（全局图标表索引，见 features/ui/AppIcon.vue） */
  icon: string
  description: string
  /** 搜索同义词 */
  keywords: string[]
  hotkey?: string
  /** 懒加载工厂：dynamic import 工具组件 */
  component: () => Promise<{ default: Component }>
  /** 后台服务型工具（剪贴板/番茄钟），第二批用 */
  background?: boolean
  settingsSchema?: SettingsField[]
  tags?: string[]
}

/** 应用设置（与 core/ipc/contracts.ts AppSettings 一致；此类型供前端各层引用） */
export interface AppSettings {
  theme: 'light' | 'dark' | 'system'
  language: 'zh-CN' | 'en-US'
  globalHotkey: string
  launchAtStartup: boolean
  defaultDownloadDirectory: string
  recentTools: string[]
  tools: Record<string, Record<string, unknown>>
}
