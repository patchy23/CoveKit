# P7a 平台启动基线记录（Windows · 2026-09-13）

> 依据 [`剩余项决策书.md`](剩余项决策书.md) §7「L0 冻结后先做 P7a 可用平台基线」。
> 本文件只记录**实测到的事实**：命令、退出码、日志片段、窗口证据与缺项。缺项按决策书口径标「环境待补」，不得用单测或构建代替。

## 1. 记录时点与代码状态

| 项 | 值 |
| --- | --- |
| 记录时间 | 2026-09-13 03:15–03:35（UTC+08:00） |
| 仓库 | `G:\workspace\patchyBox`，分支 `main` |
| HEAD | `f4433b9`（L0 数据边界冻结），工作区仅 `?? branding/` 未跟踪 |
| 构建来源 | 当前代码新构建（`pnpm tauri dev` 路径），非旧 exe 存活 |
| 冒烟前置 | 见 §2 |

## 2. 冒烟前处置（须记录）

首次冷启动失败：`Error: Port 1420 is already in use`。定位到残留 vite 开发服务器 **PID 66092**（启动于 2026-09-12 04:07，命令行指向本仓库 `node_modules/.pnpm/vite@6.4.3`），即上一次会话遗留、已运行约 23 小时。终止该进程并确认端口释放（`Get-NetTCPConnection -LocalPort 1420 -State Listen` 计数 0）后重新冷启动。

结论：首次尝试的失败**不是代码缺陷**，属环境残留；本条记录以免后续误判。

另一个同源注意：dev 实例运行期间 `patchybox.exe` 被占用，`python scripts/check_docs.py`（内部先 cargo 编译）会报 `failed to remove file … 拒绝访问 (os error 5)`、`cargo 退出码 101`。记录结束已终止实例，终止后该检查恢复通过（`✅ 检查通过（入口 scan_docs，实测运行 1 个测试）`）。

## 3. 冷启动实测

| 项 | 实测值 |
| --- | --- |
| 命令 | `pnpm tauri dev`（BeforeDevCommand `pnpm dev`，DevCommand `cargo run --no-default-features --color always --`） |
| Vite | v6.4.3，`ready in 606 ms`，`Local: http://localhost:1420/` |
| Rust 编译 | `Finished \`dev\` profile … in 35.23s`（增量，无 warning 输出） |
| 进程 | `target\debug\patchybox.exe`，PID 97548，常驻内存约 77 MB |
| 启动日志 | 全文 12 行，`error`/`panic`/`unused` 命中数 **0** |
| 存活 | 记录结束时进程仍在（未崩溃、未重启） |

日志全文（去回车）：

```
$ tauri "dev"
     Running BeforeDevCommand (`pnpm dev`)
$ vite
  VITE v6.4.3  ready in 606 ms
  ➜  Local:   http://localhost:1420/
     Running DevCommand (`cargo  run --no-default-features --color always --`)
        Info Watching G:\workspace\patchyBox\src-tauri for changes...
   Compiling patchybox v0.1.0 (G:\workspace\patchyBox\src-tauri)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 35.23s
     Running `target\debug\patchybox.exe`
```

## 4. 窗口与界面证据

| 项 | 实测值 |
| --- | --- |
| 窗口列表 | `patchybox.exe` PID 97548 有两个窗口：标题 **CoveKit**（HWND 2100850 / 0x200e72）与单实例窗口 `com.patchy23.patchybox-siw` |
| 标题栏 | 显示 **CoveKit**（与 2026-09-11 的应用显示名一致；进程名仍 `patchybox.exe`） |
| 可访问性树 | 73 个可交互元素：搜索框「搜索工具…」、侧栏分类（全部工具 9 / 开发工具 3 / 文本处理 1 / 图片工具 0 / 网络工具 4 / 系统工具 1 / 我的收藏 1）、9 张工具卡、卡片/列表视图切换、底部「浅色模式」「设置」、关闭按钮标注「关闭（最小化到托盘）」 |
| 渲染内容 | 工具卡与描述文字全部渲染（格式转换、接口调试、数据库、Hosts 编辑、DNS 解析、SSH 远程管理、FRP 客户端、文字转语音、组件实验室），收藏星标状态正确（SSH 为已收藏 ★） |
| 截图 | `%LOCALAPPDATA%\hermes\cache\images\computer_use_aad41e954d2d423dbedb7418a9f9b5d6.png`（另有 som 版截图） |
| IPC 报错 | 启动后界面无错误提示；dev 日志无 error（前端 IPC 异常会打印到该日志） |

## 5. 存储布局第一手实测（同时校验 L0 §13）

`%APPDATA%\com.patchy23.patchybox`（= 当前 `storageRoot`，`layoutVersion = 2`）：

| 路径 | 内容 | L0 归属表核对 |
| --- | --- | --- |
| `settings.json` | `app`：`theme=dark`、`globalHotkey=Ctrl+Shift+Space`、`launchAtStartup=false`、`layoutVersion=2`、`tools.ssh.{idleDisconnectMinutes,idleDisconnectV2}` | 与 §13.1 一致 |
| `patchybox.json` | `settings`（`theme=light`、`language=zh-CN`、`globalHotkey`、`tools`）、`recent`、`favorites` | 与 §13.1 登记的「偏好重复建模」一致 |
| `data/` | `api.db`、`clipboard.db`、`database.db`、`dns.db`、`frp.db`、`frp/`、`credentials/`、`credentials-master.key`、`db-master.key` | 插件库 + 两套密钥 + 历史密钥，已按本次补登写入 §13.1/§13.5 |
| `vault/` | `vault.dat`、`vault-master.key` | 与 §13.5「降级文件在 `<storageRoot>/vault/`」一致 |
| `cache/`、`logs/`、`.window-state.json` | 设备级、不导出 | 与 §13.1 一致 |

实测差一点需要强调：**`theme` 在两处取值不同（`settings.json` 为 `dark`，`patchybox.json` 为 `light`）**，当前界面按其中一处生效。这是 L0 已登记的同名偏好重复建模，本轮只记录现象，不改行为（改动属 L1 范围）。

## 6. 缺项（环境待补，不得视为通过）

1. **真实鼠标/键盘交互未取证**：后台合成点击与键盘投递均返回 `effect: unverifiable` 且界面状态未变；升级到 foreground 时被系统拒绝（`foreground_unavailable`，实际前台窗口属 `chrome.exe`）。因此「点开工具页、切换页签、打开设置」这类交互本轮**没有**可核验证据。
2. **macOS 未记录**：无该平台环境，命令、日志、冷启动与窗口交互全部待补。
3. **release 构建未记录**：本轮只记录 dev 路径（`pnpm tauri dev`）；打包产物（NSIS 安装包 / release exe）的冷启动与首启行为待补。
4. **托盘与全局快捷键**：未验证系统托盘图标、`Ctrl+Shift+Space` 全局快捷键、关闭到托盘后再唤起。

## 7. 结论

- Windows 10（10.0.19045.6093，x86_64，WebView2 152.0.4191.66）上，**当前 HEAD 的 dev 冷启动通过**：Vite 就绪、Rust 增量编译成功、进程常驻无 panic、窗口标题与界面渲染正确、无 IPC 报错。
- 平台基线**不完整**：交互类证据、macOS、release 构建、托盘与全局快捷键均属缺项（§6），后续任一平台相关改动仍需补测。
