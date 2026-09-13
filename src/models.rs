//! 数据模型：Project / Issue / Label 结构体与 serde 序列化。

use clap::ValueEnum;
use serde::{Deserialize, Serialize};

/// Issue 的 kind：问题 / 需求 / 杂务。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Problem,
    Requirement,
    Task,
}

impl Kind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Kind::Problem => "problem",
            Kind::Requirement => "requirement",
            Kind::Task => "task",
        }
    }
}

/// Issue 的状态（6 态，见 notes/DDD.md）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Open,
    Planned,
    Dev,
    Test,
    Done,
    Dropped,
}

impl Status {
    pub fn as_str(&self) -> &'static str {
        match self {
            Status::Open => "open",
            Status::Planned => "planned",
            Status::Dev => "dev",
            Status::Test => "test",
            Status::Done => "done",
            Status::Dropped => "dropped",
        }
    }
}

/// 项目（来源标签，非隔离边界）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub git: Option<String>,
    pub abs_dir: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Issue 条目。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Issue {
    pub id: i64,
    pub title: String,
    pub body: Option<String>,
    pub kind: Kind,
    pub status: Status,
    pub priority: i64,
    pub project: Option<String>,
    pub test_cmd: Option<String>,
    pub dropped_reason: Option<String>,
    pub last_commit_id: Option<String>,
    pub plan_id: Option<i64>,
    /// 直属挂载的 milestone（无 plan 的 issue 经 milestone_direct_issues；TUI 跳转用，#258）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direct_milestone: Option<i64>,
    pub machine_id: Option<String>,
    pub uid: Option<String>,
    pub hit_count: i64,
    pub labels: Vec<String>,
    /// label 名 → color 映射（TUI 渲染着色用，不进 export JSON）。
    #[serde(default, skip_serializing)]
    pub label_colors: std::collections::HashMap<String, String>,
    pub links: Vec<Link>,
    pub created_at: String,
    pub updated_at: String,
}

/// issue 链接类型：related（相关）/ solves（解决）/ duplicates（重复）/
/// blocked_by（被阻塞）/ blocks（阻塞）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum LinkType {
    Related,
    Solves,
    Duplicates,
    #[serde(rename = "blocked_by")]
    BlockedBy,
    #[serde(rename = "blocks")]
    Blocks,
}

impl LinkType {
    pub fn as_str(&self) -> &'static str {
        match self {
            LinkType::Related => "related",
            LinkType::Solves => "solves",
            LinkType::Duplicates => "duplicates",
            LinkType::BlockedBy => "blocked_by",
            LinkType::Blocks => "blocks",
        }
    }

    /// 反向类型的字符串表示（仅显示用，不落库）：
    /// solves → "solved-by"，duplicates → "duplicated-by"，
    /// blocked_by ↔ blocks 互逆，related 对称仍为 "related"。
    pub fn reverse(&self) -> &'static str {
        match self {
            LinkType::Related => "related",
            LinkType::Solves => "solved-by",
            LinkType::Duplicates => "duplicated-by",
            LinkType::BlockedBy => "blocks",
            LinkType::Blocks => "blocked_by",
        }
    }
}

/// issue 链接（从某 issue 视角聚合出向 + 入向）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Link {
    /// 对端 issue id
    pub other_id: i64,
    /// 对端 issue 标题
    pub other_title: String,
    /// 显示关系：related / solves / solved-by / duplicates / duplicated-by
    pub rel: String,
    pub created_at: String,
}

/// Label 标签。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Label {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub color: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// 容器状态（milestone/plan 共享，5 态派生）：open/running/partial/dropped/done。
/// open=从未开始；running=曾/正运行；partial=done+dropped 混合无活跃。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum ContainerStatus {
    Open,
    Running,
    Partial,
    Dropped,
    Done,
}

impl ContainerStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ContainerStatus::Open => "open",
            ContainerStatus::Running => "running",
            ContainerStatus::Partial => "partial",
            ContainerStatus::Dropped => "dropped",
            ContainerStatus::Done => "done",
        }
    }
}

/// 容器（milestone/plan 共享模型）：milestone 有 version，plan 有 milestone_id。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Container {
    pub id: i64,
    pub title: String,
    pub version: Option<String>,
    pub body: Option<String>,
    pub milestone_id: Option<i64>,
    pub status: ContainerStatus,
    pub created_at: String,
    pub updated_at: String,
}

/// issue 摘要（容器 show 内嵌用，避免拖全量 Issue）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueSummary {
    pub id: i64,
    pub title: String,
    pub kind: Kind,
    pub status: Status,
    pub project: Option<String>,
}

#[path = "models_impls.rs"]
mod impls;

#[cfg(test)]
#[path = "models_tests.rs"]
mod tests;
