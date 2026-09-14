/**
 * 工具注册表 · 数据模型
 * 与 docs/standards/02-架构.md §3 同步。
 */
import type { Component } from 'vue'
import type { SettingsField } from '@/core/ipc/contracts'

/** 工具分类（分类是数据不是枚举：新增分类 = 扩展联合 + 图标表） */
export type CategoryId = 'dev' | 'text' | 'image' | 'net' | 'sys'

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
  /** 懒加载工厂：dynamic import 工具组件 */
  component: () => Promise<{ default: Component }>
  /** 后台服务型工具（剪贴板/番茄钟），第二批用 */
  background?: boolean
  settingsSchema?: SettingsField[]
  tags?: string[]
}

/**
 * 设置类型直接从 IPC 契约再导出：契约文件是 AppSettings 与 SettingsField 的唯一事实源，
 * 此处只做转发，避免出现两份定义各自漂移。
 */
export type { AppSettings, SettingsField } from '@/core/ipc/contracts'
