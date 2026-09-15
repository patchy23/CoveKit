//! agent JSON-RPC 2.0 客户端（NDJSON over stdin/stdout）
//! 生命周期：spawn（等待 ready）→ handshake（能力协商）→ open_session →
//! 会话级方法（execute_query / list_tables / get_columns 等）→ close_session → shutdown。
//! 并发模型：请求按 id 关联 oneshot 通道，响应乱序返回也正确路由；
//! 同一会话的请求由上层串行（连接状态不并发安全，协议要求同会话串行）。

use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::{mpsc, oneshot};
use tokio::time::{timeout, Duration};

/// 单个 RPC 调用的超时（连接/查询等长操作默认 60s；调用方可覆盖）
const RPC_TIMEOUT: Duration = Duration::from_secs(60);

/// 待响应请求表类型（id → 响应发送端；读循环与客户端共享）
type PendingMap = Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value, String>>>>>;

/// agent 进程客户端（一个运行时一个实例，进程内多会话共享）
pub struct AgentClient {
    /// 子进程句柄（保持存活；drop 时由 runtime 层负责 shutdown）
    child: Arc<Mutex<Child>>,
    /// 请求行写入通道（后台写任务消费）
    writer: mpsc::UnboundedSender<String>,
    /// 待响应的请求表（id → 响应发送端）
    pending: PendingMap,
    /// 请求 id 递增
    next_id: AtomicU64,
}

/// 打开会话的入参（snake_case，与 dbx ConnectParams 对齐）
#[derive(Debug, Clone)]
pub struct AgentConnectParams {
    /// 会话 id（客户端生成，进程内唯一）
    pub session_id: String,
    /// 会话角色：workload（默认）/ metadata
    pub session_role: &'static str,
    /// 主机
    pub host: String,
    /// 端口
    pub port: u16,
    /// 数据库/服务名
    pub database: String,
    /// 用户名
    pub username: String,
    /// 密码
    pub password: String,
    /// 是否启用 TLS
    pub ssl: bool,
}

impl AgentClient {
    /// 启动 agent 可执行文件并等待 ready 信号（不带额外环境变量）
    pub async fn spawn(program: &Path, working_dir: &Path) -> Result<Self, String> {
        Self::spawn_with_env_and_args(program, working_dir, &[], &[]).await
    }

    /// 启动 agent 可执行文件（可注入环境变量与命令行参数；测试用子进程重入需要 --exact 过滤）
    pub async fn spawn_with_env_and_args(
        program: &Path,
        working_dir: &Path,
        envs: &[(&str, &str)],
        args: &[&str],
    ) -> Result<Self, String> {
        let mut cmd = Command::new(program);
        cmd.current_dir(working_dir)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        for (k, v) in envs {
            cmd.env(k, v);
        }
        let mut child = cmd
            .spawn()
            .map_err(|e| format!("启动 agent 失败（{}）：{e}", program.display()))?;

        let stdin = child.stdin.take().ok_or("agent stdin 不可用")?;
        let stdout = child.stdout.take().ok_or("agent stdout 不可用")?;

        let (writer_tx, writer_rx) = mpsc::unbounded_channel::<String>();
        let pending = Arc::new(Mutex::new(HashMap::new()));
        let client = AgentClient {
            child: Arc::new(Mutex::new(child)),
            writer: writer_tx,
            pending: pending.clone(),
            next_id: AtomicU64::new(1),
        };

        // 写任务：请求行 → 子进程 stdin
        tokio::spawn(write_loop(stdin, writer_rx));

        // 等待 ready 行（10s 超时）：测试二进制等宿主会先输出噪音行，逐行跳过直到出现 ready
        let mut reader = BufReader::new(stdout);
        let ready = timeout(Duration::from_secs(10), async {
            let mut line = String::new();
            for _ in 0..200 {
                line.clear();
                match reader.read_line(&mut line).await {
                    Ok(0) => return Err("agent 进程提前退出（未输出 ready）".to_string()),
                    Err(e) => return Err(format!("读取 agent 输出失败: {e}")),
                    Ok(_) => {}
                }
                if line.contains("\"ready\"") {
                    return Ok(());
                }
            }
            Err("agent 启动输出异常（未收到 ready 信号）".to_string())
        })
        .await
        .map_err(|_| "等待 agent ready 超时（10s）".to_string())?;
        ready?;

        // 读任务：子进程 stdout → 按 id 路由响应（与 ready 等待共用同一读取器）
        tokio::spawn(read_loop(reader, pending.clone()));
        Ok(client)
    }

