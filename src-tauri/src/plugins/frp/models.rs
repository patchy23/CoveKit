//! frp 插件 · serde 数据结构（与前端 src/plugins/frp/contracts.ts 逐字段同步）
//! 传输契约与 src/plugins/frp/contracts.ts 保持一致，命令名与字段名两侧同步。
//! 约定：Rust 侧 snake_case + `rename_all = "camelCase"`；可选字段缺省时不序列化（对应前端 `field?`）。

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// 档案运行状态取值（契约字面量：stopped / starting / running / error）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FrpStateName {
    /// 未运行（初始态；进程退出码 0 也回到此态）
    #[default]
    Stopped,
    /// 进程已拉起，等待「连接成功」类日志
    Starting,
    /// 已连接服务端并运行中
    Running,
    /// 启动失败或运行中掉线（原因见 last_error）
    Error,
}

/// frpc 可执行文件来源（契约字面量：settings / path / common / downloaded）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FrpBinarySource {
    /// 来自工具设置项 frpcPath
    Settings,
    /// 来自系统 PATH 扫描
    Path,
    /// 来自常见安装位置探测
    Common,
    /// 由工具一键下载并落盘
    Downloaded,
}

/// 日志流来源（契约字面量：stdout / stderr）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FrpLogStream {
    /// 标准输出
    Stdout,
    /// 标准错误
    Stderr,
}

/// 下载阶段（契约字面量：download / extract / verify / done）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FrpDownloadPhase {
    /// 正在下载压缩包
    Download,
    /// 正在解压
    Extract,
    /// 正在校验 SHA256
    Verify,
    /// 已完成（含落盘与 chmod）
    Done,
}

/// 档案摘要（列表项；元数据来自 DB，统计来自 TOML 解析）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrpProfileSummary {
    /// 档案文件名（单段文件名，.toml 结尾）
    pub file_name: String,
    /// 展示名（DB display_name；缺省为文件名去扩展名）
    pub display_name: String,
    /// 备注（DB remark；不污染用户 TOML）
    pub remark: String,
    /// 服务器地址（serverAddr；解析失败为空串）
    pub server_addr: String,
    /// 服务器端口（serverPort；解析失败为 0）
    pub server_port: u16,
    /// 代理条目总数
    pub proxy_count: u32,
    /// 启用中的代理数（enabled != false）
    pub enabled_proxy_count: u32,
    /// 文件修改时间（Unix 毫秒）
    pub mtime: i64,
    /// 绑定的客户端 id（不出现 = 跟随默认客户端）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,
    /// 当前运行状态
    pub state: FrpStateName,
    /// 运行中的进程 id（未运行时不出现）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    /// 最近一次错误原因（已脱敏；正常时不出现）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
}

/// 档案列表返回
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrpProfileList {
    /// 是否成功（false 时 error 说明原因）
    pub ok: bool,
    /// 实际使用的配置目录绝对路径（前端展示与排障用）
    pub dir: String,
    /// 档案摘要列表
    pub profiles: Vec<FrpProfileSummary>,
    /// 失败原因（ok=false 时出现）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// 档案内容返回（源码模式原文 + TOML→JSON 解析结果）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrpProfileContent {
    /// 是否成功
    pub ok: bool,
    /// 档案文件名
    pub file_name: String,
    /// 原文（原样，不做任何改写）
    pub content: String,
    /// TOML 解析结果（未知段落全量透传）
    pub parsed: Value,
    /// 原文是否含注释（表单保存会丢注释，前端据此二次确认）
    pub has_comments: bool,
    /// 失败原因（ok=false 时出现）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// 写操作统一返回（新建 / 复制 / 重命名 / 删除 / 保存 / 备注）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrpOpResult {
    /// 是否成功
    pub ok: bool,
    /// 涉及的档案文件名（成功时出现）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_name: Option<String>,
    /// 失败原因（ok=false 时出现）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// 单条校验错误
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrpVerifyError {
    /// 出错行号（解析不出时不出现）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    /// 出错列号（解析不出时不出现）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<u32>,
    /// 错误信息（原文，已剥 ANSI 与时间戳前缀）
    pub message: String,
}

/// 校验结果（frpc verify -c <path>）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrpVerifyResult {
    /// 是否校验通过（frpc 退出码为 0）
    pub ok: bool,
    /// 档案文件名
    pub file_name: String,
    /// frpc 输出（清理 ANSI 并脱敏，供用户判读）
    pub raw: String,
    /// 解析出的结构化错误列表（无法解析时降级为单条）
    pub errors: Vec<FrpVerifyError>,
}

