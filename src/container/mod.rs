//! 容器（milestone/plan）共享模块：聚合 issue/plan。
//!
//! 层级：milestone → plan（milestone_id）→ issue（plan_id）；milestone 可直接挂无 plan 的 issue
//! （milestone_direct_issues，二选一约束）。容器状态 5 态派生（写后同步，CLI 只读）。
//!
//! 子模块：`derive`（5 态派生纯函数）、`sync`（派生写回与级联）、`affiliation`（归属挂载）、
//! `lifecycle`（更新/删除/跨桶移动）。对外 API 由本模块 `pub use` 再导出，调用方路径不变。

use rusqlite::{Connection, OptionalExtension, params};

use crate::db;
use crate::error::Error;
use crate::models::{Container, ContainerStatus};

mod affiliation;
mod derive;
mod lifecycle;
mod sync;

pub use affiliation::{issues_for, link_direct, set_issue_plan, unlink_direct, unset_issue_plan};
pub(crate) use lifecycle::delete_txn;
pub use lifecycle::{
    delete_issue, delete_milestone, delete_plan, move_plan, update_milestone, update_plan,
};
pub use sync::{set_milestone_status, set_plan_status, sync_container_status};

#[cfg(test)]
mod tests;

/// 容器类型：milestone / plan。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContainerKind {
    Milestone,
    Plan,
}

impl ContainerKind {
    fn insert_sql(self) -> &'static str {
        match self {
            ContainerKind::Milestone => db::MILESTONE_INSERT,
            ContainerKind::Plan => db::PLAN_INSERT,
        }
    }

    fn list_sql(self) -> &'static str {
        match self {
            ContainerKind::Milestone => db::MILESTONE_LIST,
            ContainerKind::Plan => db::PLAN_LIST,
        }
    }

    fn select_sql(self) -> &'static str {
        match self {
            ContainerKind::Milestone => db::MILESTONE_SELECT,
            ContainerKind::Plan => db::PLAN_SELECT,
        }
    }
}

/// 新建容器，返回 id。milestone 必填 version；plan 可带 milestone_id。
pub fn create(
    conn: &Connection,
    kind: ContainerKind,
    title: &str,
    version: Option<&str>,
    body: Option<&str>,
    milestone_id: Option<i64>,
) -> Result<i64, Error> {
    match kind {
        ContainerKind::Milestone => {
            let v = version.filter(|s| !s.trim().is_empty());
            if v.is_none() {
                return Err(Error::Other("milestone requires --version".to_string()));
            }
            conn.execute(kind.insert_sql(), params![title, v, body])?;
        }
        ContainerKind::Plan => {
            conn.execute(kind.insert_sql(), params![title, body, milestone_id])?;
        }
    }
    Ok(conn.last_insert_rowid())
}

/// 列出全部容器（含子项计数），(容器, 计数)。
/// `status: Some(..)` 时按容器状态过滤（显式 status 放开 done 排除，对齐 issue_list）。
pub fn list(
    conn: &Connection,
    kind: ContainerKind,
    all: bool,
    status: Option<ContainerStatus>,
) -> Result<Vec<(Container, i64)>, Error> {
    let all_flag: i64 = if all { 1 } else { 0 };
    let mut stmt = conn.prepare(kind.list_sql())?;
    let rows = stmt.query_map(params![all_flag, status], |r| {
        let container = Container {
            id: r.get(0)?,
            title: r.get(1)?,
            version: r.get(2)?,
            body: r.get(3)?,
            milestone_id: r.get(4)?,
            status: r.get(5)?,
            created_at: r.get(6)?,
            updated_at: r.get(7)?,
        };
        let count: i64 = r.get(8)?;
        Ok((container, count))
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Error::from)
}

/// 查询单条容器（不存在返回 None）。
pub fn get(conn: &Connection, kind: ContainerKind, id: i64) -> Result<Option<Container>, Error> {
    conn.query_row(kind.select_sql(), params![id], |r| {
        Ok(Container {
            id: r.get(0)?,
            title: r.get(1)?,
            version: r.get(2)?,
            body: r.get(3)?,
            milestone_id: r.get(4)?,
            status: r.get(5)?,
            created_at: r.get(6)?,
            updated_at: r.get(7)?,
        })
    })
    .optional()
    .map_err(Error::from)
}
