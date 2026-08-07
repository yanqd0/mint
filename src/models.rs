//! 数据模型：Project / Issue / Tag 结构体与 serde 序列化。

use serde::{Deserialize, Serialize};

/// Issue 的 kind：问题 / 需求。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Problem,
    Requirement,
}

/// Issue 的状态（6 态，见 notes/DDD.md）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Open,
    Planned,
    Dev,
    Test,
    Done,
    Dropped,
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
