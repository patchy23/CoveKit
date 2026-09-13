# rel-202609-001 · A 批（T04 / T05）实现方案

> 日期：2026-09-13。范围：T04 系统密钥库真实接入与降级透明化、T05 合并凭证文件安全原语并补齐崩溃恢复。
> 依据：`可靠性与扩展治理任务书.md` §4 T04/T05、§7 交付规范；规则与门禁以仓库 `AGENTS.md`、`TODO.md`、`docs/standards/20-验证矩阵.md` 为准。
> 本文只覆盖 A 批；T01/T02/T03 走 B 批，T15 与 T14/T16 不动。

## 1. 开工核对（对任务书与用户现状说明的更正）

| 核对项 | 结论 | 证据 |
| --- | --- | --- |
| keyring 未启用原生后端 | 成立 | `src-tauri/Cargo.toml:59` 为 `keyring = "3"`，3.6.3 的 `[features]` 无 `default`，`cargo tree -e features` 只见 default 空集 |
| mock 后端后果比任务书更重 | 成立 | 3.6.3 `src/mock.rs` 把密文存在**凭据实例内部**，新建 `Entry` 读回必为 `NoEntry`；因此 `create_master_key` 的回读校验**每次都判定未持久化** → 每台机器都会落 `vault-master.key`，代码从未真正用过系统密钥库 |
| 本机 `vault.dat`/`vault-master.key` 不存在 | **更正：两者都存在** | `%APPDATA%\com.patchy23.patchybox\vault\{vault.dat,vault-master.key}`（460B / 32B，修改时间 2026-09-05 与 2026-08-16）。Vault 已在被真实使用，不能用「未被使用」的假设做迁移设计 |
| `db-master.key` 零引用 | **更正：有引用，归属明确** | `plugins/database/secrets.rs:37` 读它解密旧 `db-secrets.enc` 后导入公共库；它是 database 插件的旧密文迁移输入，本批只做确认，不移动、不删除 |
| T05 原语重复 | 成立 | `credentials.rs:100` 与 `vault/store.rs:207` 各有一份 `replace_file`；`credentials.rs:46` 无锁生成主密钥；`read_map_at` 不恢复 `.bak` |

备份：动代码前已整份复制真实数据目录到 `G:\workspace\back\patchybox-data-20260913-225543`（28 个文件 33MB，密钥与密文 sha256 与原件一致）。

## 2. 技术路线与取舍

### 2.1 新增 `framework/secure_store/`（T05-1）

一个域=「一个主密钥 + 一组密文文件」。Vault 仍负责凭证条目模型，`credentials` 仍负责兼容 KV 格式，两者只共用底层原语。

| 文件 | 职责 |
| --- | --- |
| `mod.rs` | 模块文档、关键约束常量（`KEYRING_SERVICE` 不变）、再导出、`KeySpec`/`KeySource`/`ResolvedKey`/`ProtectionBackend` 等契约类型 |
| `crypto.rs` | AES-256-GCM `nonce(12B)‖ciphertext` 加解密（唯一实现） |
| `key.rs` | `MasterKeyStore` 抽象（按 account 读写）、`KeyringStore`、主密钥**按密文证据**解析、降级密钥提升进密钥库、脱敏诊断文案 |
| `file.rs` | 唯一临时名 + fsync 的原子替换、备份恢复、坏文件归档、按认证+解析结果择版、密钥文件权限收紧 |
| `win_acl.rs` | Windows 专用 DACL 收紧（`cfg(windows)`，unsafe 集中在 2 处并带 SAFETY 论证） |
| `test_support.rs` | `cfg(test)` 内存密钥库替身（可注入读失败/写失败/静默丢写） |

**取舍：**
- `MasterKeyStore` 改为按 account 读写（`read(account)`/`write(account, key)`），避免「store 里的 account 与 spec 不一致」这类静默错配；生产实现仍固定 `KEYRING_SERVICE = com.patchy23.patchybox`（任务书 §6 兼容要求：不改 service/identifier）。
- 主密钥解析不再「keyring 优先」，而是**以现有密文能否被认证解密为准**（T04-3）。两处候选都不匹配且密文存在时返回锁死错误，绝不生成新密钥。
- 降级文件里的正确旧密钥**提升**进系统密钥库（写后回读校验），但**不删除降级文件**：任务书要求沿用兼容策略，且回读失败时必须仍有可用来源。
- `credentials` 命名空间的主密钥仍是本地 `credentials-master.key`（历史事实），但纳入同一解析链：有系统密钥库记录就用，没有就用文件并尝试提升。这样 T04-5 的保护状态对数据库密码域也成立。
- 版本择取（主文件 vs `.bak`）以「认证解密 + JSON 解析」双通过为判据；主文件坏而备份可用时，主文件改名归档（`.corrupt-<ts>`），备份转正，**不删备份**。

### 2.2 T04 具体落点

| 要求 | 落点 |
| --- | --- |
| 1 启用平台原生后端 | `Cargo.toml`：`keyring = { version = "3", features = ["apple-native", "windows-native"] }`；两个 feature 的依赖在 keyring 内已按 target 门控，非对应平台为空操作。Linux 无系统密钥库后端（非产品目标平台），按「不可用」如实上报，不冒充 |
| 2 跨进程读写测试 | `key.rs` 用例：专用 service `com.patchy23.patchybox.tests` + 随机 account，父进程写，子进程（`current_exe` 重入同一用例）新 `Entry` 读，末段 `delete_credential` 清理；非 Windows/macOS 编译目标显式跳过并打印原因 |
| 3 旧降级密钥是迁移输入 | `resolve_master_key`：候选=密钥库/降级文件，判据=现有密文认证；降级密钥胜出时提升进密钥库并跨 Entry 回读校验 |
| 4 不再打印密钥材料 | `create_master_key` 的回读失败文案改由 `key.rs` 纯函数生成（只写「无记录/长度不符/读失败类别」），配单测断言文案不含原字节、hex、base64 |
| 5 保护状态与文件权限 | 新 IPC `vault_protection_status`（`vault::` 框架命令）+ 设置页「凭证管理」区块常驻显示后端与原因；降级密钥文件写后收紧权限（Windows DACL 仅当前用户；macOS/Unix 0600） |
| 6 不夹带产品决策 | 保留降级路径；不引入主密码、不删除降级文件、不改 service/identifier |

