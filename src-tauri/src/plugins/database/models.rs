//! 数据库工作台插件 · serde 契约结构
//! 与前端 src/plugins/database/contracts.ts 逐字段同步（统一 camelCase），
//! 字段含义注释见各属性；错误统一走 `{ ok: false, error }` 结构，不抛错给前端。

use serde::{Deserialize, Serialize};
use std::fmt;

/// 数据库类型（与前端 V2DbType 一一对应；达梦 UI 保留入口但后端暂不支持）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DbType {
    /// MySQL
    Mysql,
    /// PostgreSQL
    Postgresql,
    /// Oracle（agent 驱动）
    Oracle,
    /// 达梦 DM8（agent 驱动，本批次未实现）
    Dameng,
    /// 海量 Vastbase（agent 驱动）
    Vastbase,
    /// 人大金仓 Kingbase（agent 驱动）
    Kingbase,
    /// 阿里 PolarDB（MySQL 兼容模式，走 mysql_async + polardb profile）
    Polardb,
    /// Redis
    Redis,
    /// SQLite（文件库）
    Sqlite,
}

impl fmt::Display for DbType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl DbType {
    /// 字符串转类型（未知值返回 None，命令入口统一校验）
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "mysql" => Some(Self::Mysql),
            "postgresql" => Some(Self::Postgresql),
            "oracle" => Some(Self::Oracle),
            "dameng" => Some(Self::Dameng),
            "vastbase" => Some(Self::Vastbase),
            "kingbase" => Some(Self::Kingbase),
            "polardb" => Some(Self::Polardb),
            "redis" => Some(Self::Redis),
            "sqlite" => Some(Self::Sqlite),
            _ => None,
        }
    }

    /// 类型名（小写，与 serde 标签一致；存储用）
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Mysql => "mysql",
            Self::Postgresql => "postgresql",
            Self::Oracle => "oracle",
            Self::Dameng => "dameng",
            Self::Vastbase => "vastbase",
            Self::Kingbase => "kingbase",
            Self::Polardb => "polardb",
            Self::Redis => "redis",
            Self::Sqlite => "sqlite",
        }
    }

    /// 是否走 agent 侧车进程驱动（oracle/vastbase/kingbase；dameng 未实现）
    pub fn is_agent(self) -> bool {
        matches!(self, Self::Oracle | Self::Vastbase | Self::Kingbase)
    }

    /// 是否 Redis（无 SQL 语义，查询页签执行 Redis 命令）
    pub fn is_redis(self) -> bool {
        matches!(self, Self::Redis)
    }

    /// 是否 SQLite（连接参数是文件路径而非 host/port）
    pub fn is_sqlite(self) -> bool {
        matches!(self, Self::Sqlite)
    }

    /// 默认端口（新建连接对话框预填；sqlite 无端口）
    pub fn default_port(self) -> u16 {
        match self {
            Self::Mysql | Self::Polardb => 3306,
            Self::Postgresql | Self::Vastbase | Self::Kingbase | Self::Oracle => 5432,
            Self::Dameng => 5236,
            Self::Redis => 6379,
            Self::Sqlite => 0,
        }
    }

    /// 默认数据库名（新建连接对话框预填）
    pub fn default_database(self) -> &'static str {
        match self {
            Self::Mysql | Self::Polardb => "patchybox",
            Self::Postgresql | Self::Vastbase | Self::Kingbase => "postgres",
            Self::Oracle => "ORCL",
            Self::Dameng => "DAMENG",
            Self::Redis => "db0",
            Self::Sqlite => "main",
        }
    }
}

/// 连接配置（保存于本地库；密码不入本结构，单独存 stronghold）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnConfig {
    /// 连接唯一 id（前端生成，如 `conn-1723xxxx`）
    pub id: String,
    /// 展示名称（如「生产 · 订单库」）
    pub label: String,
    /// 数据库类型
    pub db_type: DbType,
    /// 主机地址（sqlite 为文件路径）
    pub host: String,
    /// 端口（sqlite 为 0）
    pub port: u16,
    /// 用户名（sqlite/redis 可为空）
    pub username: String,
    /// 默认数据库（redis 为 db 索引名如 db0；sqlite 为 main）
    pub database: String,
    /// 环境标记：生产/测试/开发
    pub env: String,
    /// 只读连接（前端只读徽标与写操作拦截）
    pub readonly: bool,
    /// 是否启用 TLS（mysql/pg/redis 原生驱动支持）
    pub ssl: bool,
    /// 连接超时（毫秒）
    pub connect_timeout_ms: u64,
}

