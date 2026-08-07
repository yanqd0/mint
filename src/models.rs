//! 数据模型：Project / Issue / Tag 结构体与 serde 序列化。

use clap::ValueEnum;
use serde::{Deserialize, Serialize};

/// Issue 的 kind：问题 / 需求。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Problem,
    Requirement,
}

impl Kind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Kind::Problem => "problem",
            Kind::Requirement => "requirement",
        }
    }
}

impl std::fmt::Display for Kind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl rusqlite::ToSql for Kind {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        Ok(rusqlite::types::ToSqlOutput::Borrowed(self.as_str().into()))
    }
}

impl rusqlite::types::FromSql for Kind {
    fn column_result(v: rusqlite::types::ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        match v.as_str()? {
            "problem" => Ok(Kind::Problem),
            "requirement" => Ok(Kind::Requirement),
            other => Err(rusqlite::types::FromSqlError::Other(
                format!("invalid kind: {other}").into(),
            )),
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

impl std::fmt::Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl rusqlite::ToSql for Status {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        Ok(rusqlite::types::ToSqlOutput::Borrowed(self.as_str().into()))
    }
}

impl rusqlite::types::FromSql for Status {
    fn column_result(v: rusqlite::types::ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        match v.as_str()? {
            "open" => Ok(Status::Open),
            "planned" => Ok(Status::Planned),
            "dev" => Ok(Status::Dev),
            "test" => Ok(Status::Test),
            "done" => Ok(Status::Done),
            "dropped" => Ok(Status::Dropped),
            other => Err(rusqlite::types::FromSqlError::Other(
                format!("invalid status: {other}").into(),
            )),
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
    pub project_id: i64,
    pub project: Option<String>,
    pub test_cmd: Option<String>,
    pub dropped_reason: Option<String>,
    pub tags: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Tag 标签。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// 容器状态（roadmap/plan 共享）：open/done/dropped，独立于 issue 6 态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum ContainerStatus {
    Open,
    Done,
    Dropped,
}

impl ContainerStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ContainerStatus::Open => "open",
            ContainerStatus::Done => "done",
            ContainerStatus::Dropped => "dropped",
        }
    }
}

impl std::fmt::Display for ContainerStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl rusqlite::ToSql for ContainerStatus {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        Ok(rusqlite::types::ToSqlOutput::Borrowed(self.as_str().into()))
    }
}

impl rusqlite::types::FromSql for ContainerStatus {
    fn column_result(v: rusqlite::types::ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        match v.as_str()? {
            "open" => Ok(ContainerStatus::Open),
            "done" => Ok(ContainerStatus::Done),
            "dropped" => Ok(ContainerStatus::Dropped),
            other => Err(rusqlite::types::FromSqlError::Other(
                format!("invalid container status: {other}").into(),
            )),
        }
    }
}

/// 容器（roadmap/plan 共享模型）：聚合多个 issue。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Container {
    pub id: i64,
    pub title: String,
    pub description: Option<String>,
    pub status: ContainerStatus,
    pub dropped_reason: Option<String>,
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
