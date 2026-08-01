/**
 * 字符统计 · 工具注册
 */
import { registerTool } from "@/core/registry/toolRegistry";

registerTool({
  id: "char-stat",
  name: "字符统计",
  category: "text",
  icon: "stat",
  description: "字数、行数、词数、字节数实时统计。",
  keywords: ["字符", "字数", "统计", "count", "字节"],
  presentation: "modal",
  component: () => import("./index.vue"),
  tags: ["文本"],
});
