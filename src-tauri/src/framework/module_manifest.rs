//! 静态模块清单（AR07 §10.2）
//!
//! 一个模块的 owner、前端 featureId、数据存储键与命令清单**只声明一处**，由宏同时生成
//! IPC 入库元数据（名称 + 中文说明）与命令 handler：命令名取自与 handler 相同的函数路径
//! 标识符（`stringify!` 末段），不再存在「实现改了、登记表忘改」的独立编辑来源。
//!
//! 模块内声明（放在模块门面文件里，例如 plugins/http_ws/mod.rs）：
//!
//! ```ignore
//! crate::covekit_module! {
//!     owner: "http_ws",
//!     feature: "http-ws",
//!     storage: "api",                                  // 可选：数据文件 <root>/data/api.db
//!     commands: {
//!         http::http_request => "发送 HTTP 请求",
//!         persistence::api_save => "保存/更新接口（id=0 新增）",
//!     },
//! }
//! ```
//!
//! 生成物：`IPC_OWNER` / `FEATURE_ID` / `IPC_ENTRIES` / `MODULE` / `register_ipc()` / `invoke_handler()`。
//! 路由与装配清单由 `plugins/mod.rs` 的 `covekit_routes!` 声明，两份清单共用同一批 owner 字面量，
//! 启动校验据此判定「登记了命令但没有路由分支」的死命令。

use std::sync::Mutex;

/// 模块静态描述（启动校验、契约测试与文档生成的数据源）
pub struct ModuleSpec {
    /// 归属者 id：插件 id（与 plugins/<id>/ 目录同名）或 "framework"
    pub owner: &'static str,
    /// 前端稳定 feature id（与 src/plugins/<id>/manifest 一致；产品侧改名不牵动此处）
    pub feature_id: &'static str,
    /// Rust 模块路径（由 `module_path!()` 生成，不手写）
    pub module_path: &'static str,
    /// 数据文件存储键（`<storageRoot>/data/<key>.db`）；None = 本模块不落库
    pub storage_key: Option<&'static str>,
    /// 命令清单：(IPC 名称, 中文说明)
    pub commands: &'static [(&'static str, &'static str)],
}

/// 已登记的模块描述（进程内一次性登记，模块 register() 时写入）
static MODULE_TABLE: Mutex<Vec<&'static ModuleSpec>> = Mutex::new(Vec::new());

/// 登记一个模块描述；同一 owner 只保留首次（重复装配时不重复入表）
pub fn register_module(spec: &'static ModuleSpec) -> Result<(), String> {
    let mut table = MODULE_TABLE.lock().map_err(|e| e.to_string())?;
    if table.iter().any(|m| m.owner == spec.owner) {
        return Ok(());
    }
    table.push(spec);
    Ok(())
}

/// 命令入库（重复注册 = 编码错误，直接 fail-fast；装配期唯一的 panic 入口）
///
/// 同名命令被两个模块登记属装配错误，早炸比运行期静默覆盖好（docs/05 §1 例外）。
pub(crate) fn register_ipc_or_fail(spec: &'static ModuleSpec) {
    register_module(spec).expect("模块描述表不可用（锁 poisoned）");
    crate::framework::ipc_registry::register(spec.owner, spec.commands).expect("IPC 命令重复注册");
}

/// 清空模块描述表（仅测试：表是进程级单例，用例之间不能互相残留）
#[cfg(test)]
pub(crate) fn reset_modules() {
    if let Ok(mut guard) = MODULE_TABLE.lock() {
        guard.clear();
    }
}

/// 已登记却没有路由分支的 owner：装配与清单漂移，明确报错而非静默当成命令不存在。
///
/// 启动校验会先拦下这种状态，因此本函数只在清单被绕过时触发（防御性入口，有负例测试）。
pub(crate) fn unrouted_owner(owner: &str) -> ! {
    panic!("IPC 归属者 {owner} 已登记命令但没有路由分支（plugins/mod.rs 清单缺该模块）");
}

