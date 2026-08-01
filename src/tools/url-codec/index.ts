/**
 * URL 编解码 · 工具注册
 */
import { registerTool } from "@/core/registry/toolRegistry";

registerTool({
  id: "url-codec",
  name: "URL 编解码",
  category: "dev",
  icon: "url",
  description: "URL 编码与解码，支持中文与特殊字符。",
  keywords: ["url", "编码", "解码", "encode", "decode"],
  presentation: "modal",
  component: () => import("./index.vue"),
  tags: ["开发"],
});
