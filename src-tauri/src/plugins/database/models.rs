//! 数据库工作台插件 · serde 契约结构
//! 与前端 src/plugins/database/contracts.ts 逐字段同步（统一 camelCase），
//! 字段含义注释见各属性；错误统一走 `{ ok: false, error }` 结构，不抛错给前端。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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

    /// 默认数据库名（新建连接对话框预填；mysql/polardb 可空——库不存在会导致连接失败）
    pub fn default_database(self) -> &'static str {
        match self {
            Self::Mysql | Self::Polardb => "",
            Self::Postgresql | Self::Vastbase | Self::Kingbase => "postgres",
            Self::Oracle => "ORCL",
            Self::Dameng => "DAMENG",
            Self::Redis => "db0",
            Self::Sqlite => "main",
        }
    }
}

/// 连接配置（保存于本地库；密码不入本结构，单独存 stronghold）
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnConfig {
    /// 公共 Vault 引用；旧记录缺省为空，由连接列表幂等迁移。
    #[serde(default)]
    pub credential_id: Option<String>,
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
    /// 公共凭证引用；不向前端返回密码。
    pub credential_id: Option<String>,
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

/// 可逆单元格；binary 的 value 为十六进制，数值保留十进制字符串精度。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DbValue {
    /// 值类型标签；null、binary、decimal 等由驱动显式区分。
    pub kind: String,
    /// 无损文本载荷；SQL NULL 为 None，空字符串仍为 Some。
    pub value: Option<String>,
}

/// SQL 页签的实际执行目标，空值使用连接默认值。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionScope {
    /// 目标数据库；空值使用连接默认库。
    pub database: String,
    /// 目标 schema；无 schema 的驱动忽略。
    pub schema: String,
}

/// SQL 执行结果（查询返回表格，非查询返回影响行数）
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryResult {
    /// 可证明为单表直接投影的结果来源；仍须校验完整列和主键才能编辑。
    pub edit_target: Option<ResultEditTarget>,
    /// 多结果集按原 SQL 语句归属，不把 CALL 的第二结果误当成后续 SQL。
    pub statement_index: Option<usize>,
    /// 当前工作页是否处于显式事务；错误后的事务仍需回滚。
    pub transaction_active: bool,
    /// 与 rows 一一对应的原值，NULL 与文本 NULL 独立。
    pub values: Vec<Vec<DbValue>>,
    /// 驱动原生列类型；无声明类型的 SQLite 可为空。
    pub column_types: Vec<String>,
    /// 多语句执行按顺序保留各结果及失败，单语句为空。
    pub statements: Vec<QueryResult>,
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

/// 查询执行时固定的编辑目标，不从可被继续修改的编辑器正文推断。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultEditTarget {
    /// 原连接标识。
    pub conn_id: String,
    /// 执行时数据库。
    pub database: String,
    /// 执行时 schema，MySQL 为库名。
    pub schema: String,
    /// 单一来源表。
    pub table: String,
}

/// 表数据分页结果（数据浏览页签）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DbTablePage {
    /// 本页实际执行的参数化 SQL。
    pub query_sql: String,
    /// 与 SQL 占位符顺序一致的绑定值。
    pub query_params: Vec<DbValue>,
    /// 与显示行逐格对应的可逆值。
    pub values: Vec<Vec<DbValue>>,
    /// 通过额外读取一行判断是否存在下一页。
    pub has_more: bool,
    /// lowerBound/pageEnd 均非独立 COUNT 的精确总数。
    pub total_kind: String,
    /// 排序是否包含唯一主键以避免页边界随机漂移。
    pub stable_order: bool,
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
    /// 执行时实际目标数据库。
    pub database: String,
    /// 执行时实际目标 schema。
    pub schema: String,
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

// ──────────────────────────────────────────────────────────────────────────
// 管理操作（建库/授权/DDL/索引/表维护）
// ──────────────────────────────────────────────────────────────────────────

/// 字符集选项（新建数据库对话框；collationsByCharset 用于字符集→排序规则联动）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DbCharsetOptions {
    /// 字符集清单
    pub charsets: Vec<String>,
    /// 字符集 → 排序规则清单
    pub collations_by_charset: HashMap<String, Vec<String>>,
}

