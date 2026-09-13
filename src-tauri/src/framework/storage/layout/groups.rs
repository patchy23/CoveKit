//! 框架 · 旧布局迁移的集合定义
//!
//! 迁移矩阵集中在这里：哪个旧文件属于哪个分区、哪些成员必须同进同退。
//! 新增旧格式只需在此加一条，迁移引擎（`engine.rs`）与编排（`layout.rs`）都不用改。

use std::path::Path;

/// 集合成员类型：文件、目录，或由扫描动态发现的一组数据库文件
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ItemKind {
    /// 普通文件
    File,
    /// 目录（整体搬入分区）
    Dir,
}

/// 集合成员：旧位置名 + 目标分区
#[derive(Debug, Clone, Copy)]
pub(crate) struct Member {
    /// 根下旧名字
    pub name: &'static str,
    /// 目标分区（data / vault / logs / cache）
    pub partition: &'static str,
    /// 文件或目录
    pub kind: ItemKind,
}

/// 迁移集合
#[derive(Debug, Clone, Copy)]
pub(crate) struct Group {
    /// 集合标识（报告与日志用）
    pub id: &'static str,
    /// 集合成员
    pub members: &'static [Member],
}

/// 构造文件的简写
pub(crate) const fn file(name: &'static str, partition: &'static str) -> Member {
    Member {
        name,
        partition,
        kind: ItemKind::File,
    }
}

/// 构造目录的简写
pub(crate) const fn dir(name: &'static str, partition: &'static str) -> Member {
    Member {
        name,
        partition,
        kind: ItemKind::Dir,
    }
}

/// 凭据目录与主密钥：目录与密钥必须同时迁移，否则会「读到密文却没有密钥」
pub(crate) const CREDENTIALS: [Member; 2] = [
    file("credentials-master.key", "data"),
    dir("credentials", "data"),
];

/// Vault 数据文件与主密钥（成对迁移）
pub(crate) const VAULT: [Member; 2] = [
    file("vault.dat", "vault"),
    file("vault-master.key", "vault"),
];

/// SSH 旧手工凭证文件与主密钥（成对迁移）
pub(crate) const SSH_CREDENTIALS: [Member; 2] = [
    file("ssh-credentials.json", "data"),
    file("ssh-master.key", "data"),
];

/// 数据库插件密文、备份与主密钥（三者同进同退）
pub(crate) const DB_SECRETS: [Member; 3] = [
    file("db-secrets.enc", "data"),
    file("db-master.key", "data"),
    file("db-secrets.bak", "data"),
];

/// 已弃用的 stronghold 快照与其客户端快照（仅清理，不迁移内容）
pub(crate) const DB_STRONGHOLD: [Member; 2] = [
    file("db.stronghold", "data"),
    file("db-client.snapshot", "data"),
];

/// SSH 已知主机指纹文件
pub(crate) const SSH_KNOWN_HOSTS: [Member; 1] = [file("ssh-known-hosts", "data")];
/// TTS 语音缓存目录（可重建，但保持一致以免重复合成）
pub(crate) const TTS_CACHE: [Member; 1] = [dir("tts", "cache")];
/// agents 缓存目录（可重建）
pub(crate) const AGENTS_CACHE: [Member; 1] = [dir("agents", "cache")];

/// 固定集合表（`*.db` 由扫描动态补充）
pub(crate) const GROUPS: [Group; 8] = [
    Group {
        id: "credentials",
        members: &CREDENTIALS,
    },
    Group {
        id: "vault",
        members: &VAULT,
    },
    Group {
        id: "ssh-credentials",
        members: &SSH_CREDENTIALS,
    },
    Group {
        id: "db-secrets",
        members: &DB_SECRETS,
    },
    Group {
        id: "db-stronghold",
        members: &DB_STRONGHOLD,
    },
    Group {
        id: "ssh-known-hosts",
        members: &SSH_KNOWN_HOSTS,
    },
    Group {
        id: "tts-cache",
        members: &TTS_CACHE,
    },
    Group {
        id: "agents-cache",
        members: &AGENTS_CACHE,
    },
];

impl Group {
    /// 成员名对应的目标分区（数据库伴随文件跟随其 `.db` 成员）
    pub(crate) fn partition_of(&self, name: &str) -> &'static str {
        for member in self.members {
            if name == member.name || name.starts_with(&format!("{}.db", member.name)) {
                return member.partition;
            }
        }
        self.members[0].partition
    }

    /// 成员名对应的期望类型（数据库伴随文件一律按文件处理）
    pub(crate) fn kind_of(&self, name: &str) -> ItemKind {
        for member in self.members {
            if name == member.name {
                return member.kind;
            }
        }
        ItemKind::File
    }
}

/// 数据库恢复文件后缀：与 `X.db` 同属一个集合，必须一起搬
const DB_COMPANIONS: [&str; 3] = ["-wal", "-shm", "-journal"];

/// 扫描根下属于某个数据库集合的成员名（`X.db` 与其伴随文件，只列出实际存在的）
pub(crate) fn db_group_members(root: &Path, db_name: &str) -> Vec<String> {
    let mut names = vec![db_name.to_string()];
    for suffix in DB_COMPANIONS {
        let candidate = format!("{db_name}{suffix}");
        if root.join(&candidate).exists() {
            names.push(candidate);
        }
    }
    names
}

/// 列出根下所有动态数据库集合（`*.db`）
pub(crate) fn dynamic_db_groups(root: &Path) -> Result<Vec<String>, String> {
    let mut names = Vec::new();
    let entries = std::fs::read_dir(root).map_err(|e| format!("读取存储根目录失败: {e}"))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("读取存储根目录项失败: {e}"))?;
        let name = entry.file_name().to_string_lossy().to_string();
        if name.to_ascii_lowercase().ends_with(".db") {
            names.push(name);
        }
    }
    names.sort();
    Ok(names)
}

/// 旧位置残留清单（修复遍判定：即使 `layoutVersion` 已达标也要检查）
pub(crate) fn leftovers(root: &Path) -> Result<Vec<String>, String> {
    let mut names = Vec::new();
    for group in GROUPS {
        for member in group.members {
            if root.join(member.name).exists() {
                names.push(member.name.to_string());
            }
        }
    }
    names.extend(dynamic_db_groups(root)?);
    names.sort();
    names.dedup();
    Ok(names)
}
