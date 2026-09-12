# patchyBox 文档索引

桌面工具箱项目 · 设计方向：**② 明净浅色 · 内容优先**

| 文档 | 内容 |
|------|------|
| [`DESIGN.md`](../DESIGN.md) | 设计 tokens 规范（颜色/字体/圆角/组件，单一事实源） |
| [`01-tech-stack.md`](01-tech-stack.md) | 技术选型分析（Tauri 2.11 + Vue 3 + TS + Tailwind 4） |
| [`02-architecture.md`](02-architecture.md) | 架构：现行架构与依赖方向 + 2026-08 设计期留档 |
| [`03-plugin-development.md`](03-plugin-development.md) | 模块开发规则（owner 边界 / 目录与规模评审信号 / IPC 入库 / 数据库管理 / 质量门槛） |
| [`04-ui-components.md`](04-ui-components.md) | 公共 UI 组件规范 |
| [`05-rust-code-standard.md`](05-rust-code-standard.md) | Rust 代码规范（panic/clone/生命周期/异步/注释/依赖评审） |
| [`06-release.md`](06-release.md) | 发布、自动更新与代码签名配置 |
| [`07-product-requirements.md`](07-product-requirements.md) | 产品需求文档 |

## 原型（已验收）

```
sketches/
├── 001-command-dark/     # 深色命令式（未采纳，留档）
├── 002-clean-light/      # ✅ 采纳方向
└── 003-glass-launcher/   # 玻璃启动器（未采纳，留档）
```

## 快速开始（M0 后生效）

```bash
pnpm install
pnpm tauri dev
```
