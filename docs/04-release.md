# M4 发布与签名

## 发布模式

普通开发和 CI 构建继续读取 `src-tauri/tauri.conf.json`，不要求更新签名私钥。推送 `v*` 标签后，Release 工作流会生成一份不入库的 `src-tauri/tauri.release.conf.json`，打包 Windows NSIS、macOS DMG 和带签名的更新产物，然后创建 GitHub 草稿版本。

更新器固定读取：

`https://github.com/patchy23/patchyBox/releases/latest/download/latest.json`

## GitHub 配置

必需的 Actions 配置：

- Repository variable `TAURI_UPDATER_PUBLIC_KEY`：Tauri 更新签名公钥。
- Secret `TAURI_SIGNING_PRIVATE_KEY`：Tauri 更新签名私钥，绝不入库。
- Secret `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`：私钥口令；无口令时留空。

macOS 签名和公证另需 `APPLE_CERTIFICATE`、`APPLE_CERTIFICATE_PASSWORD`、`APPLE_SIGNING_IDENTITY`、`APPLE_ID`、`APPLE_PASSWORD`、`APPLE_TEAM_ID`。Windows Authenticode 需另行配置可用的代码签名证书；更新签名不等同于操作系统代码签名。

## 密钥和安全

首次发布前按 Tauri 官方流程离线生成更新密钥对，将私钥放入 GitHub Actions secret，并在安全位置备份。丢失私钥后，已安装的应用无法验证新更新。不要在本项目、日志、聊天或发布产物中传播私钥。

## 发布步骤

1. 确认 `package.json` 和 `src-tauri/tauri.conf.json` 版本一致。
2. 完成全量质量门槛，更新 `CHANGELOG.md`。
3. 创建并推送同版本标签，例如 `v0.1.0`。
4. 检查 Release 工作流产物、`latest.json` 及签名，在实机验证升级。
5. 确认 Windows/macOS 代码签名和 macOS 公证后再发布草稿。
