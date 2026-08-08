/**
 * DNS 工具 · 工具注册
 */
import { registerTool } from "@/core/registry/toolRegistry";

registerTool({
  id: "dns",
  name: "DNS 工具",
  category: "net",
  icon: "dns",
  description:
    "DNS 查询（多类型、多服务器对比）+ 阿里云/DNSPod 云解析记录管理（增删改查）。",
  keywords: [
    "dns",
    "域名",
    "解析",
    "dig",
    "nslookup",
    "查询",
    "A记录",
    "CNAME",
    "MX",
    "TXT",
    "阿里云",
    "dnspod",
    "腾讯云",
    "云解析",
  ],
  presentation: "workspace",
  component: () => import("./index.vue"),
  tags: ["网络"],
});
