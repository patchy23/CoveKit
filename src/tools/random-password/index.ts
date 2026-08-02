/**
 * 随机密码 · 工具注册
 */
import { registerTool } from "@/core/registry/toolRegistry";

registerTool({
  id: "random-password",
  name: "随机密码",
  category: "sys",
  icon: "lock",
  description: "安全随机密码生成，支持字符集与易混字符排除。",
  keywords: ["密码", "随机", "password", "生成", "安全"],
  presentation: "workspace",
  component: () => import("./index.vue"),
  tags: ["系统"],
});