/// 单个档案的运行状态
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrpRuntimeState {
    /// 档案文件名
    pub file_name: String,
    /// 当前状态
    pub state: FrpStateName,
    /// 进程 id（运行中才出现）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    /// 启动时刻（Unix 毫秒；未启动过则不出现）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<i64>,
    /// 最后一行日志（已脱敏；无日志则不出现）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_line: Option<String>,
    /// 最近一次错误原因（已脱敏；无错误则不出现）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
    /// 进程退出码（进程已结束后才出现）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
}

/// frpc 可执行文件信息
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrpBinaryInfo {
    /// 是否探测/下载成功
    pub ok: bool,
    /// 可执行文件绝对路径（成功时出现）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// 版本号（探测不到则为空，仅表示「可执行文件存在」）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// 来源（失败时不出现）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<FrpBinarySource>,
    /// 失败原因（ok=false 时出现）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// 客户端来源
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FrpClientSource {
    /// 工具内一键下载安装
    Download,
    /// 引用外部已有可执行文件（不复制文件，用户自编译产物可持续更新）
    External,
}

/// 已登记的一个 frpc 客户端
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrpClient {
    /// 稳定标识（路径哈希；档案绑定与移除都用它）
    pub id: String,
    /// 展示名（取自文件名）
    pub label: String,
    /// 可执行文件绝对路径
    pub path: String,
    /// 版本号（探测不到时为空，仅表示文件存在）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// 来源
    pub source: FrpClientSource,
    /// 是否为默认客户端
    pub is_default: bool,
    /// 文件当前是否仍然存在（外部引用可能被移走或改名）
    pub exists: bool,
}

/// 客户端清单（客户端管理弹窗与档案选择器的数据源）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrpClientList {
    /// 是否读取成功
    pub ok: bool,
    /// 已登记的客户端
    pub clients: Vec<FrpClient>,
    /// 默认客户端 id（无默认时为空）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_id: Option<String>,
    /// 失败原因（ok=false 时出现）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// 上游 Release 资产
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrpReleaseAsset {
    /// 资产文件名（如 frp_0.61.0_windows_amd64.zip）
    pub name: String,
    /// 资产字节数
    pub size: u64,
}

/// 上游 Release 条目（供下载选择）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrpReleaseInfo {
    /// 版本号（已去掉前缀 v）
    pub version: String,
    /// 发布时间（ISO8601 原文）
    pub published_at: String,
    /// 该 Release 的资产列表
    pub assets: Vec<FrpReleaseAsset>,
}

/// 事件 `frp://log` 负载（每行日志）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrpLogPayload {
    /// 档案文件名
    pub file_name: String,
    /// 日志行（已剥 ANSI、已脱敏）
    pub line: String,
    /// 时间戳（Unix 毫秒）
    pub ts: i64,
    /// 来源流
    pub stream: FrpLogStream,
}

/// 事件 `frp://download` 负载（下载进度）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrpDownloadPayload {
    /// 目标版本号
    pub version: String,
    /// 当前阶段
    pub phase: FrpDownloadPhase,
    /// 已接收字节数（download 阶段递增）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub received: Option<u64>,
    /// 总字节数（服务端未给 Content-Length 时不出现）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<u64>,
    /// 失败原因（仅失败时有值）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/* ── TOML ⇄ JSON 互转（表单模式的数据基础；未知段落与字段全量透传） ── */

/// TOML 原文 → JSON Value（`FrpProfileContent.parsed` 的数据源；解析失败给出中文原因）
pub fn toml_text_to_value(text: &str) -> Result<Value, String> {
    let value: toml::Value = toml::from_str(text).map_err(|e| format!("TOML 解析失败: {e}"))?;
    serde_json::to_value(&value).map_err(|e| format!("TOML 转 JSON 失败: {e}"))
}

/// JSON Value → TOML 原文（表单保存用；null 视为「字段已清空」应删除，与表单清空语义一致）
pub fn parsed_to_toml_text(parsed: &Value) -> Result<String, String> {
    if !parsed.is_object() {
        return Err("表单数据必须是 TOML 表（对象）".into());
    }
    toml::to_string_pretty(&strip_nulls(parsed)).map_err(|e| format!("TOML 生成失败: {e}"))
}

