# CoveKit

CoveKit 是一个基于 [Tauri](https://tauri.app/) 的桌面端工具箱，集成常用的开发、调试与效率工具。应用标识为 `com.patchyx.covekit`，工程包名为 `covekit`。

## 项目规划

- **技术栈**：Tauri 2 + Vue 3 + TypeScript + Tailwind CSS 4（打包体积小、性能好）
- **第一批次**：整体 UI + 后端框架 + 8 个常用文本小工具（JSON 格式化、时间戳、Base64、URL 编解码、字符统计、文本对比、Markdown 预览、正则测试）
- **第二批次**：复杂工具——HTTP/WS 调试、数据库（MySQL/PG/SQLite）、本地 hosts 修改、DNS 管理（阿里/腾讯/CF）、SSH 工具
- **可扩展**：以「工具注册表」为核心架构，新增工具即插即用；第二批的工具形态（工作区、长连接、凭据、提权）在第一批框架中预留抽象

## 设计文档

| 文档                                                             | 说明                                              |
| ---------------------------------------------------------------- | ------------------------------------------------- |
| [DESIGN.md](DESIGN.md)                                           | 设计规范（颜色 / 字体 / 组件 tokens，单一事实源） |
| [docs/standards/01-技术选型.md](docs/standards/01-技术选型.md)                   | 技术选型分析                                      |
| [docs/standards/02-架构.md](docs/standards/02-架构.md)               | 详细设计（架构 / IPC 契约 / 数据模型 / 路线图）   |
| [docs/standards/06-发布与更新.md](docs/standards/06-发布与更新.md)                         | 发布、自动更新与代码签名配置                 |

## 当前进度

截至 2026-08-24，项目已完成 **M4 代码与 Windows 本地 RC 验证**：

- M0–M2 已完成，HTTP/WS、hosts、格式转换等工具可用。
- 数据库工作台已支持 MySQL、PostgreSQL、SQLite、Redis、Oracle、PolarDB、Vastbase、Kingbase；达梦入口暂未实现后端驱动。
- DNS 已支持查询及阿里云、腾讯云 DNSPod、Cloudflare 解析管理，三平台均支持 Vault 或手工凭据。
- SSH 已支持终端、SFTP、远程编辑、资源监控、进程、systemd 服务和 Docker 管理。
- 框架级 Vault 已采用系统 keyring + AES-256-GCM 落地；SSH、DNS 可引用 Vault 凭证并保留手工输入方式（数据库暂为插件私有加密存储）。
- 中英文核心界面可在运行时切换；已接入 Tauri 自动更新与 GitHub Release 工作流，实际签名发布需在 GitHub 配置私钥和平台证书。
- Windows x64 NSIS 生产包已在本地构建通过；发布候选版验证记录见 `docs/releases/v0.1.0-发布候选.md`。

## 开发

```bash
pnpm install
pnpm tauri dev
```

## 打包

执行 `pnpm build` 构建当前平台的桌面应用。Windows 默认生成：

- 应用程序：`src-tauri/target/release/covekit.exe`。
- EXE 安装包：`src-tauri/target/release/bundle/nsis/*-setup.exe`。

两份文件来自同一次构建；直接运行应用程序仍需要系统具备 WebView2。macOS 默认生成 DMG。仅构建前端时使用 `pnpm build:web`；Tauri 的前端构建钩子也使用该命令，避免递归调用桌面打包。
