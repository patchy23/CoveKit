# CoveKit 首个公开测试版

CoveKit 是面向 Windows 和 macOS 的开源桌面工具箱，把远程管理、接口调试、数据库操作和常用开发工具放在同一个工作区。

这是 CoveKit 的首个 Alpha 版本，欢迎试用并反馈问题。

## 主要功能

- **SSH 远程管理**：终端、SFTP 文件传输、远程文件编辑、隧道、服务器监控，以及 Docker 和 Compose 管理。
- **接口调试**：HTTP、SSE 和 WebSocket 请求，多页签与接口分组。
- **数据库工作台**：连接管理、SQL 编辑与执行、表数据浏览和编辑、CSV 导入导出。
- **网络与系统工具**：DNS 查询与解析管理、Hosts 编辑、FRP 客户端管理；Windows 另提供文件占用和端口占用查询。
- **日常开发工具**：JSON/XML 格式化、时间戳与 Base64 转换、文字转语音。

## 下载选择

| 你的设备 | 下载选择 |
| --- | --- |
| Windows 64 位电脑，免安装运行 | `CoveKit_版本号.exe` |
| Windows 64 位电脑，安装到系统 | `CoveKit_版本号_setup.exe` |
| Mac，Apple Silicon 芯片 | 文件名含 `aarch64` 的 `.dmg` |
| Mac，Intel 处理器 | 文件名含 `x86_64` 的 `.dmg` |

Windows 免安装版下载后直接运行，安装版运行 setup 程序；macOS 打开 DMG 后将 CoveKit 拖入“应用程序”。附件 `SHA256SUMS.txt` 用于核对下载文件完整性。

## 使用提示

- Windows 免安装版依赖 Microsoft Edge WebView2 Runtime；数据保存在应用数据目录，不随 exe 文件搬移。缺少运行环境时可使用安装版。
- Windows 程序尚未配置发布者签名，可能出现未知发布者提示。macOS 尚未通过 Apple 公证，首次打开若被阻止，可按“系统设置 → 隐私与安全性”的提示处理。
- Alpha 暂不提供应用内更新，后续版本请从发布页手动下载。
- 当前导入引用凭证库 Token 的 FRP 配置时，若数据包缺少对应凭证，导入会被拒绝。

## 问题反馈

遇到问题请通过 [GitHub Issues](https://github.com/patchy23/CoveKit/issues) 反馈，并附系统版本、芯片架构、应用版本与复现步骤；日志和截图中请移除服务器凭据及业务内容。
