/**
 * SSH 工具 · 工具注册
 */
import { registerTool } from "@/core/registry/toolRegistry";

registerTool({
  id: "ssh",
  name: "SSH 工具",
  category: "net",
  icon: "net",
  description: "SSH 远程服务器管理：终端 / 文件 / 监控 / 服务 / 进程 / Docker。",
  keywords: ["ssh", "terminal", "sftp", "远程", "服务器", "linux", "终端"],
  presentation: "workspace",
  component: () => import("./index.vue"),
  tags: ["网络", "热门"],
});
