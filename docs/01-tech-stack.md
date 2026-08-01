# 技术选型分析 · ToolKit 桌面工具箱

> 结论先行：**Tauri 2.11 + Vue 3 + TypeScript + Vite + Tailwind CSS 4 + Pinia**
> 前端框架以 Vue 3 为推荐默认；若团队更熟 React，本文档的架构设计同样适用（框架只影响 `src/features` 层的写法，`core/` 与 Rust 侧完全不变）。

---

## 1. 桌面容器：为什么是 Tauri 2

用户已确定 Tauri。这里给出与主要替代方案的客观对比，作为团队内部决策依据。

| 维度 | Tauri 2 | Electron | WPF (.NET 8) | Qt (C++) | Flutter Desktop |
|------|---------|----------|--------------|----------|-----------------|
| 安装包体积 | **~8–15 MB** | 100–200 MB | 60–120 MB（.NET 运行时） | 30–60 MB | 20–40 MB |
| 内存占用 | **~100–200 MB** | 300–600 MB | 150–300 MB | 120–250 MB | 150–300 MB |
| UI 技术 | HTML/CSS/JS | HTML/CSS/JS | XAML | QML/C++ | Flutter 自绘 |
| 前端复用 | ✅ 全复用 | ✅ 全复用 | ❌ | ❌ | ⚠️ 部分 |
| 后端语言 | Rust（内存安全） | Node.js | C# | C++ | Dart |
| 系统能力 | 插件化（官方 30+ 插件） | 全量 Node API | 强 | 强 | 中 |
| 跨平台 | Win/macOS/Linux | Win/macOS/Linux | Windows 为主 | 全平台 | 全平台 |
| 打包分发 | NSIS/MSI/dmg/AppImage + 自动更新 | 成熟 | ClickOnce/MSIX | 复杂 | 中 |
| 生态活跃度 | 高（Tauri 2 于 2024 年正式版，持续迭代） | 极高（但体积/内存是硬伤） | 成熟但偏企业 | 成熟但授权费 | 中 |

**Tauri 2 的核心优势（对本项目）：**
1. **体积与内存**：工具箱是常驻/常唤起型应用，内存占用直接影响用户是否愿意开机自启。
2. **Rust 后端**：剪贴板监听、屏幕取色、磁盘扫描这类系统级工具，Rust 性能与内存安全兼备。
3. **官方插件生态**（v2.11 已确认）：`clipboard-manager`、`global-shortcut`、`store`、`sql`、`single-instance`、`autostart`、`notification`、`window-state`、`updater` 等，覆盖工具箱 90% 的系统能力需求。
4. **权限模型**：capabilities 白名单机制，天然安全（详见架构文档 §7）。

**需要接受的代价：**
- 前端渲染依赖 WebView2（Win10 1803+ 内置，旧系统需引导安装）；
- Rust 首次编译较慢（后续增量编译可接受）；
- Windows 上透明/异形窗口有已知坑（毛玻璃需谨慎，见风险 §7）。

---

## 2. 前端框架：Vue 3 + TypeScript

| 维度 | Vue 3.5 | React 19 | Svelte 5 |
|------|---------|----------|----------|
| 学习曲线 | 低（模板直观） | 中（hooks 心智） | 低 |
| 中文资料/社区 | **最丰富** | 丰富 | 一般 |
| 与 Tauri 模板契合 | 官方模板首选 | 官方模板 | 官方模板 |
| 状态管理 | Pinia（轻量、TS 友好） | Zustand/Redux | 内置 stores |
| 生态（组件/工具） | 大 | 最大 | 小 |
| 打包产物 | 小 | 中 | **最小** |

**决策：Vue 3 + TypeScript。** 理由：
1. 团队（假定中文开发者）查资料成本最低；
2. Tauri 官方 `create-tauri-app` 的 vue-ts 模板开箱即用；
3. Pinia + `<script setup>` 组合式 API 与工具注册表这类"可插拔"架构天然契合；
4. 本项目是自研组件（不依赖重型 UI 库），Vue 的单文件组件粒度正合适。

> 若团队 React 熟练度明显更高，替换成本集中在 `src/features/` 与 `src/stores/`，`core/`、`tools/` 目录结构、IPC 契约、Rust 侧零改动。

---

## 3. UI 方案：Tailwind CSS 4 + 自研组件库

**不引入 Element Plus / Naive UI 等重型组件库。** 理由：
- 设计规范（`DESIGN.md`）已锁定视觉语言，组件库默认样式会与规范冲突，改造工作量大于自研；
- 工具箱的工具界面形态差异大（表单、画布、取色器…），通用组件库帮不上核心部分；
- 桌面应用无浏览器兼容性包袱，可以放心用现代 CSS。

**Tailwind CSS 4**（CSS-first 配置）与 `DESIGN.md` tokens 无缝对接：

```css
/* src/assets/styles/main.css */
@import "tailwindcss";

@theme {
  --color-primary: #191D23;
  --color-secondary: #5C6672;
  --color-accent: #F0562C;
  --color-accent-strong: #C2410C;
  --color-accent-soft: #FDEEE7;
  --color-bg: #F4F5F7;
  --color-surface: #FFFFFF;
  --color-border: #E5E8EC;
  --radius-card: 14px;
  --font-sans: "Inter", "PingFang SC", "Microsoft YaHei", sans-serif;
}
/* 深色模式：@custom-variant dark 绑定 [data-theme=dark] */
@custom-variant dark (&:where([data-theme="dark"], [data-theme="dark"] *));
```