### 2.3 T05 具体落点

| 要求 | 落点 |
| --- | --- |
| 1 公共原语 | 见 2.1；`credentials.rs` 与 `vault/store.rs` 的重复 `replace_file`/加解密删除 |
| 2 备份早于空表 | 读路径先 `recover`：主文件缺失且备份在 → 备份转正；主文件在且备份在 → 认证通过则采用主文件并清理已提交的备份，主文件坏而备份可解 → 归档坏文件、备份转正 |
| 3 锁死而非重造密钥 | 证据非空且无候选可认证 → `locked` 错误；正常空环境才生成 |
| 4 唯一临时名与故障点 | 临时名 `<名>.tmp-<uuid>`，写完 fsync 再替换；崩溃点与恢复规则写进模块文档注释与测试名 |
| 5 不删 database 命名空间 | 保持 `credentials/database.enc` 现有格式与读写兼容（消费方 `plugins/database/secrets.rs`） |
| 6 并发 | 读改写统一在 `update_map_at`/`save_all` 内持锁完成（进程内互斥），配并发用例 |

## 3. 改动面（逐文件）

- 新增：`src-tauri/src/framework/secure_store/{mod,crypto,key,file,win_acl,test_support}.rs`
- 改：`src-tauri/src/framework/credentials.rs`（改用公共原语、加锁、备份恢复、锁死语义）
- 改：`src-tauri/src/framework/vault/store.rs`（瘦身为凭证模型 + 读写 + 引用统计，加保护状态查询）
- 改：`src-tauri/src/framework/vault/mod.rs`（新增 `vault_protection_status` 命令）
- 改：`src-tauri/src/framework/mod.rs`（命令入库清单加一行）
- 改：`src-tauri/src/framework/manifest_contract_tests.rs`（已发布命令表加同一行，这是契约守卫的既定用法）
- 改：`src-tauri/Cargo.toml`（keyring features；windows-sys 增加 Security/Authorization/Threading/FileSystem）
- 改：`src/core/ipc/contracts.ts`、`src/core/ipc/ipc.ts`（新命令契约）
- 改：`src/features/settings/SettingsPage.vue`（凭证管理区块显示保护状态与恢复入口）、`src/i18n/locales/{zh-CN,en-US}.ts`
- 文档：本方案、`可靠性与扩展治理任务书.md` §4 T04/T05 实施说明（如行为与任务书不一致只追加说明）、`docs/进度台账.md`、`TODO.md`

不碰：`branding/`、其它批次任务书、T15、`db-master.key`、`plugins/database/secrets.rs`（仅只读确认归属）、`paths.rs` 布局迁移（T03 范围）。

## 4. 步骤与提交划分

1. 方案落盘（本文件）。
2. `secure_store` 骨架 + 原语（crypto/key/file/权限）+ 单测；故障注入用例先写（红）再实现（绿）。
3. `credentials.rs` 接入；`vault/store.rs` 接入并瘦身；旧用例迁到公共替身。
4. T04-1/2/4：Cargo features、跨进程用例、日志脱敏用例。
5. 保护状态：后端查询 + IPC + 前端显示 + i18n。
6. 门禁：`pnpm lint/format/test/build`、`cargo fmt --check`、`cargo clippy --all-targets -D warnings`、`cargo test --lib`、`source_rules`、文档检查；`pnpm tauri dev` 冷启动（Cargo 变更必需）+ 真实数据只读核对。
7. 独立记录提交更新台账与 TODO。

每步一个提交，标题按 `21-Git提交规范.md` 写实际变化，不带批次号；只 `git add` 本会话文件。

## 5. 风险与回退

| 风险 | 处理 |
| --- | --- |
| 真实数据被误写 | 动前整份备份（路径见 §1）；所有读路径只读，写路径只在「新密钥已就绪」时执行；解析失败一律报错不改文件 |
| 提升密钥进系统密钥库引发争议 | 只在「该密钥已被现有密文认证」时写入，且不删降级文件；报告中单列该行为供用户裁决 |
| Windows DACL 代码出错导致文件不可读 | 收紧后立刻自读自写校验；失败只告警不阻断（保护性降级不阻塞业务），并在报告中列未验证项 |
| `replace_file` 语义变化影响 SSH 手工凭证 | SSH 手工路径使用 `plugins/ssh/credential.rs` 自己的实现，本批不改其调用契约；`vault`/`credentials` 对外行为（键值读写语义）不变 |
| 门禁失败 | 同一失败三轮无新证据即停手，改方案或列未验证项 |

## 6. 预期未验证项（收尾按实际结果更新）

- macOS：原生 Keychain 生效、0600 权限、跨进程用例，本机无法运行。
- Linux CI 仅编译与单测，系统密钥库按「不可用」上报，不做原生后端验证。
- 真实系统密钥库中的旧条目（换机/清空场景）与断电时刻的物理落盘顺序，只能靠故障注入用例近似。