/// 命令级一致性比较（纯函数，便于单测）：返回差异描述清单
///
/// 两侧都按 (owner, 命令名) 比较：
/// - 模块清单里声明了、注册表里没有 → 声明了却没实现（启动后调用必然失败）
/// - 注册表里有、模块清单里没声明 → 实现了却没入库（清单漏写，路由与文档都会对不上）
pub(crate) fn declaration_diff(
    declared: &[(&str, &str)],
    registered: &[(&str, &str)],
) -> Vec<String> {
    use std::collections::BTreeSet;
    let declared: BTreeSet<(&str, &str)> = declared.iter().copied().collect();
    let registered: BTreeSet<(&str, &str)> = registered.iter().copied().collect();
    let mut diff = Vec::new();
    for (owner, name) in declared.difference(&registered) {
        diff.push(format!(
            "命令 {owner}::{name} 已声明但未登记（清单有、注册表无）"
        ));
    }
    for (owner, name) in registered.difference(&declared) {
        diff.push(format!(
            "命令 {owner}::{name} 已登记但清单未声明（实现了却没入库）"
        ));
    }
    diff.sort();
    diff
}

/// 命令级校验（T12-1）：登记的命令与模块清单逐条对应，少一条就 fail-fast
///
/// 只校验归属者名字出现在路由清单里是不够的：命令少入库或清单漏写时，
/// owner 级校验仍然通过，问题会推迟到运行期变成 404。
pub(crate) fn validate_command_declarations() {
    let declared: Vec<(&str, &str)> = modules()
        .iter()
        .flat_map(|module| {
            module
                .commands
                .iter()
                .map(move |(name, _doc)| (module.owner, *name))
        })
        .collect();
    let registered: Vec<(&str, &str)> = crate::framework::ipc_registry::snapshot()
        .iter()
        .map(|entry| (entry.owner, entry.name))
        .collect();
    let diff = declaration_diff(&declared, &registered);
    assert!(
        diff.is_empty(),
        "IPC 命令清单与注册表不一致（{} 处）：
{}",
        diff.len(),
        diff.join(
            "
"
        )
    );
}

/// 已登记模块（顺序 = 登记顺序），供启动校验与契约测试遍历
pub fn modules() -> Vec<&'static ModuleSpec> {
    MODULE_TABLE.lock().map(|t| t.clone()).unwrap_or_default()
}

/// 命令路径取最后一段作为 IPC 名（编译期展开，避免手写字符串与实现脱节）
#[macro_export]
macro_rules! covekit_last_segment {
    ($single:ident) => {
        stringify!($single)
    };
    ($head:ident :: $($rest:tt)+) => {
        $crate::covekit_last_segment!($($rest)+)
    };
}

/// IPC 名称解析：默认取实现路径末段；需要兼容别名时显式写 `impl::path as "别名"`。
///
/// 别名是唯一允许「注册名 ≠ 实现名」的入口，且必须在清单里写明（禁止靠字符串替换猜测），
/// 契约测试按已发布命令表比对，别名写错即失败。
#[macro_export]
macro_rules! covekit_ipc_name {
    ($($segment:ident)::+, $alias:literal) => {
        $alias
    };
    ($($segment:ident)::+) => {
        $crate::covekit_last_segment!($($segment)::+)
    };
}

/// 可选字面量 → `Option<&str>`（供模块清单的 storage 字段使用）
#[macro_export]
macro_rules! covekit_optional_str {
    () => {
        ::core::option::Option::None
    };
    ($value:literal) => {
        ::core::option::Option::Some($value)
    };
}

