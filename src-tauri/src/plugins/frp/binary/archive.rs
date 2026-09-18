//! FRP 客户端压缩包的平台解压适配。

use std::path::Path;
use tokio::process::Command;

/// 命令行路径参数（按原样传参，交给 OS 处理；仅非 Windows 的 tar 分支使用）
#[cfg(not(windows))]
fn arg_str(path: &Path) -> String {
    path.display().to_string()
}

/// PowerShell 单引号字面量（字符串内的单引号需转义成两个，否则路径含引号时脚本被截断）
#[cfg(windows)]
pub(super) fn ps_literal(path: &Path) -> String {
    path.display().to_string().replace('\'', "''")
}

/// 目标目录是否已产出内容（系统解压工具退出码不可信时的兜底判据）
pub(super) async fn has_entries(dir: &Path) -> bool {
    match tokio::fs::read_dir(dir).await {
        Ok(mut entries) => entries.next_entry().await.ok().flatten().is_some(),
        Err(_) => false,
    }
}

/// 从命令输出里取一段可读原因（stderr 优先；中文系统上 PowerShell 的中文报错会因 GBK
/// 编码显示为乱码，但其中的英文错误标识仍可辨认，比笼统的「解压失败」有用）
fn output_tail(stderr: &[u8], stdout: &[u8]) -> String {
    let pick = |bytes: &[u8]| {
        String::from_utf8_lossy(bytes)
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .take(3)
            .collect::<Vec<_>>()
            .join(" ")
    };
    let from_stderr = pick(stderr);
    if from_stderr.is_empty() {
        pick(stdout)
    } else {
        from_stderr
    }
}

/// 解压压缩包到目标目录（复用系统工具，避免为一次性操作引入压缩库依赖）
///
/// 三个坑都是实测踩到才发现的，改动本函数前先读（Windows 分支）：
/// 1) **压缩包必须保留 `.zip` 扩展名**：PowerShell 5.1 的 `Expand-Archive` 对 `.zip.tmp`
///    这类后缀直接报 NotSupportedArchiveFileExtension，内容都不看就拒绝；
/// 2) **不要按名字调用 `tar` 解 zip**：PATH 上的 `tar` 可能是 MSYS / GNU tar（本机实测
///    GNU tar 1.35），GNU tar 不支持 zip，还会把 `C:\` 当远程主机（`Cannot connect to C:`）；
///    只有 Windows 自带的 bsdtar 支持 zip，按名字调用等于把成败押在用户机器的 PATH 顺序上；
/// 3) `Expand-Archive` 遇到坏包时**写 stderr 但退出码仍为 0**，必须用 `try/catch + exit 1`
///    才能拿到非零退出码，并且额外校验目录确实产出了内容，否则会把「什么都没解出来」当成功。
#[cfg(windows)]
pub(super) async fn extract(archive: &Path, dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("创建解压目录失败：{e}"))?;
    let script = format!(
        "try {{ Expand-Archive -LiteralPath '{}' -DestinationPath '{}' -Force -ErrorAction Stop }} \
         catch {{ [Console]::Error.WriteLine($_.Exception.Message); exit 1 }}",
        ps_literal(archive),
        ps_literal(dir)
    );
    let output = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .output()
        .await
        .map_err(|e| format!("调用系统解压工具失败：{e}"))?;
    if output.status.success() && has_entries(dir).await {
        return Ok(());
    }
    let detail = output_tail(&output.stderr, &output.stdout);
    if detail.is_empty() {
        Err("解压失败：系统解压工具未能解开该压缩包".to_string())
    } else {
        Err(format!("解压失败：{detail}"))
    }
}

/// 解压压缩包到目标目录（其它平台：系统 tar 原生支持 tar.gz）
#[cfg(not(windows))]
pub(super) async fn extract(archive: &Path, dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("创建解压目录失败：{e}"))?;
    let output = Command::new("tar")
        .args(["-xzf", &arg_str(archive), "-C", &arg_str(dir)])
        .output()
        .await
        .map_err(|e| format!("调用系统解压工具失败：{e}"))?;
    if output.status.success() && has_entries(dir).await {
        return Ok(());
    }
    let detail = output_tail(&output.stderr, &output.stdout);
    if detail.is_empty() {
        Err("解压失败：系统 tar 无法解开该压缩包".to_string())
    } else {
        Err(format!("解压失败：{detail}"))
    }
}
