/**
 * SSH 工具 · 工具注册
 */
import { registerTool } from "@/core/registry/toolRegistry";

registerTool({
  id: "ssh",
  name: "SSH 远程管理",
  category: "net",
  icon: "net",
  description: "SSH 远程运维一体化：终端、文件传输、监控、服务与 Docker 管理。",
  keywords: ["ssh", "terminal", "sftp", "远程", "服务器", "linux", "终端"],
  presentation: "workspace",
  component: () => import("./index.vue"),
  tags: ["网络", "热门"],
});
