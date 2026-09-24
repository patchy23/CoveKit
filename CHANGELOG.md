# Changelog

本项目遵循 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/) 格式，版本号遵循语义化版本。

## [Unreleased]

### 新增

- FRP 客户端管理、Windows 文件占用与端口占用查询、Codex 重置消息工具。
- SSH 服务器分组与 CSV 批量管理、可取消的文件传输队列、远程归档、Compose 管理和可移入独立系统窗口的多文档编辑器。
- HTTP、SSE、WebSocket 独立请求页签与接口分组管理。
- 数据库查询文档、独立事务、单表编辑、CSV 导入及完整只读导出。
- 本地数据空间、选择性导入导出与应用资源监测。

### 变更

- SSH 手工认证支持本地明文保存、不保存和凭证库三种方式；新建手工认证默认本地保存。
- 移除全局唤起快捷键，格式转换采用自动转换交互。
- 项目采用 MIT 许可证，更新项目介绍、开发文档与产品展示。
- 构建目标限定为 Windows NSIS 和 macOS DMG，Rust 检查使用 Windows 环境。

未发布变更的功能与平台验证状态见[平台能力矩阵](docs/standards/09-平台能力矩阵.md)。以下版本条目保留对应时期的历史记录，不代表当前版本的验证结论。

## [0.1.0] - 2026-08-24

### 新增

- 完整的桌面工具箱框架：工具注册表、多页签工作区、搜索、收藏、设置、托盘和全局快捷键。
- HTTP/WebSocket 调试、hosts 编辑、格式转换（JSON/XML/时间戳/Base64）、文字转语音等常用工具。
- MySQL、PostgreSQL、SQLite、Redis、Oracle 及多种兼容数据库的多连接工作台。
- DNS 查询和阿里云、腾讯云 DNSPod、Cloudflare 解析管理。
- SSH 终端、SFTP、远程编辑、资源监控、进程、systemd 和 Docker 管理。
- 系统 keyring 主密钥与 AES-256-GCM 加密的公共 Vault；SSH/DNS 可引用 Vault 凭证并保留手工输入，支持删除保护和加密备份。
- 核心界面中英文运行时切换。
- Tauri 自动更新、GitHub 标签发布和更新产物签名流程。

### 安全

- SSH 主机密钥校验，DNS/SSH 同时支持 Vault 与原手工凭证输入。
- hosts 修改采用按需 UAC 最小提权，应用本体不常驻管理员权限。
- Tauri 命令与插件能力经最小权限白名单暴露。

[0.1.0]: https://github.com/patchy23/CoveKit/releases/tag/v0.1.0
