# patchyBox

patchyBox 是一个基于 [Tauri](https://tauri.app/) 的桌面端工具箱，计划集成各种常用的开发与效率工具。

## 项目规划

- **技术栈**：Tauri 2 + Vue 3 + TypeScript + Tailwind CSS 4（打包体积小、性能好）
- **工具箱形态**：以「工具集合」的方式组织，按类别分组（开发 / 文本 / 图片 / 网络 / 系统）
- **可扩展**：工具数量会持续增加，架构上以「工具注册表」为核心，预留插件化/模块化扩展能力

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
