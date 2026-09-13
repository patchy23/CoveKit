# C 批交接与验收证据（rel-202609-001 可靠性与扩展治理）

> 范围：T07、T09、T08。规则与门禁以仓库为准；只写实际执行过的验证，未验证项单列。

## T07 · 设置读写、系统副作用与 schema 统一

- 字段级校验（`framework/settings.rs`）：主题 / 语言按枚举校验，自启按布尔、默认下载目录按字符串、
  工具设置按两级对象校验，快捷键先解析格式再落盘；错误信息可直接展示。
- 读改写层级统一：`settings_get` 从 `app.<key>` 读取并回落历史顶层写入位置，与 `settings_set` 的写入位置一致。
- 内部键保护：`storageRoot` / `layoutVersion` / `pendingMigration` / `lastMigration`（存储自举键）与
  `settingsRevision` / `globalHotkeyActive`（应用维护字段）拒绝通用写入。
- 敏感字段名（password / passphrase / secret / token / apiKey / privateKey）拒绝进入明文设置文件，提示改用凭证管理。
- 并发一致性：新增 `settings_revision` 与 `settings_patch`，读改写串行化 + 版本号校验，
  陈旧版本一律拒绝并提示重试；工具级设置新增 `settings_set_tool`，按 owner/key 合并，不再回传整个 `tools` 对象。
- 系统副作用顺序：开机自启与快捷键**先执行系统调用、成功才落盘**；失败返回错误并保持设置不变。
- 快捷键切换改为「先注册新的成功后才注销旧的」，失败时旧键继续有效，
  并把实际生效值写入只读字段 `globalHotkeyActive`；设置页据此显示「当前快捷键未生效」。
- 启动核对：`settings::init` 比对设置与系统实际自启状态，以系统状态为准修正设置，不再出现「显示已开启但系统没生效」。
- 前端（`stores/settings.ts`）：桌面环境只走 IPC；保存失败回滚乐观更新、暴露错误、保留可重试状态，
  不再写 localStorage 伪装成功；保存按调用顺序串行；工具设置走粒度接口；主题切换注册媒体查询订阅并支持释放。
- 持久化适配层（`core/storage.ts`）：按文件名缓存 Promise、加载失败清缓存可重试、可选 shape 校验、
  损坏 JSON 隔离备份后再按空值处理。
- 类型唯一源：`AppSettings` 与 `SettingsField` 收敛到 `core/ipc/contracts.ts`，`registry/types.ts` 只做再导出；
  `SettingsField` 改为判别联合（toggle / text / number / select），不再包含 secret 类型；
  清理无消费的 `recentTools` 镜像字段。
- 新增 14 个前端用例 + 6 个 Rust 用例覆盖上述行为（保存回滚、串行顺序、工具粒度、预览降级、缓存复用、损坏隔离）。

## T09 · 插件自报凭证引用，删除前状态可信

- 框架定义只读 `CredentialReferenceProvider`（`framework/credential_refs.rs`），插件在装配阶段 `register`；
  框架不再写任何插件的业务 SQL（原 `vault/store.rs` 里的 dns_config 查询整体移出）。
- 批量扫描：每个插件一次查询返回全部引用，框架按凭证聚合；返回 owner、对象 id/名称、计数与扫描状态。
- 扫描失败（库不可读、表结构不认识）记为 `unknown` 并带上原因，**不计入 0 条引用**；
  `matches_total` 用于删除前复核。
- DNS 与 SSH 各实现自己的只读扫描：DNS 查 `dns_config.credential_ref`，SSH 查 `ssh_profiles.credential_ref`；
  旧表缺列时返回未知而不是「无引用」。
- 删除语义：`vault_delete` 支持显式 `force` 与 `expected_references`；存在引用或有插件计数未知时没有显式确认一律拒绝，
  引用数在确认期间发生变化时返回错误要求重新确认，不依赖前端可能过期的计数。
