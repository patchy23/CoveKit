//! SSH 插件 · 远程系统信息与磁盘明细
//! 一次 exec 采集（主机名 / os-release / uname / uptime / nproc / df），
//! 解析拆成纯函数（parse_system_info / parse_disks）以便独立单测；
//! 缺失字段留空、异常行跳过，不 panic、不 unwrap（Rust 规范 v1.1）。

use tauri::State;

use crate::plugins::ssh::conn::{exec_collect, get_session, SshState};
use crate::plugins::ssh::models::{SshDiskEntry, SshSystemInfo, SshSystemInfoResult};

/// 采集命令：`---` 整行分节，共 6 节（主机名 / os-release / uname / uptime / nproc / df）。
/// `export LC_ALL=C` 让固定语言覆盖整条命令（含 df 表头与 uptime 文案）；
/// 任务书里的 `LC_ALL=C hostname` 只作用于 hostname 一条，df/uptime 仍会被本地化。
/// `df -hlPT`：-h 人类可读、-l 只本地文件系统、-P 一条目一行（避免长设备名折行）、-T 带类型列。
const COLLECT_CMD: &str = "export LC_ALL=C; hostname; echo ---; cat /etc/os-release; echo ---; uname -srm; echo ---; uptime; echo ---; nproc; echo ---; df -hlPT";

/// 节分隔行（整行仅三个连字符，由 `echo ---` 输出）
const SECTION_SEP: &str = "---";

/// 按分隔行切分采集输出（纯函数）：每节保留原始输出行，便于逐节解析
fn split_sections(out: &str) -> Vec<Vec<&str>> {
    let mut sections: Vec<Vec<&str>> = vec![Vec::new()];
    for line in out.lines() {
        if line.trim() == SECTION_SEP {
            sections.push(Vec::new());
            continue;
        }
        if let Some(last) = sections.last_mut() {
            last.push(line);
        }
    }
    sections
}

/// 取首个非空行（纯函数）：用于 hostname / uname 这类单行输出
fn first_line(lines: &[&str]) -> String {
    lines
        .iter()
        .map(|line| line.trim())
        .find(|line| !line.is_empty())
        .unwrap_or_default()
        .to_string()
}

/// 按节取首个非空行（纯函数）；节不存在时返回空串
fn section_first_line(sections: &[Vec<&str>], index: usize) -> String {
    sections
        .get(index)
        .map(|lines| first_line(lines))
        .unwrap_or_default()
}

