//! models 枚举的 SQL 映射与显示实现（ToSql/FromSql/Display）。

use super::*;

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

impl std::fmt::Display for ContainerLinkType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl rusqlite::ToSql for ContainerLinkType {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        Ok(rusqlite::types::ToSqlOutput::Borrowed(self.as_str().into()))
    }
}

impl rusqlite::types::FromSql for ContainerLinkType {
    fn column_result(v: rusqlite::types::ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        match v.as_str()? {
            "blocked_by" => Ok(ContainerLinkType::BlockedBy),
            "blocks" => Ok(ContainerLinkType::Blocks),
            other => Err(rusqlite::types::FromSqlError::Other(
                format!("invalid container link type: {other}").into(),
            )),
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
