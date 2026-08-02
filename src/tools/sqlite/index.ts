/**
 * SQLite 数据库 · 工具注册
 */
import { registerTool } from "@/core/registry/toolRegistry";

registerTool({
  id: "sqlite",
  name: "SQLite 数据库",
  category: "dev",
  icon: "db",
  description: "打开 SQLite 文件，浏览表结构与数据，执行 SQL 查看结果。",
  keywords: ["sqlite", "sql", "数据库", "db", "表", "查询", "执行"],
  presentation: "workspace",
  component: () => import("./index.vue"),
  tags: ["开发", "热门"],
});
