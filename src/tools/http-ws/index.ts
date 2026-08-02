/**
 * HTTP/WS 调试 · 工具注册
 */
import { registerTool } from "@/core/registry/toolRegistry";

registerTool({
  id: "http-ws",
  name: "HTTP/WS 调试",
  category: "net",
  icon: "net",
  description: "HTTP 请求构建与 WebSocket 长连接测试。",
  keywords: ["http", "https", "请求", "api", "websocket", "ws", "调试", "postman"],
  presentation: "workspace",
  component: () => import("./index.vue"),
  tags: ["网络", "热门"],
});
