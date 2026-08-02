/**
 * 二维码 · 工具注册
 */
import { registerTool } from "@/core/registry/toolRegistry";

registerTool({
  id: "qrcode",
  name: "二维码",
  category: "image",
  icon: "qr",
  description: "文本/链接生成二维码，容错自适应，下载 PNG。",
  keywords: ["二维码", "qr", "qrcode", "生成", "扫码"],
  presentation: "workspace",
  component: () => import("./index.vue"),
  tags: ["图片"],
});