    /// 发起一个 RPC（方法 + 参数对象），返回响应 result；错误统一转 Err
    pub async fn call(&self, method: &str, params: Value) -> Result<Value, String> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        self.pending
            .lock()
            .map_err(|e| e.to_string())?
            .insert(id, tx);
        let request = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        self.writer
            .send(request.to_string())
            .map_err(|_| "agent 写通道已关闭（进程退出？）".to_string())?;

        let result = timeout(RPC_TIMEOUT, rx)
            .await
            .map_err(|_| format!("agent 方法 {method} 超时（{RPC_TIMEOUT:?}）"))?
            .map_err(|_| format!("agent 方法 {method} 响应通道已关闭"))?;

        result
    }

    /// 能力协商：确认 multi_session / structured_error_v1 等能力（尽力而为，失败不阻断）
    pub async fn handshake(&self) -> Result<(), String> {
        let result = self.call("handshake", json!({})).await?;
        let capabilities = result
            .get("capabilities")
            .and_then(|c| c.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| s.to_string())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if capabilities.iter().any(|c| c == "multi_session") {
            Ok(())
        } else {
            // 老版单会话 agent 也可用（走 legacy connect 路径由调用方决定）
            Ok(())
        }
    }

    /// 打开一个数据库会话（参数含连接凭据；同进程可多会话）
    pub async fn open_session(&self, params: &AgentConnectParams) -> Result<(), String> {
        let result = self
            .call(
                "open_session",
                json!({
                    "agentSessionId": params.session_id,
                    "sessionRole": params.session_role,
                    "host": params.host,
                    "port": params.port,
                    "database": params.database,
                    "username": params.username,
                    "password": params.password,
                    "ssl": params.ssl,
                }),
            )
            .await?;
        // 成功响应一般返回 {ok:true} 或连接信息；仅错误时进入 Err 分支
        let _ = result;
        Ok(())
    }

    /// 校验/重连单个会话（连接保活探测）
    pub async fn validate_session(&self, session_id: &str) -> Result<(), String> {
        let result = self
            .call("validate_session", json!({ "agentSessionId": session_id }))
            .await?;
        let _ = result;
        Ok(())
    }

    /// 取消会话内正在执行的语句/游标抓取
    pub async fn cancel_session(&self, session_id: &str) -> Result<(), String> {
        let result = self
            .call("cancel_session", json!({ "agentSessionId": session_id }))
            .await?;
        let _ = result;
        Ok(())
    }

    /// 关闭会话（资源释放；不影响进程内其它会话）
    pub async fn close_session(&self, session_id: &str) -> Result<(), String> {
        let result = self
            .call("close_session", json!({ "agentSessionId": session_id }))
            .await?;
        let _ = result;
        Ok(())
    }

    /// 测试连接（不建立会话；成功返回 ok，失败 Err）
    pub async fn test_connection(&self, params: &AgentConnectParams) -> Result<(), String> {
        let result = self
            .call(
                "test_connection",
                json!({
                    "host": params.host,
                    "port": params.port,
                    "database": params.database,
                    "username": params.username,
                    "password": params.password,
                    "ssl": params.ssl,
                }),
            )
            .await?;
        let _ = result;
        Ok(())
    }

    /// 连接信息（版本探测等）
    pub async fn connection_info(&self, session_id: &str) -> Result<Value, String> {
        self.call("connection_info", json!({ "agentSessionId": session_id }))
            .await
    }

    /// 执行查询（返回 {columns, rows, ...}；max_rows 为服务端行数上限）
    pub async fn execute_query(
        &self,
        session_id: &str,
        sql: &str,
        max_rows: u64,
    ) -> Result<Value, String> {
        self.call(
            "execute_query",
            json!({
                "agentSessionId": session_id,
                "sql": sql,
                "options": { "maxRows": max_rows },
            }),
        )
        .await
    }

    /// 数据库列表
    pub async fn list_databases(&self, session_id: &str) -> Result<Vec<String>, String> {
        let result = self
            .call("list_databases", json!({ "agentSessionId": session_id }))
            .await?;
        extract_names(result, "databases")
    }

    /// schema 列表
    pub async fn list_schemas(&self, session_id: &str) -> Result<Vec<String>, String> {
        let result = self
            .call("list_schemas", json!({ "agentSessionId": session_id }))
            .await?;
        extract_names(result, "schemas")
    }

    /// 对象列表（kind/name 二元组；kind 由 agent 归一化）
    pub async fn list_objects(
        &self,
        session_id: &str,
        schema: &str,
    ) -> Result<Vec<(String, String)>, String> {
        let result = self
            .call(
                "list_objects",
                json!({ "agentSessionId": session_id, "schema": schema }),
            )
            .await?;
        let mut out = Vec::new();
        if let Some(items) = result.get("objects").and_then(|v| v.as_array()) {
            for item in items {
                let name = item.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let kind = item.get("type").and_then(|v| v.as_str()).unwrap_or("table");
                if !name.is_empty() {
                    out.push((kind.to_string(), name.to_string()));
                }
            }
        }
        Ok(out)
    }

    /// 表结构列（name/type/nullable/default/comment/key）
    pub async fn get_columns(
        &self,
        session_id: &str,
        schema: &str,
        table: &str,
    ) -> Result<Value, String> {
        self.call(
            "get_columns",
            json!({
                "agentSessionId": session_id,
                "schema": schema,
                "table": table,
            }),
        )
        .await
    }

    /// 立即结束 agent 子进程（同步；供退出清理使用）
    ///
    /// 为什么不走协议 `shutdown`：退出路径只有总清理预算（`framework::lifecycle::DISPOSE_TIMEOUT`），
    /// 在这里等一次 RPC 往返可能把预算耗光、后面的模块轮不到清理；进程随即退出，
    /// 服务端会话随 stdout 管道断开一起结束。
    pub fn kill_now(&self) -> Result<(), String> {
        let mut guard = self.child.lock().map_err(|e| e.to_string())?;
        let _ = guard.start_kill();
        Ok(())
    }

    /// 关闭全部会话并终止进程
    pub async fn shutdown(&self) -> Result<(), String> {
        // shutdown 后进程自行退出；失败时由调用方 kill（start_kill 为同步信号，避免跨 await 持锁）
        let result = self.call("shutdown", json!({})).await;
        let mut guard = self.child.lock().map_err(|e| e.to_string())?;
        let _ = guard.start_kill();
        drop(guard);
        result.map(|_| ())
    }
}

