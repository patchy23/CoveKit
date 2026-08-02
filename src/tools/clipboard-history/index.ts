/**
 * 剪贴板历史 · 工具注册
 */
import { registerTool } from "@/core/registry/toolRegistry";

registerTool({
  id: "clipboard-history",
  name: "剪贴板历史",
  category: "sys",
  icon: "clipboard",
  description: "最近复制的文本历史，支持搜索、置顶与一键复制。",
  keywords: ["剪贴板", "clipboard", "历史", "复制记录", "粘贴板"],
  presentation: "workspace",
  component: () => import("./index.vue"),
  tags: ["系统"],
});
