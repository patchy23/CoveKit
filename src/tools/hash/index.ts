/**
 * 哈希计算 · 工具注册
 */
import { registerTool } from "@/core/registry/toolRegistry";

registerTool({
  id: "hash",
  name: "哈希计算",
  category: "dev",
  icon: "hash",
  description: "MD5 / SHA-1 / SHA-256 / SHA-384 / SHA-512 实时计算。",
  keywords: ["哈希", "hash", "md5", "sha", "摘要", "校验"],
  presentation: "workspace",
  component: () => import("./index.vue"),
  tags: ["开发"],
});
