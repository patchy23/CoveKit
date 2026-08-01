/**
 * Markdown 预览 · 工具注册
 */
import { registerTool } from "@/core/registry/toolRegistry";

registerTool({
  id: "markdown-preview",
  name: "Markdown 预览",
  category: "text",
  icon: "md",
  description: "实时渲染 GFM，支持代码块与表格。",
  keywords: ["markdown", "md", "预览", "渲染"],
  presentation: "workspace",
  component: () => import("./index.vue"),
  tags: ["文本", "热门"],
});
