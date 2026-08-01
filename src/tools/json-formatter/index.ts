/**
 * JSON 格式化 · 工具注册（自注册：建目录 + 注册一行，框架零改动）
 */
import { registerTool } from "@/core/registry/toolRegistry";
import type { SettingsField } from "@/core/registry/types";

const settingsSchema: SettingsField[] = [
  {
    key: "indent",
    type: "select",
    label: "缩进宽度",
    default: 2,
    options: [
      { label: "2 空格", value: "2" },
      { label: "4 空格", value: "4" },
      { label: "Tab", value: "tab" },
    ],
  },
];

registerTool({
  id: "json-formatter",
  name: "JSON 格式化",
  category: "dev",
  icon: "json",
  description: "格式化、校验与压缩 JSON，错误自动定位行号。",
  keywords: ["json", "格式化", "格式化", "压缩", "校验", "pretty"],
  presentation: "modal",
  component: () => import("./index.vue"),
  settingsSchema,
  tags: ["开发", "热门"],
});
