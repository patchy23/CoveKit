/**
 * 时间戳转换 · 工具注册
 */
import { registerTool } from "@/core/registry/toolRegistry";

registerTool({
  id: "ts-converter",
  name: "时间戳转换",
  category: "dev",
  icon: "ts",
  description: "Unix 时间戳与日期互转，支持毫秒精度与相对时间。",
  keywords: ["时间戳", "timestamp", "unix", "日期", "转换"],
  presentation: "workspace",
  component: () => import("./index.vue"),
  tags: ["开发"],
});
