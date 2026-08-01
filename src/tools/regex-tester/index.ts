/**
 * 正则测试 · 工具注册
 */
import { registerTool } from "@/core/registry/toolRegistry";

registerTool({
  id: "regex-tester",
  name: "正则测试",
  category: "dev",
  icon: "regex",
  description: "实时匹配高亮，常用表达式一键插入。",
  keywords: ["正则", "regex", "匹配", "pattern", "表达式"],
  presentation: "modal",
  component: () => import("./index.vue"),
  tags: ["开发"],
});