/// 模块静态清单宏：生成 IPC 入库元数据 + 命令 handler + 模块描述
#[macro_export]
macro_rules! covekit_module {
    (
        owner: $owner:literal,
        feature: $feature:literal,
        $( storage: $storage:literal, )?
        commands: { $( $( $segment:ident )::+ $( as $alias:literal )? => $doc:literal ),* $(,)? }
        $(,)?
    ) => {
        /// 归属者 id（本模块 IPC 命令的 owner，用于路由与登记）
        pub(crate) const IPC_OWNER: &str = $owner;

        /// 前端稳定 feature id（与前端同 id 插件对应）
        pub(crate) const FEATURE_ID: &str = $feature;

        /// 命令清单：(IPC 名称, 中文说明)。名称与 handler 取自同一函数路径标识符，
        /// 兼容别名用 `impl::path as "别名"` 显式声明。
        pub(crate) const IPC_ENTRIES: &[(&str, &str)] = &[
            $( ($crate::covekit_ipc_name!($($segment)::+ $(, $alias)?), $doc), )*
        ];

        /// 本模块静态描述（启动校验与契约测试读取）
        pub(crate) const MODULE: $crate::framework::module_manifest::ModuleSpec =
            $crate::framework::module_manifest::ModuleSpec {
                owner: IPC_OWNER,
                feature_id: FEATURE_ID,
                module_path: module_path!(),
                storage_key: $crate::covekit_optional_str!($($storage)?),
                commands: IPC_ENTRIES,
            };

        /// 登记本模块描述与命令（重复注册返回 Err，供契约测试断言）
        #[cfg(test)]
        pub(crate) fn register_ipc() -> Result<(), String> {
            $crate::framework::module_manifest::register_module(&MODULE)?;
            $crate::framework::ipc_registry::register(IPC_OWNER, IPC_ENTRIES)
        }

        /// 登记本模块描述与命令并 fail-fast（装配期调用；重复注册属编码错误）
        pub(crate) fn register_ipc_or_fail() {
            $crate::framework::module_manifest::register_ipc_or_fail(&MODULE)
        }

        /// 分派本模块命令（handler 引用与 IPC_ENTRIES 同源）
        pub(crate) fn invoke_handler(
            invoke: tauri::ipc::Invoke<tauri::Wry>,
        ) -> bool {
            let handler: fn(tauri::ipc::Invoke<tauri::Wry>) -> bool =
                tauri::generate_handler![ $( $( $segment )::+ ),* ];
            handler(invoke)
        }
    };
}

