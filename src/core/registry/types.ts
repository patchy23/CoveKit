/**
 * 工具注册表 · 数据模型
 * 与 docs/02-architecture.md §3 同步；第二批预留类型（ConnectionProfile）一并定义。
 */
import type { Component } from "vue";

/** 工具分类（分类是数据不是枚举：新增分类 = 扩展联合 + 图标表） */
export type CategoryId = "dev" | "text" | "image" | "net" | "sys";

/** 展示载体：workspace=多页签工作区（2026-08-02 起第一批工具全用，子页面形态）；modal=轻量弹窗（备用） */
export type Presentation = "workspace" | "modal";

/** 工具级设置声明式 schema：框架自动渲染设置表单并存 settings.tools[id] */
export interface SettingsField {
  key: string;
  type: "toggle" | "text" | "number" | "select" | "secret";
  label: string;
  default?: unknown;
  options?: { label: string; value: string }[];
}

/** 工具清单：工具目录自注册，新增工具 = 建目录 + registerTool 一行 */
export interface ToolManifest {
  id: string;
  name: string;
  category: CategoryId;
  /** 图标 key（全局图标表索引，见 features/ui/AppIcon.vue） */
  icon: string;
  description: string;
  /** 搜索同义词 */
  keywords: string[];
  hotkey?: string;
  /** 默认 'modal'；第二批大工具声明 'workspace' */
  presentation: Presentation;
  /** 懒加载工厂：dynamic import 工具组件 */
  component: () => Promise<{ default: Component }>;
  /** 后台服务型工具（剪贴板/番茄钟），第二批用 */
  background?: boolean;
  settingsSchema?: SettingsField[];
  tags?: string[];
}

/** 应用设置（与 core/ipc/contracts.ts AppSettings 一致；此类型供前端各层引用） */
export interface AppSettings {
  theme: "light" | "dark" | "system";
  language: "zh-CN" | "en-US";
  globalHotkey: string;
  launchAtStartup: boolean;
  clipboard: { enabled: boolean; historyLimit: number; ignore: string[] };
  recentTools: string[];
  tools: Record<string, Record<string, unknown>>;
}

/** 第二批预留：连接型工具（DB/SSH）的统一连接配置 */
export interface ConnectionProfile {
  id: string;
  name: string;
  kind: "mysql" | "postgres" | "sqlite" | "ssh";
  host?: string;
  port?: number;
  user?: string;
  /** 密码/密钥引用（存 stronghold，不落明文） */
  secretRef?: string;
  options?: Record<string, string>;
}

/** 剪贴板记录（与 Rust modules/clipboard.rs serde 同步） */
export interface ClipboardRecord {
  id: string;
  kind: "text" | "image" | "file";
  content: string;
  preview: string;
  pinned: boolean;
  createdAt: number;
}