/// 解析 `key=value` 文本（纯函数）：返回首个匹配键的去引号非空值
fn lookup_key_value(lines: &[&str], key: &str) -> String {
    for line in lines {
        let Some((name, value)) = line.split_once('=') else {
            continue;
        };
        if name.trim() != key {
            continue;
        }
        let trimmed = value.trim().trim_matches('"').trim_matches('\'').trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    String::new()
}

/// 解析 uptime 的负载段（纯函数）：`load average: 0.08, 0.03, 0.00` → `[0.08, 0.03, 0.00]`
/// 兼容 `load averages:`（复数）与空白分隔（busybox），解析不足时缺失位补 0。
fn parse_load_avg(text: &str) -> [f64; 3] {
    let Some(index) = text.find("load average") else {
        return [0.0; 3];
    };
    let mut values = [0.0f64; 3];
    let mut count = 0usize;
    for token in text[index..].split(|c: char| c == ',' || c == ':' || c.is_whitespace()) {
        if count == 3 {
            break;
        }
        let Ok(value) = token.parse::<f64>() else {
            continue;
        };
        if !value.is_finite() {
            continue;
        }
        values[count] = value;
        count += 1;
    }
    values
}

/// 解析 uptime 输出（纯函数）→ (运行时长文本, 1/5/15 分钟负载)
/// 形如 ` 15:32:04 up 42 days, 20:05,  1 user,  load average: 0.08, 0.03, 0.00`
/// 运行时长取 `up` 之后、负载之前的内容，并去掉尾部的 `N users` 段。
fn parse_uptime(line: &str) -> (String, [f64; 3]) {
    let trimmed = line.trim();
    let load_avg = parse_load_avg(trimmed);
    let Some(index) = trimmed.find("up ") else {
        return (String::new(), load_avg);
    };
    let mut head = trimmed[index + 3..].trim();
    if let Some(cut) = head.find("load average") {
        head = head[..cut].trim();
    }
    let kept: Vec<&str> = head
        .split(',')
        .map(|part| part.trim())
        .filter(|part| !part.is_empty() && !part.ends_with("user") && !part.ends_with("users"))
        .collect();
    let text = if kept.is_empty() {
        head.trim().to_string()
    } else {
        kept.join(", ")
    };
    (text, load_avg)
}

/// 解析 nproc 输出（纯函数）：首个可解析且大于 0 的整数，否则 0
fn parse_cores(lines: &[&str]) -> u32 {
    for line in lines {
        let Ok(value) = line.trim().parse::<u32>() else {
            continue;
        };
        if value > 0 {
            return value;
        }
    }
    0
}

/// 解析 df 的使用率列（纯函数）：`87%` → 87.0；`-`/非数字 → 0.0；超界夹在 0-100
fn parse_percent(text: &str) -> f64 {
    match text.trim_end_matches('%').parse::<f64>() {
        Ok(value) if value.is_finite() => value.clamp(0.0, 100.0),
        _ => 0.0,
    }
}

/// 组合输出 → 系统信息（纯函数，供命令与单测共用）
/// 任一节缺失（无 /etc/os-release、无 nproc、uptime 格式异常）只留空该字段，不影响其它字段。
fn parse_system_info(out: &str) -> SshSystemInfo {
    let sections = split_sections(out);
    let os_release = sections.get(1).map(Vec::as_slice).unwrap_or(&[]);
    let uptime_line = section_first_line(&sections, 3);
    let (uptime_text, load_avg) = parse_uptime(&uptime_line);
    let os_name = {
        let pretty = lookup_key_value(os_release, "PRETTY_NAME");
        if pretty.is_empty() {
            lookup_key_value(os_release, "NAME")
        } else {
            pretty
        }
    };

    SshSystemInfo {
        hostname: section_first_line(&sections, 0),
        os_name,
        kernel: section_first_line(&sections, 2),
        uptime_text,
        load_avg,
        cpu_cores: sections.get(4).map(|lines| parse_cores(lines)).unwrap_or(0),
    }
}

/// `df -hlPT` 输出 → 磁盘明细（纯函数）
/// 跳过空行、表头（首列 `Filesystem`）与列数不足 7 的异常行；
/// 虚拟文件系统（tmpfs / devtmpfs / overlay）照常保留，仅由前端按使用率排序。
fn parse_disks(lines: &[&str]) -> Vec<SshDiskEntry> {
    let mut disks: Vec<SshDiskEntry> = Vec::new();
    for line in lines {
        if line.trim().is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        // 表头行：Filesystem Type Size Used Avail Use% Mounted on
        if parts.first().is_some_and(|head| *head == "Filesystem") {
            continue;
        }
        // -P 保证一条目一行（Filesystem Type Size Used Avail Use% Mounted on，7 列起；
        // 挂载点含空格时列数更多）。列数不足说明输出异常，跳过而不猜测。
        if parts.len() < 7 {
            continue;
        }
        disks.push(SshDiskEntry {
            filesystem: parts[0].to_string(),
            fs_type: parts[1].to_string(),
            size_text: parts[2].to_string(),
            used_text: parts[3].to_string(),
            avail_text: parts[4].to_string(),
            use_percent: parse_percent(parts[5]),
            mount_point: parts[6..].join(" "),
        });
    }
    disks
}

/// 采集远程系统信息与磁盘明细（前端 30 秒轮询 + 手动刷新调用）
#[tauri::command]
pub async fn ssh_system_info_get(
    ssh_state: State<'_, SshState>,
    connection_id: String,
) -> Result<SshSystemInfoResult, String> {
    let session = get_session(&ssh_state, &connection_id)?;
    let out = exec_collect(&session, COLLECT_CMD).await?;
    Ok(SshSystemInfoResult {
        info: parse_system_info(&out),
        disks: parse_disks(&split_sections(&out).pop().unwrap_or_default()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 采样输出（OpenCloudOS + GNU coreutils：xfs 根分区、虚拟文件系统、含空格的挂载点）
    const SAMPLE: &str = "prod-01\n\
        ---\n\
        NAME=\"OpenCloudOS\"\n\
        VERSION=\"9.2\"\n\
        PRETTY_NAME=\"OpenCloudOS 9.2\"\n\
        ID=\"opencloudos\"\n\
        ---\n\
        Linux 5.10.134-16.3.oc9.x86_64 x86_64\n\
        ---\n\
         15:32:04 up 42 days, 20:05,  1 user,  load average: 0.08, 0.03, 0.00\n\
        ---\n\
        8\n\
        ---\n\
        Filesystem     Type      Size  Used Avail Use% Mounted on\n\
        /dev/vda1      xfs        99G   12G   87G  13% /\n\
        devtmpfs       devtmpfs  1.9G     0  1.9G   0% /dev\n\
        tmpfs          tmpfs     386M     0  386M   0% /dev/shm\n\
        overlay        overlay    99G   12G   87G  13% /var/lib/docker/overlay2/abc\n\
        /dev/vdb1      ext4      500G  460G   15G  97% /data\n\
        /dev/vdc1      ext4      500G  440G   45G  88% /mnt/my data\n";

    #[test]
    fn parse_system_info_fields() {
        let info = parse_system_info(SAMPLE);
        assert_eq!(info.hostname, "prod-01");
        assert_eq!(info.os_name, "OpenCloudOS 9.2");
        assert_eq!(info.kernel, "Linux 5.10.134-16.3.oc9.x86_64 x86_64");
        assert_eq!(info.uptime_text, "42 days, 20:05");
        assert_eq!(info.load_avg, [0.08, 0.03, 0.00]);
        assert_eq!(info.cpu_cores, 8);
    }

    #[test]
    fn parse_disks_entries() {
        let disks = parse_disks(&split_sections(SAMPLE).pop().unwrap_or_default());
        // 表头被跳过：6 条数据行（含 tmpfs / devtmpfs / overlay 虚拟文件系统）
        assert_eq!(disks.len(), 6);
        let root = &disks[0];
        assert_eq!(root.filesystem, "/dev/vda1");
        assert_eq!(root.fs_type, "xfs");
        assert_eq!(root.size_text, "99G");
        assert_eq!(root.used_text, "12G");
        assert_eq!(root.avail_text, "87G");
        assert_eq!(root.use_percent, 13.0);
        assert_eq!(root.mount_point, "/");
        assert!(disks.iter().any(|d| d.mount_point == "/dev/shm"));
        assert_eq!(disks[5].mount_point, "/mnt/my data");
        assert_eq!(disks[5].use_percent, 88.0);
    }

    #[test]
    fn parse_disks_skips_header_and_broken_lines() {
        let lines = [
            "Filesystem Type Size Used Avail Use% Mounted on",
            "",
            "/dev/vda1",
            "tmpfs tmpfs 386M 0 386M 0% /run",
        ];
        let disks = parse_disks(&lines);
        assert_eq!(disks.len(), 1);
        assert_eq!(disks[0].mount_point, "/run");
    }

    #[test]
    fn parse_disks_empty_output() {
        assert!(parse_disks(&[]).is_empty());
        assert!(parse_disks(&["", "  "]).is_empty());
    }

    #[test]
    fn parse_percent_clamps_and_falls_back() {
        assert_eq!(parse_percent("97%"), 97.0);
        assert_eq!(parse_percent("-"), 0.0);
        assert_eq!(parse_percent("abc"), 0.0);
        assert_eq!(parse_percent("150%"), 100.0);
    }

    #[test]
    fn parse_uptime_variants() {
        // 无用户登录（无 users 段）
        let (text, load) =
            parse_uptime(" 12:00:00 up 3 days,  2:15,  load average: 1.50, 1.20, 0.90");
        assert_eq!(text, "3 days, 2:15");
        assert_eq!(load, [1.50, 1.20, 0.90]);
        // busybox 风格：空格分隔、无 users 段
        let (text, load) = parse_uptime(" 12:00:00 up 5 min, load average: 0.00, 0.01, 0.05");
        assert_eq!(text, "5 min");
        assert_eq!(load, [0.00, 0.01, 0.05]);
        // 格式异常：运行时长留空、负载归零，不 panic
        let (text, load) = parse_uptime("garbage");
        assert_eq!(text, "");
        assert_eq!(load, [0.0; 3]);
    }

    #[test]
    fn parse_system_info_tolerates_missing_sections() {
        // 只有主机名：其余字段留空、核数 0（无 /etc/os-release、无 nproc）
        let info = parse_system_info("legacy-host\n---\n---\n---\n---\n");
        assert_eq!(info.hostname, "legacy-host");
        assert_eq!(info.os_name, "");
        assert_eq!(info.kernel, "");
        assert_eq!(info.uptime_text, "");
        assert_eq!(info.cpu_cores, 0);
        // 完全空输出也不 panic
        let info = parse_system_info("");
        assert_eq!(info.hostname, "");
        assert_eq!(info.load_avg, [0.0; 3]);
    }

    #[test]
    fn os_name_falls_back_to_name_key() {
        let out = "h\n---\nNAME=\"CentOS Linux\"\nID=\"centos\"\n---\nLinux x86_64\n";
        assert_eq!(parse_system_info(out).os_name, "CentOS Linux");
    }
}
