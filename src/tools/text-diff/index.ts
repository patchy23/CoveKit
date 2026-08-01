/**
 * 文本对比 · 工具注册
 */
import { registerTool } from "@/core/registry/toolRegistry";

registerTool({
  id: "text-diff",
  name: "文本对比",
  category: "text",
  icon: "diff",
  description: "左右分栏 Diff，逐行高亮增删改。",
  keywords: ["diff", "对比", "比较", "差异"],
  presentation: "workspace",
  component: () => import("./index.vue"),
  tags: ["文本"],
});
