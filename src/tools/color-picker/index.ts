/**
 * 颜色选择器 · 工具注册
 */
import { registerTool } from "@/core/registry/toolRegistry";

registerTool({
  id: "color-picker",
  name: "颜色选择器",
  category: "image",
  icon: "color",
  description: "HEX/RGB/HSL 互转、随机色、屏幕取色与对比度检查。",
  keywords: ["颜色", "color", "取色", "hex", "rgb", "hsl", "调色板"],
  presentation: "workspace",
  component: () => import("./index.vue"),
  tags: ["图片"],
});
