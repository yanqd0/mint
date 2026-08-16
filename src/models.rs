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
            "task" => Ok(Kind::Task),
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
    pub priority: i64,
    pub project_id: i64,
    pub project: Option<String>,
    pub test_cmd: Option<String>,
    pub dropped_reason: Option<String>,
    pub last_commit_id: Option<String>,
    pub plan_id: Option<i64>,
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

impl std::fmt::Display for LinkType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl rusqlite::ToSql for LinkType {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        Ok(rusqlite::types::ToSqlOutput::Borrowed(self.as_str().into()))
    }
}

impl rusqlite::types::FromSql for LinkType {
    fn column_result(v: rusqlite::types::ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        match v.as_str()? {
            "related" => Ok(LinkType::Related),
            "solves" => Ok(LinkType::Solves),
            "duplicates" => Ok(LinkType::Duplicates),
            "blocked_by" => Ok(LinkType::BlockedBy),
            "blocks" => Ok(LinkType::Blocks),
            other => Err(rusqlite::types::FromSqlError::Other(
                format!("invalid link type: {other}").into(),
            )),
        }
    }
}

/// issue 链接（从某 issue 视角聚合出向 + 入向）。
#[derive(Debug, Clone, Serialize, Deserialize)]
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
            "running" => Ok(ContainerStatus::Running),
            "partial" => Ok(ContainerStatus::Partial),
            "dropped" => Ok(ContainerStatus::Dropped),
            "done" => Ok(ContainerStatus::Done),
            other => Err(rusqlite::types::FromSqlError::Other(
                format!("invalid container status: {other}").into(),
            )),
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

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    /// Status：as_str / Display / ToSql-FromSql 往返。
    #[rstest]
    #[case(Status::Open, "open")]
    #[case(Status::Planned, "planned")]
    #[case(Status::Dev, "dev")]
    #[case(Status::Test, "test")]
    #[case(Status::Done, "done")]
    #[case(Status::Dropped, "dropped")]
    fn status_str_and_roundtrip(#[case] s: Status, #[case] text: &str) {
        assert_eq!(s.as_str(), text);
        assert_eq!(s.to_string(), text);
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute("CREATE TABLE t (x TEXT)", []).unwrap();
        conn.execute("INSERT INTO t VALUES (?1)", [s]).unwrap();
        let got: Status = conn.query_row("SELECT x FROM t", [], |r| r.get(0)).unwrap();
        assert_eq!(got, s);
    }

    /// Kind：as_str / Display / 往返。
    #[rstest]
    #[case(Kind::Problem, "problem")]
    #[case(Kind::Requirement, "requirement")]
    #[case(Kind::Task, "task")]
    fn kind_str_and_roundtrip(#[case] k: Kind, #[case] text: &str) {
        assert_eq!(k.as_str(), text);
        assert_eq!(k.to_string(), text);
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute("CREATE TABLE t (x TEXT)", []).unwrap();
        conn.execute("INSERT INTO t VALUES (?1)", [k]).unwrap();
        let got: Kind = conn.query_row("SELECT x FROM t", [], |r| r.get(0)).unwrap();
        assert_eq!(got, k);
    }

    /// LinkType：as_str / reverse / Display / 往返。
    #[rstest]
    #[case(LinkType::Related, "related", "related")]
    #[case(LinkType::Solves, "solves", "solved-by")]
    #[case(LinkType::Duplicates, "duplicates", "duplicated-by")]
    #[case(LinkType::BlockedBy, "blocked_by", "blocks")]
    #[case(LinkType::Blocks, "blocks", "blocked_by")]
    fn link_type_str_reverse_roundtrip(
        #[case] ty: LinkType,
        #[case] text: &str,
        #[case] rev: &str,
    ) {
        assert_eq!(ty.as_str(), text);
        assert_eq!(ty.to_string(), text);
        assert_eq!(ty.reverse(), rev);
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute("CREATE TABLE t (x TEXT)", []).unwrap();
        conn.execute("INSERT INTO t VALUES (?1)", [ty]).unwrap();
        let got: LinkType = conn.query_row("SELECT x FROM t", [], |r| r.get(0)).unwrap();
        assert_eq!(got, ty);
    }

    /// ContainerStatus：as_str / Display / 往返。
    #[rstest]
    #[case(ContainerStatus::Open, "open")]
    #[case(ContainerStatus::Running, "running")]
    #[case(ContainerStatus::Partial, "partial")]
    #[case(ContainerStatus::Dropped, "dropped")]
    #[case(ContainerStatus::Done, "done")]
    fn container_status_str_and_roundtrip(#[case] s: ContainerStatus, #[case] text: &str) {
        assert_eq!(s.as_str(), text);
        assert_eq!(s.to_string(), text);
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute("CREATE TABLE t (x TEXT)", []).unwrap();
        conn.execute("INSERT INTO t VALUES (?1)", [s]).unwrap();
        let got: ContainerStatus = conn.query_row("SELECT x FROM t", [], |r| r.get(0)).unwrap();
        assert_eq!(got, s);
    }

    /// 非法 Status 值 FromSql 报错（含大小写敏感）。
    #[rstest]
    #[case("bogus")]
    #[case("OPEN")]
    #[case("")]
    fn status_invalid_errors(#[case] val: &str) {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute("CREATE TABLE t (x TEXT)", []).unwrap();
        conn.execute("INSERT INTO t VALUES (?1)", [val]).unwrap();
        let err = conn
            .query_row::<Status, _, _>("SELECT x FROM t", [], |r| r.get(0))
            .unwrap_err();
        assert!(err.to_string().contains("invalid status"), "{err}");
    }

    /// 非法 Kind 值 FromSql 报错。
    #[rstest]
    #[case("bogus")]
    #[case("Problem")]
    fn kind_invalid_errors(#[case] val: &str) {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute("CREATE TABLE t (x TEXT)", []).unwrap();
        conn.execute("INSERT INTO t VALUES (?1)", [val]).unwrap();
        let err = conn
            .query_row::<Kind, _, _>("SELECT x FROM t", [], |r| r.get(0))
            .unwrap_err();
        assert!(err.to_string().contains("invalid kind"), "{err}");
    }

    /// 非法 LinkType 值 FromSql 报错。
    #[rstest]
    #[case("bogus")]
    #[case("solved-by")] // 反向字符串不落库
    #[case("blocked-by")] // 反向字符串不落库
    fn link_type_invalid_errors(#[case] val: &str) {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute("CREATE TABLE t (x TEXT)", []).unwrap();
        conn.execute("INSERT INTO t VALUES (?1)", [val]).unwrap();
        let err = conn
            .query_row::<LinkType, _, _>("SELECT x FROM t", [], |r| r.get(0))
            .unwrap_err();
        assert!(err.to_string().contains("invalid link type"), "{err}");
    }

    /// 非法 ContainerStatus 值 FromSql 报错。
    #[rstest]
    #[case("bogus")]
    #[case("RUNNING")]
    fn container_status_invalid_errors(#[case] val: &str) {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute("CREATE TABLE t (x TEXT)", []).unwrap();
        conn.execute("INSERT INTO t VALUES (?1)", [val]).unwrap();
        let err = conn
            .query_row::<ContainerStatus, _, _>("SELECT x FROM t", [], |r| r.get(0))
            .unwrap_err();
        assert!(
            err.to_string().contains("invalid container status"),
            "{err}"
        );
    }
}