/// 路由与装配清单宏：一条 `owner => 模块` 生成路由分支、装配顺序与启动校验
#[macro_export]
macro_rules! covekit_routes {
    ( $( $owner:literal => $module:ident ),* $(,)? ) => {
        /// 有路由分支的 owner 清单（路由、装配与启动校验的唯一数据源）
        pub(crate) const ROUTABLE_OWNERS: &[&str] = &[ $( $owner ),* ];

        /// owner 是否已纳入路由清单
        pub(crate) fn is_routed(owner: &str) -> bool {
            ROUTABLE_OWNERS.contains(&owner)
        }

        /// owner → 模块 handler（路由判定的唯一可测试入口）
        pub(crate) fn route_owner(
            owner: &str,
        ) -> ::core::option::Option<fn(tauri::ipc::Invoke<tauri::Wry>) -> bool> {
            match owner {
                $( $owner => ::core::option::Option::Some($module::invoke_handler), )*
                _ => ::core::option::Option::None,
            }
        }

        /// 按注册表 owner 分派到模块 handler。
        ///
        /// 已登记却没有路由分支的 owner 属于清单与登记表漂移：启动校验（validate_routing）
        /// 会先拦下，这里是防御性明确报错，不再返回 false 让上层当成「命令不存在」吞掉。
        /// 未入库命令返回 false，交应用级 handler 处理（框架命令与官方插件命令即走此路）。
        pub(crate) fn invoke_handler(invoke: tauri::ipc::Invoke<tauri::Wry>) -> bool {
            match $crate::framework::ipc_registry::owner_of(invoke.message.command()) {
                ::core::option::Option::Some(owner) => match route_owner(owner) {
                    ::core::option::Option::Some(handler) => handler(invoke),
                    ::core::option::Option::None => {
                        $crate::framework::module_manifest::unrouted_owner(owner)
                    }
                },
                ::core::option::Option::None => false,
            }
        }

        /// 按声明顺序装配全部模块（每模块一行 register）
        pub(crate) fn register_all(
            builder: tauri::Builder<tauri::Wry>,
        ) -> tauri::Builder<tauri::Wry> {
            $( let builder = $module::register(builder); )*
            builder
        }

        /// 登记全部模块命令（契约测试用；生产路径由各模块 register 自行登记）
        #[cfg(test)]
        pub(crate) fn register_ipc_entries() {
            $( $module::register_ipc().expect("IPC 命令重复注册"); )*
        }

        /// 启动校验（fail-fast）：
        /// 1. 清单内模块的 owner 必须都有路由分支；
        /// 2. 已登记的模块命令其 owner 必须在路由清单里（登记了却没分支 = 运行期静默 404）。
        pub(crate) fn validate_routing() {
            for module in $crate::framework::module_manifest::modules() {
                // 插件模块的前端 feature id 与 Rust owner 同形（仅连字符与下划线写法不同），
                // 两者漂移会让前端契约对不上，启动即暴露；framework 不属于业务插件，豁免。
                if module.owner != "framework" {
                    assert_eq!(
                        module.feature_id.replace('-', "_"),
                        module.owner,
                        "模块 {} 的 featureId {} 与 owner 不一致",
                        module.module_path,
                        module.feature_id
                    );
                }
                assert!(
                    !module.commands.is_empty(),
                    "模块 {} 未声明任何命令（清单漏写？）",
                    module.module_path
                );
                // 存储键是 <root>/data/<key>.db 的文件名主干，禁止路径分隔符与扩展名
                if let Some(key) = module.storage_key {
                    assert!(
                        !key.contains(['/', '\\', '.']),
                        "模块 {} 的存储键 {key} 不合法（只能是文件名主干）",
                        module.module_path
                    );
                }
                if module.owner == "framework" {
                    continue;
                }
                if !is_routed(module.owner) {
                    panic!("模块 {} 的命令没有路由分支", module.owner);
                }
            }
            for entry in $crate::framework::ipc_registry::snapshot() {
                if entry.owner == "framework" {
                    continue;
                }
                assert!(
                    is_routed(entry.owner),
                    "IPC 命令 {} 的归属者 {} 没有路由分支（plugins/mod.rs 清单缺该模块）",
                    entry.name,
                    entry.owner
                );
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::declaration_diff;

    /// 命令级比较：只差一条也能被抓出来（两边方向都要覆盖）
    #[test]
    fn declaration_diff_reports_both_directions() {
        let declared = [("ssh", "ssh_list"), ("dns", "dns_config_get")];
        let registered = [("ssh", "ssh_list"), ("dns", "dns_config_set")];
        let diff = declaration_diff(&declared, &registered);
        assert_eq!(diff.len(), 2, "{diff:?}");
        assert!(diff.iter().any(|line| line.contains("dns::dns_config_get")));
        assert!(diff.iter().any(|line| line.contains("dns::dns_config_set")));
        assert!(declaration_diff(&declared, &declared).is_empty());
    }

    /// 路径末段提取：单段路径与多段路径都应取到函数名
    #[test]
    fn last_segment_extracts_function_name() {
        assert_eq!(covekit_last_segment!(foo), "foo");
        assert_eq!(
            covekit_last_segment!(crate::plugins::http_ws::http::http_request),
            "http_request"
        );
    }

    /// 可选存储键：缺省与显式两条路径
    #[test]
    fn optional_storage_key() {
        let missing: Option<&str> = covekit_optional_str!();
        assert_eq!(missing, None);
        assert_eq!(covekit_optional_str!("api"), Some("api"));
    }
}
