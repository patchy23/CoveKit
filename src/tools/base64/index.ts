/**
 * Base64 编解码 · 工具注册
 */
import { registerTool } from "@/core/registry/toolRegistry";

registerTool({
  id: "base64",
  name: "Base64 编解码",
  category: "dev",
  icon: "b64",
  description: "文本 Base64 互转，完整支持中文 Unicode。",
  keywords: ["base64", "编码", "解码", "b64"],
  presentation: "modal",
  component: () => import("./index.vue"),
  tags: ["开发"],
});