- 前端删除确认列出各插件引用对象与「无法统计引用的插件」，查询失败按未知提示；删除后 toast 提示需手动改绑的处数。
- 删除前端 localStorage 引用镜像（`core/vault/references.ts` 及其用例）：SSH 数据已在后端持久化，镜像没有调用方。

## T08 · Windows/macOS 装配、资源访问与发布门禁

- Windows 专属 WebView2 调用条件化：`disable_native_context_menu` 调用点加 `#[cfg(windows)]`，
  修掉了非 Windows 目标无法编译（macOS 构建此前必然失败）的问题，不是靠移除 macOS 目标规避。
- 资源协议范围跟随本次生效根：新增 `paths::asset_scope_dirs` / `grant_asset_scope`，
  启动时只授权 `<activeRoot>/cache/tts`（递归），配置里的静态 scope 清空为 `[]`，
  data / vault / 设置文件不再位于任何 asset 授权范围内；取不到生效根时不授权。
- capability 与消费方核对：`core:window:allow-set-theme` 与 `allow-theme` 此前缺失
  （代码调用 `setTheme` 却无权限，失败被吞），现补齐；其余权限（opener / store / window-state /
  clipboard / dialog / updater / process / autostart）都有对应的已注册插件或调用方。
  前端主题同步失败改为 `console.warn` 输出，不再静默。
- 更新可用性（`framework/updater.rs` + `update_availability` 命令）：公钥缺失、仍是仓库占位值、
  下载地址为空时一律判为不可用并把原因交给界面；设置页据此显示「更新不可用：…」并禁用检查按钮，
  不再把占位配置当可用通道。
- 发布门禁：`release.yml` 新增 `gates` 作业（标签同 SHA 跑文档守卫、前端 lint/test/build/format、
  Rust fmt/clippy/test、Rust 规范检查，并校验标签与三处版本号一致），`publish` 依赖它；
  新增 `scripts/check_versions.py` 与 `scripts/check_release_config.py`
  （后者确认正式发布用的覆盖配置不是占位公钥、有下载地址、开启更新产物）。

## 验证

- Rust：`cargo fmt --check`、`cargo clippy --all-targets -D warnings`、`cargo test --no-default-features` 通过；
  新增用例：设置 6 项、凭证引用 5 项、DNS/SSH 引用扫描 5 项、更新可用性 3 项。
- 前端：`pnpm run lint`、`pnpm run test`、`pnpm run build`（含 `vue-tsc` 类型检查）、`pnpm run format:check` 通过；
  新增 8 项行为用例（设置保存回滚/串行顺序/工具粒度/预览降级/缓存复用/损坏隔离）。
- 脚本：`check_versions.py`（一致 → 0，标签不符 → 1）、`check_release_config.py`（缺失/占位 → 1，真实配置 → 0）实测。
- 文档：`check_docs.py`、`check_progress.py`、`check_markdown.py`、`check_doc_budget.py` 通过。

## 未验证项与限制

- macOS 侧全部为静态核对：本机为 Windows，未编译 macОS 目标，未验证 Keychain、dmg 打包、TTS 播放。
  条目挂账 `⏸`，需要在 macOS 执行环境（CI 或真机）复跑。
- TTS 播放未实测：新路径、中文/空格路径、自定义存储根与重启迁移后的播放都需要真机音频验证；本次只保证授权范围与路径计算。
- 更新链路未实测：未跑真实发布通道，未验证无效签名拒绝安装；占位公钥判定为纯函数 + 配置读取，实际发版需 CI 变量就位。
- 发布工作流未在 GitHub Actions 上执行过（本地无法运行），作业定义与脚本逻辑只做了本地校验。
- 快捷键占用/自启被系统拒绝的界面表现未真机走查（后端错误路径已实现，界面文案已就位）。

## 说明

- `framework/settings.rs`、`framework/vault/mod.rs` 的命令层改动同时更新了命令表与已发布契约测试，两者当前一致。
- 删除 `vault_reference_count`，由 `vault_credential_references` 取代（返回结构化引用概况而非计数）。