- 主题切换 = 切换 `document.documentElement.dataset.theme`，配合 `dark:` 变体；
- 组件封装为 `src/features/ui/` 下的基础组件（Button / Card / Chip / Modal / Toast），输入输出对齐 DESIGN.md；
- 图标：统一内联 SVG 组件（沿用原型的线性图标集），不引入图标字体。

---

## 4. 状态管理与前端依赖清单

| 依赖 | 版本策略 | 用途 |
|------|---------|------|
| vue / vue-router | ^3.5 / ^4 | 框架；未来设置页/工具详情页路由 |
| pinia | ^3 | 全局状态（工具、设置、收藏、剪贴板） |
| tailwindcss | ^4 | 样式与 tokens |
| fuse.js | ^7 | 工具模糊搜索（名称+关键词+描述加权） |
| vue-i18n | ^11 | 中英双语（zh-CN 默认） |
| unplugin-auto-import / unplugin-vue-components | latest | 减少样板代码 |
| typescript / vue-tsc | ^5 | 类型检查 |
| vitest + @vue/test-utils + happy-dom | ^3 | 单元测试 |

---

## 5. Rust 侧与官方插件映射

当前 Tauri 版本：**2.11.x**（crates.io 已确认）。核心 crate：`tauri`、`tauri-plugin-*`、`serde`、`serde_json`、`rusqlite`（经由 sql 插件）。

| 功能需求 | 插件 / crate | 备注 |
|---------|-------------|------|
| 剪贴板历史 | `tauri-plugin-clipboard-manager` + `tauri-plugin-sql`(SQLite) | 轮询监听 + 落库 |
| 全局唤起快捷键 | `tauri-plugin-global-shortcut` | 呼出/隐藏主窗 |
| 设置/收藏持久化 | `tauri-plugin-store` | JSON 文件，简单键值 |
| 剪贴板历史/大数据 | `tauri-plugin-sql` | SQLite（`sqlite:` 方言） |
| 单实例 | `tauri-plugin-single-instance` | 防多开，二次唤起聚焦 |
| 开机自启 | `tauri-plugin-autostart` | 设置项 |
| 系统通知（番茄钟等） | `tauri-plugin-notification` | |
| 文件对话框（图片压缩/重命名） | `tauri-plugin-dialog` + `tauri-plugin-fs` | |
| 打开外链 | `tauri-plugin-opener` | 安全替代 `shell` |
| 窗口状态记忆 | `tauri-plugin-window-state` | 位置/尺寸恢复 |
| 托盘 | 内置 `tray-icon` | 托盘菜单 + 显示/退出 |
| 自动更新 | `tauri-plugin-updater` + 内置 bundler | 后期接入 |
| 屏幕取色（系统级） | 自研 Rust 命令 | Windows: `GetDC`/`GetPixel` 或全屏截图 |
| 磁盘扫描 | 自研 Rust 命令 | `walkdir` crate，异步 + 进度事件 |

**明确不引入：** `tauri-plugin-shell`（默认安全策略下权限繁琐且风险高，需要执行外部命令的场景用 `opener` 或自研白名单命令替代）。

---

## 6. 工程化与发布

| 环节 | 方案 |
|------|------|
| 脚手架 | `pnpm create tauri-app`（vue-ts 模板） |
| 包管理 | pnpm（workspace 单一包，无 monorepo 必要） |
| 代码规范 | ESLint 9 flat config + Prettier（前端）；`rustfmt` + `clippy -D warnings`（Rust） |
| 测试 | Vitest（前端单元）+ `cargo test`（Rust 命令单测）+ Playwright 可选（E2E） |
| CI | GitHub Actions：lint → test → `tauri build` → 上传安装包 artifact |
| 打包 | `tauri build`：Windows NSIS（默认）/ MSI；后期加 macOS dmg |
| 自动更新 | Tauri updater（需自建静态资源服务器或 GitHub Releases，签名密钥 `tauri signer`） |
| 代码签名 | Windows Authenticode（后期发布前必做，否则 SmartScreen 拦截） |
| 版本管理 | 常规 SemVer；`tauri.conf.json` 中 `version` 与 Cargo.toml 同步 |

---

## 7. 风险与对策

| 风险 | 等级 | 对策 |
|------|------|------|
| WebView2 缺失/过旧（Win10 1803 前） | 中 | 打包时带 WebView2 引导安装；`webviewInstallMode: downloadBootstrapper` |
| Rust 首次编译慢 | 低 | CI 缓存 `target/` 与 cargo registry；本地增量编译 |
| Windows 透明窗口毛玻璃闪烁/性能 | 中 | 方向二为**不透明浅色**设计，天然规避；仅弹窗用 backdrop-filter（WebView2 支持） |
| 剪贴板轮询 API 限制 | 中 | 剪贴板无系统级变更通知（Windows），用 500ms 轮询 + 内容 hash 去重；图片记录只存缩略图路径 |
| 全局快捷键被占用 | 低 | 注册失败降级提示；设置页提供改键 |
| 自动更新签名/服务器成本 | 低 | M4 再做，先用 GitHub Releases + 手动下载兜底 |

---

## 8. 结论

```
Tauri 2.11 (Rust)  ── 系统能力层：剪贴板/取色/磁盘/快捷键/托盘/更新
        ↑ IPC (invoke / event)
Vue 3.5 + TS      ── 应用层：工具注册表 / 状态 / 路由
        ↑
Tailwind 4 + DESIGN.md tokens ── 表现层：方向二视觉规范
```

技术栈整体成熟、轻量、可控。前端框架若需更换，架构边界已预留。下一步进入详细设计（见 `02-architecture.md`）。