/// 数据库用户（授权选择列表）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DbUserInfo {
    /// 用户名
    pub user: String,
    /// 主机（mysql 账号的 host 段）
    pub host: String,
}

/// 授权目标（建库授权入参；privilege 白名单：all/readwrite/readonly）
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DbGrantInput {
    /// 用户名
    pub user: String,
    /// 主机
    pub host: String,
    /// 权限级别：all/readwrite/readonly
    pub privilege: String,
}

/// 分步执行结果（建库+授权逐步反馈，前端逐步打勾/报错）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DbStepResult {
    /// 步骤名（如「创建数据库」「授权 root@%」）
    pub label: String,
    /// 实际执行的 SQL
    pub sql: String,
    /// 是否成功
    pub ok: bool,
    /// 错误信息（失败时）
    pub error: Option<String>,
}

/// 索引信息（结构页签 · 索引子页签）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DbIndexInfo {
    /// 索引名
    pub name: String,
    /// 覆盖列（按索引内顺序）
    pub columns: Vec<String>,
    /// 是否非唯一索引（true = 允许重复）
    pub non_unique: bool,
    /// 索引类型/定义（mysql 为 BTREE 等；pg 为完整 indexdef）
    pub definition: String,
}

/// 服务端筛选只允许固定操作符，列名须在真实结构中存在。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TableFilter {
    #[serde(default)]
    /// IN 或 BETWEEN 的绑定值列表，其余操作符忽略。
    pub values: Vec<DbValue>,
    /// 目标列名，须通过真实元数据白名单校验。
    pub column: String,
    /// 固定操作符标识，禁止传入 SQL 片段。
    pub operator: String,
    /// 单值操作符的绑定值。
    pub value: DbValue,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// 服务端排序列及方向，列名必须属于目标表。
pub struct TableSort {
    /// 已校验的排序列名。
    pub column: String,
    /// 是否降序。
    pub descending: bool,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// 数据浏览的筛选与排序条件，分页和精确计数共享此契约。
pub struct TableOptions {
    #[serde(default)]
    /// 以 AND 组合的筛选条件，最多 20 条。
    pub filters: Vec<TableFilter>,
    #[serde(default)]
    /// 排序列，最多 10 项；后端追加主键。
    pub sort: Vec<TableSort>,
}
/// 单表的一次变更；original 是完整原行，用于主键定位和并发冲突检测。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TableChange {
    /// insert、update 或 delete。
    pub action: String,
    #[serde(default)]
    /// 修改前完整原行，用于主键定位和并发校验。
    pub original: HashMap<String, DbValue>,
    #[serde(default)]
    /// 待写入列及新值；未提供的列保持默认或原值。
    pub values: HashMap<String, DbValue>,
}

/** CSV 预览只返回前 20 行；指纹绑定用户确认的文件内容。 */
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CsvPreview {
    /// CSV 表头，用户据此选择列映射。
    pub columns: Vec<String>,
    /// 前 20 行原始 CSV 文本。
    pub rows: Vec<Vec<String>>,
    /// 完整文件的数据行数。
    pub total: u64,
    /// 文件 SHA-256，导入时校验防止预览后被替换。
    pub fingerprint: String,
}
/// CSV 源索引到目标表列的显式映射。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CsvMapping {
    /// 从零开始的 CSV 列索引。
    pub source: usize,
    /// 目标表真实列名。
    pub column: String,
}

/// 可恢复 SQL 文档；结果和事务不属于持久化草稿。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryDraft {
    /// 恢复后的页签标题。
    pub label: String,
    /// 未执行的 SQL 原文。
    pub sql: String,
    /// 连接引用；连接不存在时保持离线草稿。
    pub connection_id: String,
    /// 文档目标数据库。
    pub database: String,
    /// 文档目标 schema。
    pub schema: String,
    /// CodeMirror 选区起点，UTF-16 偏移。
    pub from: usize,
    /// CodeMirror 选区终点，UTF-16 偏移。
    pub to: usize,
    /// 相对文件或收藏是否有未保存修改。
    pub dirty: bool,
    /// 是否优先恢复为当前编辑页。
    pub active: bool,
    /// 关联 SQL 收藏标识。
    pub saved_id: Option<i64>,
    /// 关联收藏的标题快照。
    pub saved_title: Option<String>,
    /// 关联 SQL 文件路径，不在恢复时自动读取。
    pub file_path: Option<String>,
}
