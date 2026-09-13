# rel-202609-001 · A 批（T04 / T05）交接与验收证据

> 日期：2026-09-13。实现提交：`88689d6`（本文件与台账/待办的更新为后续独立文档提交）。
> 方案：同目录 [`A批实现方案.md`](A批实现方案.md)。规则与门禁依据：`AGENTS.md`、`TODO.md`、`docs/standards/20-验证矩阵.md`、`docs/standards/21-Git提交规范.md`。
> 范围：只做 A 批 T04/T05；B/C/D 批与 T15 未动。

## 1. 完成的任务编号与最终行为

| 项 | 要求 | 最终行为 | 落点 |
| --- | --- | --- | --- |
| T04-1 | 启用系统密钥库平台原生后端 | `keyring = { version = "3", features = ["apple-native", "windows-native"] }`；两个特性在 keyring 内按 target 门控，非对应平台是空操作 | `src-tauri/Cargo.toml` |
| T04-2 | 跨进程读写证明真的进了系统密钥库 | 新增 `secure_store/keyring_probe.rs`：父进程写、子进程新开 Entry 回读、末段 `delete_credential` 清理；**写/读/删全程只用专用测试 service**，不碰用户真实条目 | `src-tauri/src/framework/secure_store/keyring_probe.rs` |
| T04-3 | 旧降级密钥作为迁移输入、不当成新密钥来源 | 主密钥解析以「现有密文能否认证解密」为准：密钥库候选 → 降级文件候选 → 都不匹配则锁定报错；降级密钥胜出时登记进密钥库并回读校验，**降级文件保留** | `secure_store/key.rs` |
| T04-4 | 不再打印密钥材料 | 回读失败文案由纯函数生成，只写「无记录／长度不符／读失败类别」；`ResolvedKey` 手写 `Debug`，断言点检查不含原字节、hex、base64 | `secure_store/key.rs` |
| T04-5 | 保护状态可查询、降级对用户透明 | 新命令 `vault_protection_status` 返回每个域的后端（`system-keyring`／`file-fallback`／`unavailable`）、可用性（含 `locked`）、`keyring_has_key`／`fallback_file_exists`／`ciphertext_exists` 与降级原因；设置页凭证管理区块常驻徽章展示。**查询路径只读**，不写密钥库 | `framework/vault/mod.rs`、`secure_store/status.rs`、`src/core/ipc/{contracts,ipc}.ts`、`src/features/settings/SettingsPage.vue`、`src/i18n/locales/{zh-CN,en-US}.ts` |
| T04-6 | 不夹带产品决策 | 保留降级路径与 `KEYRING_SERVICE`／identifier；不引入主密码、不删降级文件 | 同上 |
| T05-1 | 公共安全原语唯一实现 | 新增 `framework/secure_store/`：`crypto`（AES-256-GCM，`nonce(12B)‖ciphertext`）、`key`（密钥解析／登记／脱敏文案）、`file`（唯一临时名 + fsync 的原子替换、备份恢复、坏文件归档、密钥文件权限收紧）、`win_acl`（Windows DACL）、`status`、`test_support`。已删 `credentials.rs` 与 `vault/store.rs` 里重复的加解密与 `replace_file` | `src-tauri/src/framework/secure_store/` |
| T05-2 | 备份恢复早于「当空表」 | 读路径先恢复：主文件缺失且 `.bak` 在 → 备份转正；主文件在且能认证 → 用主文件并清理已提交备份；主文件坏而备份可解 → 坏文件改名 `.corrupt-<ts>` 归档、备份转正，备份不删 | `secure_store/file.rs` |
| T05-3 | 缺钥锁死而非重造 | 有密文证据（含 `.bak`）且无候选可认证 → 返回锁定错误，不生成新密钥；只有正常空环境才首次生成 | `secure_store/key.rs` |
| T05-4 | 唯一临时名与可枚举故障点 | 临时名 `<名>.tmp-<uuid>`，写后 fsync 再替换；崩溃点在模块文档注释与用例名里对应 | `secure_store/{file.rs,mod.rs}` |
| T05-5 | 不删 `database` 命名空间兼容 | `credentials/database.enc` 格式与读写语义不变；数据库插件的「是否已有密文」判断改走 `has_stored_data`（把 `.bak` 也算证据） | `framework/credentials.rs`、`plugins/database/secrets.rs` |
| T05-6 | 并发读写 | 读改写统一在持锁的 `update_map_at`／`save_all` 内完成，配并发用例 | `framework/credentials.rs`、`framework/vault/store.rs` |

## 2. 自动测试命令与实际结果

在仓库根用 Git Bash（`export PATH="/c/Users/patchy/AppData/Roaming/npm:$PATH"`）：

