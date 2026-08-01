# patchyBox

patchyBox 是一个基于 [Tauri](https://tauri.app/) 的桌面端工具箱，计划集成各种常用的开发与效率工具。

## 项目规划

- **技术栈**：Tauri 2 + Vue 3 + TypeScript + Tailwind CSS 4（打包体积小、性能好）
- **第一批次**：整体 UI + 后端框架 + 8 个常用文本小工具（JSON 格式化、时间戳、Base64、URL 编解码、字符统计、文本对比、Markdown 预览、正则测试）
- **第二批次**：复杂工具——HTTP/WS 调试、数据库（MySQL/PG/SQLite）、本地 hosts 修改、DNS 管理（阿里/腾讯/CF）、SSH 工具
- **可扩展**：以「工具注册表」为核心架构，新增工具即插即用；第二批的工具形态（工作区、长连接、凭据、提权）在第一批框架中预留抽象

## 设计文档

| 文档 | 说明 |
|------|------|
| [DESIGN.md](DESIGN.md) | 设计规范（颜色 / 字体 / 组件 tokens，单一事实源） |
| [docs/01-tech-stack.md](docs/01-tech-stack.md) | 技术选型分析 |
| [docs/02-architecture.md](docs/02-architecture.md) | 详细设计（架构 / IPC 契约 / 数据模型 / 路线图） |
| [sketches/002-clean-light/](sketches/002-clean-light/index.html) | 已采纳 UI 方向的交互原型（浏览器直接打开） |

## 开发

> 状态：设计阶段已完成，等待 M0 脚手架初始化（见 `AGENTS.md`）。

```bash
pnpm install
pnpm tauri dev
```