/// 从响应中提取名称列表（兼容 {"databases":[...]} 与裸数组两种形状）
fn extract_names(result: Value, key: &str) -> Result<Vec<String>, String> {
    let arr = result
        .get(key)
        .or_else(|| result.get("names"))
        .and_then(|v| v.as_array())
        .ok_or_else(|| format!("agent 响应缺少 {key} 字段：{result}"))?;
    Ok(arr
        .iter()
        .filter_map(|v| v.as_str().map(|s| s.to_string()))
        .collect())
}

/// 写循环：通道 → 子进程 stdin（每行一个请求）
async fn write_loop(mut stdin: ChildStdin, mut rx: mpsc::UnboundedReceiver<String>) {
    while let Some(line) = rx.recv().await {
        let mut buf = line.into_bytes();
        buf.push(b'\n');
        if stdin.write_all(&buf).await.is_err() {
            break;
        }
        let _ = stdin.flush().await;
    }
}

/// 读循环：子进程 stdout → 按 id 路由到 pending 表
async fn read_loop(mut reader: BufReader<tokio::process::ChildStdout>, pending: PendingMap) {
    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line).await {
            Ok(0) | Err(_) => break, // EOF 或读错误：进程退出
            Ok(_) => {}
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let parsed: Option<Value> = serde_json::from_str(trimmed).ok();
        let Some(value) = parsed else {
            continue;
        };
        let id = value.get("id").and_then(|v| v.as_u64());
        let Some(id) = id else {
            continue;
        };
        let sender = pending.lock().ok().and_then(|mut m| m.remove(&id));
        let Some(sender) = sender else {
            continue;
        };
        if let Some(error) = value.get("error") {
            let message = error
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("agent 未知错误")
                .to_string();
            let _ = sender.send(Err(message));
        } else if let Some(result) = value.get("result") {
            let _ = sender.send(Ok(result.clone()));
        } else {
            let _ = sender.send(Err("agent 响应缺少 result/error".to_string()));
        }
    }
    // 进程退出：全部挂起请求报错
    if let Ok(mut map) = pending.lock() {
        for (_, sender) in map.drain() {
            let _ = sender.send(Err("agent 进程已退出".to_string()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 用当前测试二进制自身模拟 agent 进程（环境变量触发 agent 模式）
    /// 模拟协议：ready 首行 → 回显 handshake/open_session/execute_query 等方法的固定结果。
    #[tokio::test]
    async fn rpc_roundtrip_with_mock_agent_process() {
        // 测试二进制重入：DBX_MOCK_AGENT=1 时本进程扮演 agent
        if std::env::var("DBX_MOCK_AGENT").is_ok() {
            mock_agent_main();
            return;
        }
        // 子进程只跑本测试（--exact）且不捕获输出（--nocapture），避免测试输出噪音与缓冲。
        // 注意：lib 目标重命名后 module_path!() 带 crate 名前缀（patchybox_lib::…），
        // 而 libtest 的测试名不含该前缀，需剥掉第一段才能 --exact 匹配。
        let exe = std::env::current_exe().unwrap();
        let dir = std::env::temp_dir();
        let module = module_path!()
            .split("::")
            .skip(1)
            .collect::<Vec<_>>()
            .join("::");
        let test_path = format!("{module}::rpc_roundtrip_with_mock_agent_process");
        let client = AgentClient::spawn_with_env_and_args(
            &exe,
            &dir,
            &[("DBX_MOCK_AGENT", "1")],
            &["--exact", &test_path, "--nocapture"],
        )
        .await
        .unwrap();
        client.handshake().await.unwrap();

        let params = AgentConnectParams {
            session_id: "s1".into(),
            session_role: "workload",
            host: "localhost".into(),
            port: 1521,
            database: "ORCL".into(),
            username: "scott".into(),
            password: "tiger".into(),
            ssl: false,
        };
        client.open_session(&params).await.unwrap();

        let result = client
            .execute_query("s1", "SELECT 1 FROM dual", 100)
            .await
            .unwrap();
        assert_eq!(result["columns"][0], "1");

        client.close_session("s1").await.unwrap();
        client.shutdown().await.unwrap();
    }

    /// 模拟 agent 主循环（stdin 读行 → 按方法回固定响应）
    fn mock_agent_main() {
        use std::io::{BufRead, Write};
        let mut out = std::io::stdout();
        writeln!(out, "{{\"ready\":true}}").unwrap();
        out.flush().unwrap();
        let stdin = std::io::stdin();
        for line in stdin.lock().lines() {
            let Ok(line) = line else { break };
            let Ok(req) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            let id = req["id"].clone();
            let method = req["method"].as_str().unwrap_or("");
            let response = match method {
                "handshake" => json!({ "jsonrpc": "2.0", "id": id, "result": {
                    "protocolVersion": 2, "capabilities": ["connect", "query", "metadata", "multi_session"]
                }}),
                "open_session" | "close_session" | "cancel_session" | "validate_session"
                | "test_connection" | "shutdown" => {
                    json!({ "jsonrpc": "2.0", "id": id, "result": { "ok": true } })
                }
                "execute_query" => json!({ "jsonrpc": "2.0", "id": id, "result": {
                    "columns": ["1", "name"],
                    "rows": [["1", "dual"]],
                    "affected_rows": 0,
                    "execution_time_ms": 3,
                    "truncated": false
                }}),
                "list_databases" => {
                    json!({ "jsonrpc": "2.0", "id": id, "result": { "databases": ["ORCL"] } })
                }
                "list_schemas" => {
                    json!({ "jsonrpc": "2.0", "id": id, "result": { "schemas": ["SCOTT", "HR"] } })
                }
                _ => {
                    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32601, "message": format!("unknown method {method}") } })
                }
            };
            writeln!(out, "{response}").unwrap();
            out.flush().unwrap();
            if method == "shutdown" {
                break;
            }
        }
    }
}