/// 连接状态（前端树节点状态点）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConnStatus {
    /// 在线
    Online,
    /// 离线
    Offline,
    /// 连接中
    Connecting,
}

/// 连接快照（前端连接列表/树节点展示）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DbConnectionInfo {
    /// 连接 id
    pub id: String,
    /// 展示名称
    pub label: String,
    /// 数据库类型
    pub db_type: DbType,
    /// 环境标记
    pub env: String,
    /// 当前状态
    pub status: ConnStatus,
    /// 探测到的版本号（未连接为空）
    pub version: String,
    /// 最近一次探测时延（毫秒）
    pub latency_ms: u64,
    /// 是否只读
    pub readonly: bool,
    /// 主机地址
    pub host: String,
    /// 默认数据库
    pub database: String,
    /// 最近一次错误信息（连接失败时展示）
    pub error: Option<String>,
    /// 建立连接时间（epoch 秒；未连接为 0）
    pub connected_at: u64,
    /// 端口（sqlite 为 0；编辑对话框回填用）
    pub port: u16,
    /// 用户名（编辑对话框回填用）
    pub username: String,
    /// 是否启用 TLS（编辑对话框回填用）
    pub ssl: bool,
    /// 连接超时（毫秒；编辑对话框回填用）
    pub connect_timeout_ms: u64,
}

/// 对象树叶子信息（表/视图/函数/序列等；分组由前端按类型固定生成）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DbObjectInfo {
    /// 对象类型：table/view/function/sequence/procedure/package/synonym/index/trigger/event
    pub kind: String,
    /// 对象名
    pub name: String,
}

/// 表结构列信息（结构页签展示）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DbColumnInfo {
    /// 列名
    pub name: String,
    /// 类型（含长度，如 varchar(64)）
    pub data_type: String,
    /// 是否可空（"是"/"否"）
    pub nullable: String,
    /// 默认值（无则空串）
    pub default_value: String,
    /// 键标记：PK/UK/—（由索引探测推导）
    pub key: String,
    /// 注释
    pub comment: String,
}

/// SQL 执行结果（查询返回表格，非查询返回影响行数）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryResult {
    /// 是否成功
    pub ok: bool,
    /// 列名（查询语句）
    pub columns: Vec<String>,
    /// 数据行（单元格统一字符串化，NULL → "NULL"）
    pub rows: Vec<Vec<String>>,
    /// 影响行数（非查询语句）
    pub rows_affected: u64,
    /// 是否为查询语句
    pub is_query: bool,
    /// 执行耗时（毫秒）
    pub duration_ms: u64,
    /// 是否因行数上限被截断
    pub truncated: bool,
    /// 错误信息（ok=false 时）
    pub error: Option<String>,
}

/// 表数据分页结果（数据浏览页签）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DbTablePage {
    /// 列名
    pub columns: Vec<String>,
    /// 当前页数据行
    pub rows: Vec<Vec<String>>,
    /// 总行数（COUNT 探测；失败时为当前页行数）
    pub total: u64,
    /// 当前页码
    pub page: u32,
    /// 页大小
    pub page_size: u32,
    /// 执行耗时（毫秒）
    pub duration_ms: u64,
    /// 错误信息
    pub error: Option<String>,
}

/// Redis 键信息（键浏览面板）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RedisKeyInfo {
    /// 键名
    pub key: String,
    /// 类型：string/list/set/zset/hash/stream/other
    pub kind: String,
    /// 剩余生存时间（秒，-1 永久）
    pub ttl: i64,
    /// 值预览（string 直接取值；集合类返回条数说明）
    pub value: String,
}

/// 查询历史条目
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    /// 自增 id
    pub id: i64,
    /// 连接 id（可为空表示未归属）
    pub conn_id: String,
    /// SQL 文本
    pub sql: String,
    /// 执行结果：success/error
    pub status: String,
    /// 执行耗时（毫秒）
    pub duration_ms: u64,
    /// 时间（HH:mm 或完整时间）
    pub at: String,
}

/// 收藏 SQL 条目
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedEntry {
    /// 自增 id
    pub id: i64,
    /// 标题
    pub title: String,
    /// SQL 文本
    pub sql: String,
    /// 创建时间
    pub at: String,
}