| 命令 | 实际结果 |
| --- | --- |
| `cargo fmt --manifest-path src-tauri/Cargo.toml --check` | 通过（退出码 0） |
| `cargo clippy --manifest-path src-tauri/Cargo.toml --no-default-features --all-targets -- -D warnings` | 通过（退出码 0，零告警） |
| `cargo test --manifest-path src-tauri/Cargo.toml --no-default-features` | 通过：lib 224 项、`source_rules` 29 项；忽略项 7 个均为既有外部依赖项（3 个需网络：DNSPod 校验、DNS 实查询、Edge TTS 实合成；3 个需真实 SSH 服务器；1 个模块清单样例）。**连续三次全量运行均通过** |
| `PYTHONIOENCODING=utf-8 python scripts/check_rust_rules.py` | 通过（棘轮基线未放松） |
| `PYTHONIOENCODING=utf-8 python scripts/check_docs.py src-tauri/src` | 通过 |
| `pnpm build`（`vue-tsc --noEmit` + vite） | 通过 |
| `pnpm lint`（`--max-warnings 0`） | 通过 |
| `pnpm test` | 42 个文件 / 407 项通过 |
| `pnpm format:check`、`pnpm check:deps` | 通过 |
| `pnpm tauri dev` 冷启动 | Cargo 变更后重新构建并启动应用窗口成功（Cargo.toml 涉及依赖特性，按 §7.1 必做） |

关键回归用例（`secure_store` 内，均为真机可跑的自动化项）：跨进程真实密钥库读写、`locked_when_no_candidate_matches`（两处候选都不匹配时报错且不改写文件）、`existing_ciphertext_wins_over_new_key`（旧降级密钥胜出并登记）、`load_recovers_when_main_missing`（只剩备份也能恢复）、`backup_only_counts_as_ciphertext_evidence`、`uninitialized_reports_without_creating_keys`（查询不产生写副作用）、脱敏文案断言。

## 3. 新旧数据兼容夹具、故障注入点与恢复操作

- 夹具：`test_support.rs` 提供内存密钥库替身（`with_key`／`set_fail_read`／`set_fail_write`／`set_lose_writes`）与 `seed_fallback_file`；所有夹具都建在 `%TEMP%` 下的唯一随机目录，跑完即删，**不碰真实数据目录**。
- 故障注入点：密钥库读失败、写失败、写成功但不落库（静默丢写）、主文件缺失只剩 `.bak`、主文件损坏、无原生后端（非 Windows/macOS 目标）、Cargo 层 mock 后端（缺原生特性时回读必为 `NoEntry`）。
- 恢复操作：「凭证管理 → 导入」恢复备份；确认要丢弃密文时删密文重新初始化（状态里 `locked` 会给出这两条出路）。
- 兼容夹具覆盖了本机无法复现的场景：`.bak` 择版与恢复、锁死、降级密钥登记。

## 4. 真实数据处置

- 动代码前已整份备份：`G:\workspace\back\patchybox-data-20260913-225543`（源 `%APPDATA%\com.patchy23.patchybox`，33MB）。
- 收尾复核：`data/credentials-master.key`、`data/db-master.key`、`vault/vault.dat`、`vault/vault-master.key` 四个文件的 sha256 与备份**完全一致**，密文未被重写、未生成任何新密钥。
- 冷启动运行真实应用期间未发生密钥写入：`cmdkey /list` 中当前**没有** patchybox 相关条目（此前探针泄漏的条目已删除，残留计数 0）。
- 需要用户拍板的行为（已实现、未由我触发）：降级密钥在通过既有密文认证后，会被**登记**进系统密钥库并回读校验（降级文件保留）。首次真实触发发生在应用真正读取该域凭证时（例如打开设置页查询状态后使用凭证管理）。它不生成新密钥、不改密文；如用户不接受「降级密钥自动登记」，需要单独裁决，按现实现可通过只读解析路径退化为「只提示不登记」。

## 5. 未验证项

- 设置页保护状态徽章的**真机视觉确认**未取到：本轮尝试用合成点击打开设置页未生效（窗口 pid 在构建重启后变化），前端类型检查、单测与构建均已通过。请手动打开「设置 → 凭证管理」确认徽章文案。
- 降级密钥登记进系统密钥库后的**真实凭据管理器条目**未在本机触发（应用启动未读取该域凭证）；条目内容与回读校验只有进程内 + 专用测试 service 的证据。
- 迁移场景（旧机器只有降级密钥文件、系统密钥库为空）只能靠夹具，未在真实旧数据上复现。
- macOS：原生 Keychain、文件 0600、跨进程用例本机无法运行。
- Linux：无系统密钥库后端，按「不可用」如实上报，不做原生验证。
- 断电时刻的物理落盘顺序，只能靠故障注入用例近似，未做掉电测试。
- 任务书 §7.2 验收链中与本批相关的一条（系统 keyring 不可用后恢复）已由故障注入用例覆盖判定逻辑，真机恢复操作仍待用户自测。
