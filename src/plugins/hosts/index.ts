/**
 * hosts 修改 · 工具注册
 */
import { registerTool } from "@/core/registry/toolRegistry";

registerTool({
  id: "hosts",
  name: "hosts 修改",
  category: "sys",
  icon: "hosts",
  description: "查看与编辑 hosts 文件，语法校验 + 保存前自动备份（UAC 最小授权）。",
  keywords: ["hosts", "域名", "host", "解析", "屏蔽", "本地", "系统文件"],
  presentation: "workspace",
  component: () => import("./index.vue"),
  tags: ["系统"],
});
