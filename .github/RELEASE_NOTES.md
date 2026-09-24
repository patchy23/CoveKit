# CoveKit Alpha

首个公开测试版本，提供 SSH 远程管理、HTTP/WebSocket 调试、数据库工作台、DNS 与 Hosts、格式转换等桌面工具。

## 下载与安装

| 你的设备 | 下载选择 |
| --- | --- |
| Windows 64 位电脑 | `.exe` 安装包 |
| Mac，Apple Silicon 芯片 | 文件名含 `aarch64` 的 `.dmg` |
| Mac，Intel 处理器 | 文件名含 `x86_64` 的 `.dmg` |

Windows 下载后运行安装程序；macOS 打开 DMG 后将 CoveKit 拖入“应用程序”。附件 `SHA256SUMS.txt` 用于核对下载文件完整性。

Windows 安装包尚未配置发布者签名，可能出现未知发布者提示。macOS 版本已经人工验证，但尚未通过 Apple 公证，首次打开可能被系统阻止；确认下载来源后，可按“系统设置 → 隐私与安全性”的提示允许打开。

Alpha 不提供应用内自动更新，请通过发布页手动下载后续版本。首次使用或更换版本前请备份重要数据。

## 已知限制

- 导入引用凭证库 Token 的 FRP 配置时，如果数据包未包含对应凭证，导入会被拒绝。
- 文件占用和端口占用查询目前仅支持 Windows。

## 问题反馈

遇到问题请通过 [GitHub Issues](https://github.com/patchy23/CoveKit/issues) 反馈，并附系统版本、芯片架构、应用版本与复现步骤；日志和截图中请移除服务器凭据及业务内容。
