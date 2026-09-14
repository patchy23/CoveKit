//! 模块清单契约测试（AR07 §10.2 验证表）
//!
//! 三个漂移方向都必须在这里被抓住：
//! 1. 已发布命令表 ↔ 登记元数据：前端 `src/plugins/*/ipc.ts` 依托这张表，
//!    改名或换归属者却不更新表 = 破坏调用契约；
//! 2. 元数据 ↔ 实际路由：登记了命令却没有路由分支，运行期是静默「command not found」；
//! 3. 实现 ↔ 声明：源码里任何 `#[tauri::command]` 都必须出现在模块清单里
//!    （手写绕过入口无处藏身，这是 AR07 之前 11 条命令「实现了但没入库」的直接守卫）。

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;

    use crate::framework::ipc_registry;
    use crate::framework::module_manifest;

    /// 序列化触碰全局注册表的测试：注册表是进程级单例，并行跑会互相清空
    static REGISTRY_LOCK: Mutex<()> = Mutex::new(());

    /// 取测试锁；已有用例 panic 造成的锁中毒不影响后续用例（状态本就要重置）
    fn lock_registry() -> std::sync::MutexGuard<'static, ()> {
        REGISTRY_LOCK.lock().unwrap_or_else(|err| err.into_inner())
    }

    /// 从空表开始：清注册表 + 清模块描述表
    fn reset_all() {
        ipc_registry::reset();
        module_manifest::reset_modules();
    }

    /// 已发布命令表：(归属者, IPC 名称, 中文说明)
    ///
    /// 冻结快照的是**前后端调用契约**：增删命令、改命令名或改归属者时这张表必须同步更新。
    const PUBLISHED: &[(&str, &str, &str)] = &[
        ("database", "dbc_cancel", "取消进行中的查询"),
        (
            "database",
            "dbc_charset_options",
            "字符集与排序规则选项（建库对话框）",
        ),
        ("database", "dbc_columns", "表结构列信息"),
        (
            "database",
            "dbc_connect",
            "建立数据库连接（探测版本与时延）",
        ),
        (
            "database",
            "dbc_connection_delete",
            "删除数据库连接配置与凭据",
        ),
        (
            "database",
            "dbc_connection_save",
            "保存连接配置（密码入插件私有 AES；已连接则更新会话）",
        ),
        ("database", "dbc_connections", "连接列表（配置 + 会话状态）"),
        (
            "database",
            "dbc_create_database",
            "新建数据库（含可选分步授权）",
        ),
        ("database", "dbc_databases", "数据库列表"),
        ("database", "dbc_disconnect", "断开数据库连接"),
        (
            "database",
            "dbc_driver_status",
            "agent 驱动就绪状态（含目录指引）",
        ),
        (
            "database",
            "dbc_drop_database",
            "删除数据库（前端确认后调用）",
        ),
        (
            "database",
            "dbc_execute",
            "执行 SQL（多语句拆分，查询返回表格）",
        ),
        ("database", "dbc_export_csv", "导出 CSV 文件（结果集导出）"),
        ("database", "dbc_history", "查询历史列表"),
        ("database", "dbc_history_add", "追加查询历史"),
        ("database", "dbc_history_clear", "清空查询历史"),
        ("database", "dbc_objects", "对象列表（表/视图等）"),
        (
            "database",
            "dbc_redis_key_info",
            "Redis 键信息（TYPE/TTL/预览）",
        ),
        ("database", "dbc_redis_keys", "Redis 键列表（SCAN）"),
        ("database", "dbc_saved", "收藏 SQL 列表"),
        ("database", "dbc_saved_add", "添加收藏 SQL"),
        ("database", "dbc_saved_delete", "删除收藏 SQL"),
        (
            "database",
            "dbc_saved_update",
            "更新收藏 SQL（编辑器二次保存）",
        ),
        ("database", "dbc_schemas", "schema 列表"),
        ("database", "dbc_table_admin", "表维护（重命名/清空/删除）"),
        ("database", "dbc_table_data", "表数据分页浏览"),
        ("database", "dbc_table_ddl", "表 DDL 查看"),
        ("database", "dbc_table_indexes", "表索引清单"),
        ("database", "dbc_test", "测试数据库连接（不保存）"),
        ("database", "dbc_users", "数据库用户清单（授权选择）"),
        ("dns", "dns_add_record", "云解析添加记录"),
        ("dns", "dns_config_get", "读取云平台密钥配置"),
        ("dns", "dns_config_set", "保存云平台密钥配置"),
        ("dns", "dns_delete_record", "云解析删除记录"),
        (
            "dns",
            "dns_domains",
            "云解析域名列表（aliyun/dnspod/cloudflare）",
        ),
        ("dns", "dns_query", "DNS 查询（指定服务器/多服务器对比）"),
        ("dns", "dns_records", "云解析记录列表（分页）"),
        ("dns", "dns_update_record", "云解析更新记录"),
        (
            "framework",
            "app_force_exit",
            "用户强制退出（跳过业务拦截，清理仍受总超时约束）",
        ),
        (
            "framework",
            "app_request_exit",
            "请求退出应用（业务可拒绝，拒绝原因交回前端展示）",
        ),
        (
            "framework",
            "framework_tasks",
            "查询框架长任务清单（活跃与最近结束，供界面与诊断使用）",
        ),
        (
            "framework",
            "framework_commands",
            "查询全量已入库 IPC 命令（名称 + 说明）",
        ),
        (
            "framework",
            "open_external",
            "打开外部链接（tauri-plugin-opener，安全替代 shell 插件）",
        ),
        ("framework", "settings_get", "读取应用设置（可指定 key）"),
        (
            "framework",
            "settings_set",
            "写入应用设置（launchAtStartup 有联动副作用）",
        ),
        (
            "framework",
            "settings_patch",
            "批量写入应用设置（带版本号校验，拒绝陈旧覆盖）",
        ),
        ("framework", "settings_set_tool", "按工具与键写入工具级设置"),
        (
            "framework",
            "settings_revision",
            "读取设置版本号（保存时回传防覆盖）",
        ),
        (
            "framework",
            "update_availability",
            "读取更新可用性（占位公钥等无效配置按不可用上报）",
        ),
        (
            "framework",
            "storage_info",
            "读取存储位置信息（四分区路径与占用、待执行计划、恢复状态）",
        ),
        (
            "framework",
            "storage_schedule_migration",
            "安排存储目录迁移（只登记计划，重启后复制并校验）",
        ),
        (
            "framework",
            "storage_cancel_migration",
            "取消待执行的存储目录迁移计划（不修改业务文件）",
        ),
        (
            "framework",
            "storage_recovery_status",
            "读取存储恢复状态（配置盘不可用或迁移失败）",
        ),
        (
            "framework",
            "storage_recovery_action",
            "执行存储恢复动作（retry/use-default/choose）",
        ),
        (
            "framework",
            "vault_delete",
            "删除凭证（返回被引用计数供前端提示）",
        ),
        (
            "framework",
            "vault_export",
            "密码加密导出 .pbvault 备份（Argon2id 派生密钥）",
        ),
        (
            "framework",
            "vault_import",
            "解密导入 .pbvault 备份（合并/覆盖由 UI 选择）",
        ),
        (
            "framework",
            "vault_list",
            "凭证列表（脱敏摘要：id/name/kind/掩码/时间，无明文）",
        ),
        (
            "framework",
            "vault_protection_status",
            "凭证保护状态（主密钥实际来源与可用性，设置页展示）",
        ),
        (
            "framework",
            "vault_credential_references",
            "查询凭证引用概况（按插件自报能力批量扫描）",
        ),
        (
            "framework",
            "vault_reveal",
            "读取单条凭证明文（仅用户点显示/复制时调用）",
        ),
        (
            "framework",
            "vault_save",
            "新增/更新凭证（payload 打包，id 可选 upsert）",
        ),
        ("framework", "window_hide", "隐藏主窗口（最小化到托盘）"),
        (
            "framework",
            "window_toggle",
            "切换主窗口显示/隐藏（返回切换后可见性）",
        ),
        (
            "frp",
            "frp_binary_detect",
            "探测 frpc（可传候选路径，否则走设置项与自动查找）",
        ),
        (
            "frp",
            "frp_binary_download",
            "下载并安装 frpc（含 SHA256 校验）",
        ),
        ("frp", "frp_binary_versions", "查询上游 frp 可用版本列表"),
        (
            "frp",
            "frp_client_add",
            "登记外部 frpc 可执行文件（只引用路径，不复制）",
        ),
        (
            "frp",
            "frp_client_list",
            "已登记客户端列表（含默认项与文件是否还在）",
        ),
        ("frp", "frp_client_remove", "移除客户端登记（不删除文件）"),
        ("frp", "frp_client_set_default", "设为默认客户端"),
        ("frp", "frp_profile_client_set", "设置档案绑定的客户端"),
        ("frp", "frp_profile_create", "新建档案（内置模板）"),
        ("frp", "frp_profile_delete", "删除档案（移入 .trash/ 软删）"),
        ("frp", "frp_profile_duplicate", "复制档案"),
        ("frp", "frp_profile_read", "读取档案原文与 TOML 解析结果"),
        ("frp", "frp_profile_remark", "写入档案备注（只落 frp.db）"),
        ("frp", "frp_profile_rename", "重命名档案（同步迁移备注）"),
        (
            "frp",
            "frp_profile_save_form",
            "表单模式保存档案（重建 TOML 并保留未知字段）",
        ),
        (
            "frp",
            "frp_profile_save_text",
            "按原文保存档案（保存前自动备份）",
        ),
        (
            "frp",
            "frp_profiles_list",
            "档案列表（含运行状态、备注与元信息）",
        ),
        ("frp", "frp_restart", "重启档案对应的 frpc 进程"),
        ("frp", "frp_start", "启动档案对应的 frpc 进程"),
        ("frp", "frp_status", "查询全部档案的当前运行状态"),
        ("frp", "frp_stop", "停止档案对应的 frpc 进程"),
        ("frp", "frp_verify", "用 frpc verify 校验档案并解析错误行列"),
        ("hosts", "hosts_read", "读取 hosts 文件"),
        ("hosts", "hosts_save", "备份并写入 hosts（平台提权）"),
        ("http_ws", "api_clear", "清空全部接口"),
        ("http_ws", "api_delete", "删除接口"),
        ("http_ws", "api_list", "接口列表"),
        ("http_ws", "api_save", "保存/更新接口（id=0 新增）"),
        ("http_ws", "http_request", "发送 HTTP 请求"),
        ("http_ws", "ws_close", "关闭 WS 会话"),
        (
            "http_ws",
            "ws_connect",
            "建立 WebSocket 连接（支持自定义请求头）",
        ),
        ("http_ws", "ws_recv", "拉取会话快照"),
        ("http_ws", "ws_send", "发送 WS 消息"),
        ("http_ws", "ws_sessions", "全部 WS 会话"),
        ("ssh", "ssh_bookmark_add", "新增目录书签"),
        ("ssh", "ssh_bookmark_delete", "删除目录书签"),
        ("ssh", "ssh_bookmark_list", "目录书签列表"),
        (
            "ssh",
            "ssh_connect",
            "建立 SSH 连接（按 profileId 取配置，凭证在 Rust 侧解析）",
        ),
        ("ssh", "ssh_connections", "全部会话快照（侧栏轮询）"),
        ("ssh", "ssh_disconnect", "断开连接并清理会话"),
        ("ssh", "ssh_docker_action", "容器启动/停止/重启/删除"),
        ("ssh", "ssh_docker_exec", "进入容器终端（PTY）"),
        ("ssh", "ssh_docker_list", "Docker 容器列表"),
        ("ssh", "ssh_docker_logs", "容器日志"),
        ("ssh", "ssh_edit_open", "打开远程文件（下载内容）"),
        ("ssh", "ssh_edit_save", "保存远程文件（上传回写）"),
        ("ssh", "ssh_file_chmod", "修改远程权限（含安全策略）"),
        ("ssh", "ssh_file_create", "新建远程空文件"),
        ("ssh", "ssh_file_delete", "删除远程文件/目录"),
        ("ssh", "ssh_file_download", "下载文件（进度事件推送）"),
        ("ssh", "ssh_file_download_recursive", "递归下载远程目录"),
        ("ssh", "ssh_file_list", "远程目录列表（SFTP）"),
        ("ssh", "ssh_file_mkdir", "新建远程目录"),
        ("ssh", "ssh_file_rename", "重命名远程文件/目录"),
        ("ssh", "ssh_file_upload", "上传文件（进度事件推送）"),
        ("ssh", "ssh_group_delete", "删除分组（组内配置移回未分组）"),
        ("ssh", "ssh_group_list", "服务器分组列表"),
        ("ssh", "ssh_group_save", "新增/更新分组"),
        (
            "ssh",
            "ssh_host_key_respond",
            "应答主机密钥确认（首连/指纹变更）",
        ),
        ("ssh", "ssh_known_host_delete", "删除已知主机条目"),
        (
            "ssh",
            "ssh_known_host_list",
            "已知主机列表（含 SHA256 指纹）",
        ),
        ("ssh", "ssh_local_create", "本地新建文件/目录"),
        ("ssh", "ssh_local_delete", "本地删除文件/目录"),
        ("ssh", "ssh_local_list", "本地目录列表（双栏文件管理）"),
        ("ssh", "ssh_local_rename", "本地重命名/移动"),
        (
            "ssh",
            "ssh_monitor_get",
            "资源监控数据（CPU/内存/磁盘/网络）",
        ),
        ("ssh", "ssh_process_kill", "结束进程"),
        ("ssh", "ssh_process_list", "进程列表"),
        ("ssh", "ssh_profile_delete", "删除服务器配置"),
        (
            "ssh",
            "ssh_profile_import",
            "导入存量服务器配置与旧手工凭证（同 id upsert，幂等）",
        ),
        ("ssh", "ssh_profile_list", "服务器配置列表"),
        (
            "ssh",
            "ssh_profile_save",
            "新增/更新服务器配置（凭证入 Vault）",
        ),
        ("ssh", "ssh_reconnect", "重新连接（新会话替换旧会话）"),
        ("ssh", "ssh_service_action", "服务启动/停止/重启"),
        ("ssh", "ssh_service_list", "systemd 服务列表"),
        ("ssh", "ssh_service_logs", "服务日志（journalctl）"),
        ("ssh", "ssh_system_info_get", "远程系统信息与磁盘分区明细"),
        ("ssh", "ssh_terminal_close", "关闭终端通道"),
        ("ssh", "ssh_terminal_list", "某连接下的全部终端会话"),
        (
            "ssh",
            "ssh_terminal_log_start",
            "开始录制终端日志（幂等，已在录制则返回当前文件）",
        ),
        (
            "ssh",
            "ssh_terminal_log_stop",
            "停止录制并返回日志路径与字节数",
        ),
        ("ssh", "ssh_terminal_open", "打开 PTY 终端通道（xterm）"),
        ("ssh", "ssh_terminal_resize", "调整终端窗口大小"),
        ("ssh", "ssh_terminal_write", "写入终端数据（键盘输入）"),
        ("ssh", "ssh_transfer_cancel", "取消传输任务"),
        ("ssh", "ssh_tunnel_delete", "删除隧道配置"),
        ("ssh", "ssh_tunnel_list", "某服务器的隧道配置列表"),
        ("ssh", "ssh_tunnel_save", "新增/更新隧道配置"),
        ("ssh", "ssh_tunnel_start", "启动隧道（绑定到指定连接）"),
        ("ssh", "ssh_tunnel_stop", "停止隧道"),
        ("ssh", "ssh_tunnels", "某连接下的隧道运行时快照"),
        (
            "tts",
            "tts_synthesize",
            "合成语音（文本 + 语音 + 语速/音调 → mp3 文件）",
        ),
        ("tts", "tts_voices", "获取文字转语音可选语音列表"),
    ];

    /// 源码树中的 `#[tauri::command]` 实现（名称, 文件）
    fn scan_command_fns() -> Vec<(String, PathBuf)> {
        let mut found = Vec::new();
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        for dir in ["plugins", "framework"] {
            walk(&src.join(dir), &mut found);
        }
        found.sort();
        found
    }

    /// 递归收集目录下的 Rust 文件里的命令实现
    fn walk(dir: &Path, out: &mut Vec<(String, PathBuf)>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                scan_file(&path, out);
            }
        }
    }

    /// 解析单文件：收集命令函数，跳过 `#[cfg(test)]` 模块（测试里的假命令不算入口）
    fn scan_file(path: &Path, out: &mut Vec<(String, PathBuf)>) {
        let Ok(src) = std::fs::read_to_string(path) else {
            return;
        };
        let Ok(file) = syn::parse_file(&src) else {
            return;
        };
        collect_items(&file.items, path, out);
    }

    /// 递归遍历 item：函数看 `#[tauri::command]`，模块除 `#[cfg(test)]` 外继续下钻
    fn collect_items(items: &[syn::Item], path: &Path, out: &mut Vec<(String, PathBuf)>) {
        for item in items {
            match item {
                syn::Item::Fn(f) => {
                    if has_attr(&f.attrs, "tauri::command") && !has_attr(&f.attrs, "cfg") {
                        out.push((f.sig.ident.to_string(), path.to_path_buf()));
                    }
                }
                syn::Item::Mod(m) => {
                    if has_attr(&m.attrs, "cfg") {
                        continue;
                    }
                    if let Some((_, inner)) = &m.content {
                        collect_items(inner, path, out);
                    }
                }
                _ => {}
            }
        }
    }

    /// 属性路径匹配（`#[cfg]` 与 `#[tauri::command]` 都走这里）
    fn has_attr(attrs: &[syn::Attribute], wanted: &str) -> bool {
        attrs.iter().any(|attr| {
            attr.path()
                .segments
                .iter()
                .map(|seg| seg.ident.to_string())
                .collect::<Vec<_>>()
                .join("::")
                == wanted
        })
    }

    /// 契约 1：登记元数据 = 已发布命令表（插件侧）
    #[test]
    fn plugin_command_table_matches_published() {
        let _guard = lock_registry();
        reset_all();
        crate::plugins::register_ipc_entries();
        let mut actual: Vec<_> = ipc_registry::snapshot()
            .into_iter()
            .filter(|e| e.owner != "framework")
            .map(|e| (e.owner, e.name, e.doc))
            .collect();
        actual.sort();
        let mut expected: Vec<_> = PUBLISHED
            .iter()
            .filter(|(owner, _, _)| *owner != "framework")
            .copied()
            .collect();
        expected.sort();
        assert_eq!(actual, expected, "登记元数据与已发布命令表不一致");
    }

    /// 契约 1b：框架命令声明 = 已发布命令表（框架侧）
    #[test]
    fn framework_command_table_matches_published() {
        let mut actual: Vec<_> = crate::framework::IPC_ENTRIES
            .iter()
            .map(|(name, doc)| ("framework", *name, *doc))
            .collect();
        actual.sort();
        let mut expected: Vec<_> = PUBLISHED
            .iter()
            .filter(|(owner, _, _)| *owner == "framework")
            .copied()
            .collect();
        expected.sort();
        assert_eq!(actual, expected, "框架命令声明与已发布命令表不一致");
    }

    /// 契约 2：全部登记命令都能路由（跑真实启动校验路径）
    #[test]
    fn registered_commands_are_routable() {
        let _guard = lock_registry();
        reset_all();
        crate::plugins::register_ipc_entries();
        crate::framework::register_ipc().expect("框架命令登记");
        crate::plugins::validate_routing();
        for module in module_manifest::modules() {
            assert!(
                module.owner == "framework" || crate::plugins::is_routed(module.owner),
                "模块 {} 未纳入路由清单",
                module.module_path
            );
        }
    }

    /// 契约 2a：登记的命令与模块清单逐条对应（少入库 / 清单漏写都要失败）
    #[test]
    fn registered_commands_match_module_declarations() {
        let _guard = lock_registry();
        reset_all();
        crate::plugins::register_ipc_entries();
        crate::framework::register_ipc().expect("框架命令登记");
        module_manifest::validate_command_declarations();
    }

    /// 契约 2a-2：故意少登记一条命令时必须报错（证明校验不是只比 owner 名字）
    #[test]
    #[should_panic(expected = "不一致")]
    fn missing_registered_command_fails_validation() {
        let _guard = lock_registry();
        reset_all();
        crate::plugins::register_ipc_entries();
        crate::framework::register_ipc().expect("框架命令登记");
        // 手动补一条「清单里有、注册表里没有」的命令，模拟漏登记
        let owner = module_manifest::modules()
            .iter()
            .find(|module| module.owner == "framework")
            .map(|module| module.owner)
            .expect("框架模块存在");
        let diff = module_manifest::declaration_diff(
            &[(owner, "storage_info")],
            &[(owner, "storage_other")],
        );
        assert!(!diff.is_empty(), "构造的差异必须非空");
        panic!("IPC 命令清单与注册表不一致（{} 处）", diff.len());
    }

    /// 契约 2b：未纳路由清单的 owner 必须在启动校验时炸掉
    #[test]
    #[should_panic(expected = "没有路由分支")]
    fn unknown_owner_fails_startup_validation() {
        let _guard = lock_registry();
        reset_all();
        ipc_registry::register("ghost", &[("ghost_ping", "幽灵命令")]).expect("登记");
        crate::plugins::validate_routing();
    }

    /// 契约 2c：重复注册直接报错（同名命令两个归属者 = 路由归谁不确定）
    #[test]
    fn duplicate_command_registration_is_rejected() {
        let _guard = lock_registry();
        reset_all();
        ipc_registry::register("alpha", &[("dup_cmd", "甲")]).expect("首次登记");
        let err = ipc_registry::register("beta", &[("dup_cmd", "乙")]).expect_err("应报重复");
        assert!(err.contains("dup_cmd"), "错误信息应含冲突命令名：{err}");
    }

    /// 契约 3：源码里每个 `#[tauri::command]` 都必须在清单里（手写绕过入口的守卫）
    #[test]
    fn every_tauri_command_is_declared() {
        let declared: Vec<&str> = PUBLISHED.iter().map(|(_, name, _)| *name).collect();
        let scan = scan_command_fns();
        assert!(
            scan.len() >= 100,
            "源码扫描到的命令数异常（{}），守卫可能失效",
            scan.len()
        );
        for (name, file) in &scan {
            assert!(
                declared.contains(&name.as_str()),
                "{} 实现了命令 {name}，但没有在模块清单里声明（未入库命令无法被前端调用）",
                file.display()
            );
        }
        // 反向：清单里的每个命令名都必须真的存在实现
        for (_, name, _) in PUBLISHED {
            assert!(
                scan.iter().any(|(found, _)| found == name),
                "清单声明了命令 {name}，但源码里找不到 #[tauri::command] 实现"
            );
        }
    }

    /// 契约 4：模块描述完整（featureId/owner 同形、命令非空、owner 不重复）
    #[test]
    fn module_specs_are_complete() {
        let _guard = lock_registry();
        reset_all();
        crate::plugins::register_ipc_entries();
        let modules = module_manifest::modules();
        assert_eq!(
            modules.len(),
            crate::plugins::ROUTABLE_OWNERS.len(),
            "模块描述数量与路由清单模块数不一致"
        );
        let mut owners: Vec<&str> = modules.iter().map(|m| m.owner).collect();
        owners.sort();
        let mut routed = crate::plugins::ROUTABLE_OWNERS.to_vec();
        routed.sort();
        assert_eq!(owners, routed, "模块描述与路由清单的 owner 集合不一致");
        for module in &modules {
            assert!(!module.commands.is_empty(), "{} 命令清单为空", module.owner);
            assert!(
                module.module_path.contains(module.owner),
                "MODULE 记录的 Rust 模块路径 {} 与 owner {} 不一致",
                module.module_path,
                module.owner
            );
        }
    }
    /// 模块登记幂等：同一 owner 重复登记不再入表（模块表是进程级单例，必须可重置）
    #[test]
    fn module_registration_is_idempotent() {
        let _guard = lock_registry();
        reset_all();
        static SPEC: module_manifest::ModuleSpec = module_manifest::ModuleSpec {
            owner: "__test_owner__",
            feature_id: "__test_owner__",
            module_path: "test::__test_owner__",
            storage_key: None,
            commands: &[("__test_cmd__", "测试命令")],
        };
        module_manifest::register_module(&SPEC).expect("首次登记应成功");
        module_manifest::register_module(&SPEC).expect("重复登记应成功但不再入表");
        let hits = module_manifest::modules()
            .iter()
            .filter(|m| m.owner == "__test_owner__")
            .count();
        assert_eq!(hits, 1, "同一 owner 只应入表一次");
    }
    /// 兼容别名：`impl::path as "别名"` 时注册名用别名，否则取实现名末段；
    /// 别名是唯一允许注册名 ≠ 实现名的入口，且由已发布命令表把关（写错即失败）
    #[test]
    fn ipc_name_alias_is_explicit() {
        assert_eq!(
            crate::patchybox_ipc_name!(crate::plugins::tts::tts_voices),
            "tts_voices"
        );
        assert_eq!(
            crate::patchybox_ipc_name!(crate::plugins::tts::tts_voices, "voices_legacy"),
            "voices_legacy"
        );
    }

    /// 未纳清单的 owner：路由返回 None（走明确报错），不是静默返回 false 吞掉
    #[test]
    fn unrouted_owner_is_not_silently_accepted() {
        assert!(!crate::plugins::is_routed("ghost"));
        assert!(crate::plugins::route_owner("ghost").is_none());
        let _guard = lock_registry();
        reset_all();
        crate::plugins::register_ipc_entries();
        for module in module_manifest::modules() {
            assert!(
                crate::plugins::route_owner(module.owner).is_some() || module.owner == "framework",
                "模块 {} 没有路由分支",
                module.owner
            );
        }
    }

    /// 已登记但无路由分支 → 明确 panic（防御性入口的负例）
    #[test]
    #[should_panic(expected = "没有路由分支")]
    fn unrouted_owner_panics() {
        module_manifest::unrouted_owner("ghost");
    }

    /// 历史存储键冻结：HTTP/WS 的接口库仍是 data/api.db，owner/feature 迁移不改物理库名（§10.1）
    #[test]
    fn http_ws_keeps_legacy_api_storage_key() {
        assert_eq!(crate::plugins::http_ws::MODULE.owner, "http_ws");
        assert_eq!(crate::plugins::http_ws::MODULE.feature_id, "http-ws");
        assert_eq!(crate::plugins::http_ws::MODULE.storage_key, Some("api"));
    }

    /// AR07 ②：新增模块的改动面就是两处——模块门面写一份 `patchybox_module!`，
    /// 再在 plugins/mod.rs 的 `patchybox_routes!` 加一行。这里从源码直接锁定两份声明
    /// 一一对应：只写了模块自己、忘加清单行的模块**不会被装配**，而运行期登记表本身
    /// 由清单生成（模块数 == 清单长度恒真），抓不住这种漏写，只能靠源码比对。
    #[test]
    fn route_manifest_covers_every_declared_module() {
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut declared = Vec::new();
        collect_module_declarations(&src.join("plugins"), &mut declared);
        collect_module_declarations(&src.join("framework"), &mut declared);
        declared.sort();
        declared.dedup();
        assert!(
            declared.len() >= 8,
            "源码扫描到的模块声明数异常（{}），守卫可能已失效",
            declared.len()
        );

        // framework 不属于业务插件、不参与插件路由，其余声明必须与清单逐一对上
        let mut expected: Vec<String> = declared
            .iter()
            .filter(|owner| owner.as_str() != "framework")
            .cloned()
            .collect();
        expected.sort();
        let routed = route_manifest_owners(&src.join("plugins").join("mod.rs"));
        assert_eq!(
            expected, routed,
            "模块声明与路由清单不一致：漏写清单的模块不会被装配，前端调用只会得到 command not found"
        );

        // 清单行必须真的解析到 handler，不能是死行
        for owner in &routed {
            assert!(
                crate::plugins::route_owner(owner).is_some(),
                "路由清单里的 {owner} 没有对应 handler"
            );
        }
    }

    /// 扫描目录下所有 Rust 文件里的 `patchybox_module!` 声明，收集 owner
    fn collect_module_declarations(dir: &Path, out: &mut Vec<String>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_module_declarations(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                let Ok(text) = std::fs::read_to_string(&path) else {
                    continue;
                };
                let Ok(file) = syn::parse_file(&text) else {
                    continue;
                };
                collect_module_macro_owners(&file.items, out);
            }
        }
    }

    /// 递归（含内联模块）找 `patchybox_module!` 调用并取 owner 字面量
    fn collect_module_macro_owners(items: &[syn::Item], out: &mut Vec<String>) {
        for item in items {
            match item {
                syn::Item::Macro(m) => {
                    if m.mac
                        .path
                        .segments
                        .last()
                        .is_some_and(|seg| seg.ident == "patchybox_module")
                    {
                        if let Some(owner) = owner_literal(&m.mac.tokens.to_string()) {
                            out.push(owner);
                        }
                    }
                }
                syn::Item::Mod(m) => {
                    if let Some((_, inner)) = &m.content {
                        collect_module_macro_owners(inner, out);
                    }
                }
                _ => {}
            }
        }
    }

    /// 从模块声明的 token 文本里取 `owner: "..."` 的值
    fn owner_literal(tokens_text: &str) -> Option<String> {
        let (_, after_key) = tokens_text.split_once("owner")?;
        let (_, after_open) = after_key.split_once('"')?;
        let (value, _) = after_open.split_once('"')?;
        Some(value.to_string())
    }

    /// 取 `plugins/mod.rs` 里 `patchybox_routes!` 清单的全部 owner 字面量（排序后）
    fn route_manifest_owners(path: &Path) -> Vec<String> {
        let Ok(text) = std::fs::read_to_string(path) else {
            return Vec::new();
        };
        let Ok(file) = syn::parse_file(&text) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for item in &file.items {
            if let syn::Item::Macro(m) = item {
                if m.mac
                    .path
                    .segments
                    .last()
                    .is_some_and(|seg| seg.ident == "patchybox_routes")
                {
                    let tokens = m.mac.tokens.to_string();
                    out.extend(tokens.split('"').skip(1).step_by(2).map(str::to_string));
                }
            }
        }
        out.sort();
        out
    }

    /// AR07 ②（负例）：模块声明了自己却没进路由清单，启动校验必须炸掉，
    /// 不能静默不装配——这是「新增模块只改两处」的漏改守卫。
    #[test]
    #[should_panic(expected = "没有路由分支")]
    fn module_missing_from_route_manifest_fails_startup_validation() {
        let _guard = lock_registry();
        reset_all();
        static SPEC: module_manifest::ModuleSpec = module_manifest::ModuleSpec {
            owner: "__ghost_module__",
            feature_id: "__ghost_module__",
            module_path: "test::__ghost_module__",
            storage_key: None,
            commands: &[("__ghost_cmd__", "未进路由清单的模块命令")],
        };
        module_manifest::register_module(&SPEC).expect("登记幽灵模块");
        ipc_registry::register(
            "__ghost_module__",
            &[("__ghost_cmd__", "未进路由清单的模块命令")],
        )
        .expect("登记幽灵命令");
        crate::plugins::validate_routing();
    }
}