/// 递归剔除 null（TOML 无 null 类型，直接写回会生成非法配置；未知字段照常保留）
fn strip_nulls(value: &Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.iter()
                .filter(|(_, v)| !v.is_null())
                .map(|(k, v)| (k.clone(), strip_nulls(v)))
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.iter().map(strip_nulls).collect()),
        other => other.clone(),
    }
}

/// 原文是否含注释（启发式：存在以 `#` 起首的行）；表单保存会丢注释，前端据此二次确认
pub fn contains_comments(text: &str) -> bool {
    text.lines().any(|line| line.trim_start().starts_with('#'))
}

/// 档案展示统计（列表项用；来自 TOML 解析）
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProfileMeta {
    /// 服务器地址（serverAddr；缺失为空串）
    pub server_addr: String,
    /// 服务器端口（serverPort；缺失或越界为 0）
    pub server_port: u16,
    /// 代理条目总数（`[[proxies]]` 数组长度）
    pub proxy_count: u32,
    /// 启用中的代理数（`enabled` 缺省视为启用，显式 false 才停用）
    pub enabled_proxy_count: u32,
}

/// 从 TOML 原文提取展示统计（best-effort：解析失败给默认值，档案仍会出现在列表中）
pub fn parse_meta(text: &str) -> ProfileMeta {
    let Ok(value) = toml::from_str::<toml::Value>(text) else {
        return ProfileMeta::default();
    };
    let table = value.as_table();
    let server_addr = table
        .and_then(|t| t.get("serverAddr"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    // 端口越界（手写配置可能写错）按 0 展示，不为一个坏字段让整个列表失败
    let server_port = table
        .and_then(|t| t.get("serverPort"))
        .and_then(|v| v.as_integer())
        .and_then(|n| u16::try_from(n).ok())
        .unwrap_or(0);
    let proxies = table
        .and_then(|t| t.get("proxies"))
        .and_then(|v| v.as_array());
    let count = proxies.map(|list| list.len()).unwrap_or(0);
    let enabled = proxies
        .map(|list| {
            list.iter()
                .filter(|item| {
                    item.get("enabled")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(true)
                })
                .count()
        })
        .unwrap_or(0);
    ProfileMeta {
        server_addr,
        server_port,
        // 统计值是内存中的数组长度（u32 范围可证安全）；极端超限按上限截断而非 panic
        proxy_count: u32::try_from(count).unwrap_or(u32::MAX),
        enabled_proxy_count: u32::try_from(enabled).unwrap_or(u32::MAX),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn form_save_keeps_unknown_sections_and_drops_null() {
        let parsed = serde_json::json!({
            "serverAddr": "frps.example.com",
            "serverPort": 7000,
            "isEmpty": Value::Null,
            "customX": { "keep": "me" },
            "proxies": [{ "name": "p1", "type": "tcp", "localPort": 8080, "secret": Value::Null }],
        });
        let text = parsed_to_toml_text(&parsed).unwrap();
        assert!(
            text.contains("[customX]") && text.contains("keep"),
            "未知段落必须保留"
        );
        assert!(
            !text.contains("isEmpty") && !text.contains("secret"),
            "null 字段应被剔除"
        );
        // 往返：重建结果可再次解析，且未知字段仍在
        let back = toml_text_to_value(&text).unwrap();
        assert_eq!(back["customX"]["keep"], "me");
        assert_eq!(back["serverPort"], 7000);
        assert!(parsed_to_toml_text(&serde_json::json!(["not-a-table"])).is_err());
    }

    #[test]
    fn meta_counts_proxies_and_detects_comments() {
        let text = "# 注释\nserverAddr = \"a.example.com\"\nserverPort = 7001\n\n\
                    [[proxies]]\nname = \"a\"\ntype = \"tcp\"\n\n\
                    [[proxies]]\nname = \"b\"\ntype = \"tcp\"\nenabled = false\n";
        let meta = parse_meta(text);
        assert_eq!(meta.server_addr, "a.example.com");
        assert_eq!(meta.server_port, 7001);
        assert_eq!(meta.proxy_count, 2);
        assert_eq!(meta.enabled_proxy_count, 1);
        assert!(contains_comments(text));
        assert!(!contains_comments("serverAddr = \"x\"\n"));
        // 坏配置与越界端口不 panic
        assert_eq!(parse_meta("这不是 toml = = =").server_port, 0);
        assert_eq!(parse_meta("serverPort = 99999\n").server_port, 0);
    }
}
