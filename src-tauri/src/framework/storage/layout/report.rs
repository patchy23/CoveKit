//! 框架 · 旧布局迁移的结果模型
//!
//! 报告要能回答三个问题：哪些集合搬好了、哪些失败（含原因）、旧位置是否还有残留。
//! 「已一致」与「冲突留档」都算处理完成，只有 [`ItemState::Failed`] 会阻止版本推进。

/// 单项处理结果
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ItemState {
    /// 已从旧位置搬入分区
    Migrated,
    /// 新位置已有相同内容，旧位置已清理
    AlreadyCurrent,
    /// 旧位置没有该数据（无需处理）
    Missing,
    /// 新位置内容不一致，已留档并改用旧位置数据
    ConflictResolved,
    /// 处理失败（该项保持原位，版本不推进）
    Failed,
}

impl ItemState {
    /// 稳定字符串（报告/日志用）
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Migrated => "migrated",
            Self::AlreadyCurrent => "already-current",
            Self::Missing => "missing",
            Self::ConflictResolved => "conflict-resolved",
            Self::Failed => "failed",
        }
    }

    /// 是否表示旧位置已无残留
    pub(crate) fn is_settled(self) -> bool {
        matches!(
            self,
            Self::Migrated | Self::AlreadyCurrent | Self::ConflictResolved
        )
    }
}

/// 单个集合成员的结果
#[derive(Debug, Clone)]
pub(crate) struct ItemOutcome {
    /// 集合标识
    pub group: &'static str,
    /// 旧位置名
    pub name: String,
    /// 目标路径（相对根）
    pub target: String,
    /// 处理结果
    pub state: ItemState,
    /// 失败原因或冲突说明
    pub detail: String,
}

/// 迁移报告
#[derive(Debug, Clone, Default)]
pub(crate) struct LayoutReport {
    /// 逐成员结果
    pub outcomes: Vec<ItemOutcome>,
    /// 本次是否推进了布局版本
    pub version_advanced: bool,
}

impl LayoutReport {
    /// 成功搬移（含冲突改用旧数据）的项数
    pub(crate) fn moved(&self) -> usize {
        self.outcomes
            .iter()
            .filter(|o| o.state == ItemState::Migrated)
            .count()
    }

    /// 失败项
    pub(crate) fn failures(&self) -> Vec<&ItemOutcome> {
        self.outcomes
            .iter()
            .filter(|o| o.state == ItemState::Failed)
            .collect()
    }

    /// 是否存在失败项（有失败则版本不推进）
    pub(crate) fn has_failures(&self) -> bool {
        !self.failures().is_empty()
    }

    /// 一行摘要（启动日志用；失败项带上集合与状态，便于直接定位）
    pub(crate) fn summary(&self) -> String {
        let detail = self
            .failures()
            .iter()
            .map(|o| format!("{}[{}]={}", o.target, o.group, o.state.as_str()))
            .collect::<Vec<_>>()
            .join(", ");
        let mut line = format!(
            "布局迁移：搬移 {} 项，一致清理 {} 项，冲突改用旧数据 {} 项，未处理 {} 项{}",
            self.moved(),
            self.outcomes
                .iter()
                .filter(|o| o.state == ItemState::AlreadyCurrent)
                .count(),
            self.outcomes
                .iter()
                .filter(|o| o.state == ItemState::ConflictResolved)
                .count(),
            self.outcomes
                .iter()
                .filter(|o| !o.state.is_settled())
                .count(),
            if self.version_advanced {
                "，版本已推进"
            } else {
                "，版本保持不变"
            }
        );
        if !detail.is_empty() {
            line.push_str(&format!("；失败/未处理：{detail}"));
        }
        line
    }
}
